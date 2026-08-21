# woot-monitor

Woot.com monitor for new deals

## Packages

### `monitor/`

The core monitor, written in Rust. Polls the Woot API for new deals, filters by keywords, manages proxies, and sends Discord webhook notifications.

### `tls-client/`

A Go HTTP API wrapping [bogdanfinn/tls-client](https://github.com/bogdanfinn/tls-client-api) to handle TLS fingerprinting. Used by the monitor to make requests that bypass TLS-based bot detection.

Upstream is built from source at image build time rather than vendored here — this directory holds only a `Dockerfile` (pinned to an upstream tag) and our `config.yml`. See [`tls-client/README.md`](tls-client/README.md).
