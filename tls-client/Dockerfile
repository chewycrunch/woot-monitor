# Builds bogdanfinn/tls-client-api from upstream github
# https://github.com/bogdanfinn/tls-client-api/releases
ARG TLS_CLIENT_API_VERSION=v1.15.1

FROM golang:1.24-bookworm AS builder

ARG TLS_CLIENT_API_VERSION

WORKDIR /src

RUN git clone --depth 1 --branch "${TLS_CLIENT_API_VERSION}" \
      https://github.com/bogdanfinn/tls-client-api.git .

RUN go mod download && go mod verify

RUN CGO_ENABLED=0 go build -o /out/tls-client-api ./cmd/tls-client-api/main.go

FROM alpine

WORKDIR /app

RUN apk add --no-cache ca-certificates

COPY --from=builder /out/tls-client-api /app/tls-client-api

# main.go loads `<dir of executable>/config.dist.yml`
COPY config.yml /app/config.dist.yml

CMD ["/app/tls-client-api"]
