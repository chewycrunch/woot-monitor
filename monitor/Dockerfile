# --- Stage 1: Base image with musl + cargo-chef ---
FROM clux/muslrust:stable AS chef
USER root
RUN cargo install cargo-chef
WORKDIR /app

# --- Stage 2: Generate dependency recipe ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Stage 3: Cook dependencies & build binary ---
FROM chef AS builder

# The musl target has to match the image's own architecture: each clux/muslrust
# image carries only its native musl toolchain, so hardcoding one triple breaks
# the moment you build on the other arch.
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl ;; \
      arm64) echo aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac > /rust-target

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target "$(cat /rust-target)" --recipe-path recipe.json
COPY . .
RUN cargo build --release --target "$(cat /rust-target)" --bin woot-monitor

# Lift the binary out of its per-target directory so the runtime stage has a
# fixed path to copy from.
RUN cp "/app/target/$(cat /rust-target)/release/woot-monitor" /woot-monitor

# --- Stage 4: Minimal runtime image (Alpine) ---
FROM alpine AS runtime

# Links the GHCR package to this repo, so it inherits the repo's README and
# shows up on the repo page instead of floating unattached under the account.
LABEL org.opencontainers.image.source=https://github.com/chewycrunch/woot-monitor

RUN addgroup -S appuser && adduser -S appuser -G appuser
WORKDIR /app

# Install TLS certs for HTTPS
RUN apk add --no-cache ca-certificates

# Copy static binary
COPY --from=builder /woot-monitor /usr/local/bin/woot-monitor

# Copy config + proxies
COPY ./proxies ./proxies
COPY ./config.yaml ./config.yaml


USER appuser
CMD ["/usr/local/bin/woot-monitor"]
