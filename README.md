# SpO₂

SpO₂ is a monitor for Kubernetes pods. For more detail and background, see the [blog post announcing the release of the project](https://blog.meilisearch.com/spo2-the-little-dynamic-monitoring-tool/). 

(In medicine, SpO₂ refers to oxygen saturation levels in blood and is used as a measure of a person's health. O₂ is the scientific designation for an oxygen atom.) 

This project uses [sled](https://github.com/spacejam/sled) to permanently save the health checked URLs.
It provides a websocket API that returns the changing status of the health checked URLs.

SpO₂ doesn't support SSL out of the box, if you need [you can setup an Nginx server as we do][1].

[1]: /enable-ssl.md

![SpO2 dashboard screenshot](/misc/screenshot.png)

## Usage

You must have [installed Rust](https://rustup.rs/) on your computer first.

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

Calling this route is "kind of" idenpotent, it means that it will not run another health checker on this URL but the custom json data will be updated.

```bash
curl -i -X PUT 'http://127.0.0.1:8000/?url=http%3A%2F%2Flocalhost%2Fhealth' -d '"your custom json data"'

# Note that 'http%3A%2F%2Flocalhost%2Fhealth' is the url to health check
# but it is url encoded and correspond to 'http://localhost/health'
```

### Remove an health checked URL

Calling this route will remove the URL from the health check pool and return you the custom json data associated with it.

```bash
curl -i -X DELETE 'http://127.0.0.1:8000/?url=http%3A%2F%2Flocalhost%2Fhealth'
```

### Get an health checked URL data

Will return the associated data of an already health checked URL.

```bash
curl -i -X GET 'http://127.0.0.1:8000/?url=http%3A%2F%2Flocalhost%2Fhealth'
```

### Get all the health checked URLs

Will return the list of all the health checked URLs aloang with the data associated with them.

```bash
curl -i -X GET 'http://127.0.0.1:8000/all'
```
