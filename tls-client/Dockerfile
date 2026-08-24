# Builds bogdanfinn/tls-client-api from upstream github
# https://github.com/bogdanfinn/tls-client-api/releases
ARG TLS_CLIENT_API_VERSION=v1.15.1

# Runs on the host's own architecture and cross-compiles to the target, so a
# multi-arch build costs one native Go build per arch rather than an emulated one.
FROM --platform=$BUILDPLATFORM golang:1.24-bookworm AS builder

ARG TLS_CLIENT_API_VERSION
ARG TARGETOS
ARG TARGETARCH

WORKDIR /src

RUN git clone --depth 1 --branch "${TLS_CLIENT_API_VERSION}" \
      https://github.com/bogdanfinn/tls-client-api.git .

RUN go mod download && go mod verify

RUN CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
      go build -o /out/tls-client-api ./cmd/tls-client-api/main.go

FROM alpine

# Links the GHCR package to this repo, so it inherits the repo's README and
# shows up on the repo page instead of floating unattached under the account.
LABEL org.opencontainers.image.source=https://github.com/chewycrunch/woot-monitor

WORKDIR /app

RUN apk add --no-cache ca-certificates

COPY --from=builder /out/tls-client-api /app/tls-client-api

# main.go loads `<dir of executable>/config.dist.yml`
COPY config.yml /app/config.dist.yml

# gosoline serves this on the health port from config.yml. Not published, so it
# is only reachable from inside the container - which is where this runs.
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8081/health || exit 1

CMD ["/app/tls-client-api"]
