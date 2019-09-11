# SpO₂
SpO₂ is oxygen saturation and is used in medical person monitoring.

This project uses [sled](https://github.com/spacejam/sled) to permanently save the health checked URLs.
It provides a websocket API that returns the changing status of the health checked URLs.

## Usage

You must have installed Rust on your computer.

```bash
# to try it in debug mode
cargo run

# or in release mode
cargo run --release

# you can also change the listen addrs with env variables
export HTTP_LISTEN_ADDR='127.0.0.1:8000'
export WS_LISTEN_ADDR='127.0.0.1:8001'
cargo run --release

# with the previous settings the HTTP and the WebSocket server will be
# available on http://127.0.0.1:8000/ and ws://127.0.0.1:8001/ respectively

# to enable the slack notifier you must set the corresponding env variable
export SLACK_HOOK_URL='Your Slack Webhook URL'
cargo run --release
```

### Add or Update a new URL to health check

```bash
curl -i -X PUT 'http://127.0.0.1:8000/http%3A%2F%2Flocalhost%2Fhealth' -d '"your custom json data"'

# Note that 'http%3A%2F%2Flocalhost%2Fhealth' is the url to health check
# but it is url encoded and correspond to 'http://localhost/health'

# Calling this function is "kind of" idenpotent, it means that it will not run another
# health checker on this URL but the custom json data will be updated
```

### Remove an health checked URL

```bash
curl -i -X DELETE 'http://127.0.0.1:8000/http%3A%2F%2Flocalhost%2Fhealth'

# Calling this function will remove the URL from the health check pool
# and return you the custom json data you associated to it
```

### Get an health checked URL data

```bash
curl -i -X GET 'http://127.0.0.1:8000/http%3A%2F%2Flocalhost%2Fhealth'

# Will return the associated data of an already health checked URL
```

### Get all the health checked URLs

```bash
curl -i -X GET 'http://127.0.0.1:8000/'

# Will return the list of all the health checked URLs
# with the data associated with them
```
