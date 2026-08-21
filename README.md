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

