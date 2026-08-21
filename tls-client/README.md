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

### dev vs prod

`config.yml` is the config that is baked into the image and deployed. `config.dev.yml`
is an *overlay* on it, not a second copy: `make tls-client-local` links `config.yml` in
as the base and passes the overlay with `--config`, which gosoline merges over it. Only
the handful of keys that differ live in the overlay, so there is nothing to keep in
sync and local runs exercise the same ports and auth keys that ship.

Only settings that genuinely cannot be carried by an env var need to be in the overlay.
The log level is one: `LOG_HANDLERS_MAIN_LEVEL` has no effect, unlike scalars such as
`API_PORT`.

### Running the binary by hand

Upstream's docs say to "modify the `config.dist.yml` file next to the binary", which
holds as long as you run the binary from its own directory — the file is actually
resolved against the *working directory*, and startup fails outright without it.
`--config other.yml` does not lift that requirement: it selects an additional file to
merge on top, but `./config.dist.yml` must still exist.
