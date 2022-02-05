# Rusty brightness service
Simple service that handles requests over a unix socket.
- `+` increases brightness
- `-` decreases brightness

## Example
```
# systemctl start brightness-rs.service
# nc -U /run/brightness.sock <<< +
# nc -U /run/brightness.sock <<< -
```
