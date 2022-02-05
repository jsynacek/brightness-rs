use std::cmp::{max, min};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use ctrlc;

const ACTUAL_BRIGHTNESS: &str = "/sys/class/backlight/intel_backlight/brightness";
const MAX_BRIGHTNESS: &str = "/sys/class/backlight/intel_backlight/max_brightness";
const SOCKET_PATH: &str = "/run/brightness.sock";

enum Brightness {
    Actual,
    Maximum,
}

fn get_brightness(which: Brightness) -> Result<i32, String> {
    let path = match which {
        Brightness::Actual => ACTUAL_BRIGHTNESS,
        Brightness::Maximum => MAX_BRIGHTNESS,
    };
    let brightness = fs::read_to_string(path).or_else(|err| {
            Err(format!("Cannot read {}: {}", path, err))
        })?;
    brightness.trim().parse().or_else(|err| {
        Err(format!("Cannot parse brightness from {}: {}", path, err))
    })
}

fn change_brightness<F>(change: F) -> Result<(), String>
    where F: FnOnce(i32) -> i32
{
    let brightness = get_brightness(Brightness::Actual)?;
    let brightness = change(brightness);

    fs::write(ACTUAL_BRIGHTNESS, format!("{}\n", brightness)).or_else(|err| {
        Err(format!("Cannot change brightness in {}: {}", ACTUAL_BRIGHTNESS, err))
    })?;

    Ok(())
}

fn cleanup() {
    eprintln!("Cleaning up {}", SOCKET_PATH);
    if let Err(err) = fs::remove_file(SOCKET_PATH) {
        eprintln!("Cannot remove {}: {}", SOCKET_PATH, err);
    }
    std::process::exit(0);
}

fn main() -> Result<(), String> {
    if let Err(err) = ctrlc::set_handler(cleanup) {
        // Not fatal.
        eprintln!("Cannot set cleanup handler: {}", err);
    }

    let max_brightness = get_brightness(Brightness::Maximum)?;
    let listener = bind()?;
    for stream in listener.incoming() {
        let stream = stream.or_else(|err| {
            Err(format!("Cannot get connection from stream: {}", err))
        })?;
        handle_connection(max_brightness, stream);
    }

    Ok(())
}

fn bind() -> Result<UnixListener, String> {
    let listener = UnixListener::bind(SOCKET_PATH).or_else(|err| {
            Err(format!("Cannot bind {}: {}", SOCKET_PATH, err))
    })?;

    let metadata = fs::metadata(SOCKET_PATH).or_else(|err| {
        Err(format!("Cannot get metadata from {}: {}", SOCKET_PATH, err))
    })?;

    let mut p = metadata.permissions();
    p.set_mode(0o777);
    fs::set_permissions(SOCKET_PATH, p).or_else(|err| {
        Err(format!("Cannot set permissions on {}: {}", SOCKET_PATH, err))
    })?;

    Ok(listener)
}

fn handle_connection(max_brightness: i32, mut stream: UnixStream) {
    let mut buffer = [0; 1024];
    if let Err(err) = stream.read(&mut buffer) {
        eprintln!("Cannot read from stream: {}", err);
        return;
    }

    let request = String::from_utf8_lossy(&buffer);
    // It turns out that the rest of the buffer is made out of UTF-8 valid null characters...
    let request = request.trim_end_matches(char::from('\0')).trim_end();

    let mut response = String::from("Ok");
    if request == "+" {
        if let Err(err) = change_brightness(|b| { min(max_brightness, b + max_brightness / 20) }) {
            let err = String::from("Error: ") + &err;
            eprintln!("{}", err);
            response = "Error: Cannot change brightness".to_string();
        }
    } else if request == "-" {
        if let Err(err) = change_brightness(|b| { max(max_brightness / 100, b - max_brightness / 20) }) {
            let err = String::from("Error: ") + &err;
            eprintln!("{}", err);
            response = "Error: Cannot change brightness".to_string();
        }
    } else {
        eprintln!("Invalid request: {}", request);
        response = "Invalid request".to_string();
    }

    response += "\n";
    if let Err(err) = stream.write(response.as_bytes()) {
        eprintln!("Cannot write into stream: {}", err);
    } else if let Err(err) = stream.flush() {
        eprintln!("Cannot flush stream: {}", err);
    }
}

