# Woot TLS Client API

[bogdanfinn/tls-client-api](https://github.com/bogdanfinn/tls-client-api), built from
upstream at image build time. No source is vendored here — this directory holds only
the Dockerfile and our config.

## Updating

Bump `TLS_CLIENT_API_VERSION` in the `Dockerfile` to a tag from
[releases](https://github.com/bogdanfinn/tls-client-api/releases), then `make tls-client`.

## Config

`config.yml` is copied into the image as `/app/config.dist.yml`, which is the path
upstream's `main.go` loads (it resolves `config.dist.yml` next to the executable).

Individual scalar values can also be overridden at runtime with env vars, without a
rebuild — gosoline maps config keys to `UPPER_SNAKE_CASE`:

| Config key        | Env var           |
| ----------------- | ----------------- |
| `api.port`        | `API_PORT`        |
| `api.health.port` | `API_HEALTH_PORT` |
| `env`             | `ENV`             |

List values such as `api_auth_keys` do *not* override cleanly this way (setting
`API_AUTH_KEYS` collapses the list into one literal string) — edit `config.yml` instead.

The auth key in `config.yml` must match the `x-api-key` the monitor sends
(see `monitor/src/monitor/instance.rs`).
