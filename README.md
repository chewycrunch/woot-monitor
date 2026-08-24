# woot-monitor

Woot.com monitor for new deals

## Packages

### `monitor/`

The core monitor, written in Rust. Polls the Woot API for new deals, filters by keywords, manages proxies, and sends Discord webhook notifications.

### `tls-client/`

A Go HTTP API wrapping [bogdanfinn/tls-client](https://github.com/bogdanfinn/tls-client-api) to handle TLS fingerprinting. Used by the monitor to make requests that bypass TLS-based bot detection.

Upstream is built from source at image build time rather than vendored here — this directory holds only a `Dockerfile` (pinned to an upstream tag) and our `config.yml`. See [`tls-client/README.md`](tls-client/README.md).

## Running locally

The monitor talks to the tls-client API at `TLS_API_URL`, defaulting to
`http://127.0.0.1:8080`. Nothing needs Docker:

```sh
task tls-client-local      # downloads the upstream release binary, runs it on :8080
cd monitor && cargo run    # in another shell
```

`task tls-client-local` fetches the release matching the version pinned in
`tls-client/Dockerfile` for your OS and arch, and runs it against
`tls-client/config.dev.yml` — no Go toolchain, no build. The binary is cached in
`.local/`, which is gitignored; delete it to pick up a version bump.

To point the monitor at an API somewhere else, set `TLS_API_URL`
(see `monitor/.env.example`; `.env` is loaded at startup).

`docker-bake.hcl` holds the image build definitions — it is a build manifest, not a
runnable stack. Images are published to GHCR with `task deploy`.

## Configuration

Two files hold private data and are gitignored, so each checkout and each server
keeps its own copy:

| File | Copy from | Holds |
|---|---|---|
| `monitor/config.yaml` | `monitor/config.example.yaml` | webhook URLs, keywords, ASINs |
| `monitor/proxies/proxies.txt` | `monitor/proxies/proxies.example.txt` | `ip:port` or `ip:port:user:pass`, one per line |

```sh
cd monitor
cp config.example.yaml config.yaml
cp proxies/proxies.example.txt proxies/proxies.txt
```

The monitor reads both at paths relative to its working directory, so run it from
`monitor/` locally and from `/app` in the image. A missing or malformed
`config.yaml` aborts at startup with the path in the message; a missing
`proxies.txt` is not an error — it yields an empty list, visible as `count=0` in
the startup log.

Everything else is environment variables, loaded from `monitor/.env` at startup:
`TLS_API_URL`, `RUST_LOG`, `LOG_FORMAT`. See `monitor/.env.example`.

## Running the published images on a server

Neither config file is baked into the image; both are bind-mounted, which is what
lets one published image serve every deployment. `compose.example.yaml` is a
ready-made stack — copy it to the server alongside your `config.yaml` and
`proxies.txt`, and adjust the mount paths if you keep them elsewhere.

For a one-off run without compose:

```sh
docker run --rm \
  -e TLS_API_URL=http://tls-client:8080 \
  -v "$PWD/monitor/config.yaml:/app/config.yaml:ro" \
  -v "$PWD/monitor/proxies/proxies.txt:/app/proxies/proxies.txt:ro" \
  ghcr.io/chewycrunch/woot-monitor/monitor:latest
```

