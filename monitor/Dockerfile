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
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl --bin woot-monitor

# --- Stage 4: Minimal runtime image (Alpine) ---
FROM alpine AS runtime
RUN addgroup -S appuser && adduser -S appuser -G appuser
WORKDIR /app

# Install TLS certs for HTTPS
RUN apk add --no-cache ca-certificates

# Copy static binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/woot-monitor /usr/local/bin/woot-monitor

# Copy config + proxies
COPY ./proxies ./proxies
COPY ./config.yaml ./config.yaml


USER appuser
CMD ["/usr/local/bin/woot-monitor"]
