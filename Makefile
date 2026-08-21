.PHONY: login build push deploy monitor tls-client tls-client-local

# Personal overrides — copy Makefile.local.example to Makefile.local and fill in your values
-include Makefile.local

ACCOUNT?=your-aws-account-id
REGION?=us-east-1
REGISTRY=$(ACCOUNT).dkr.ecr.$(REGION).amazonaws.com

login:
	aws ecr get-login-password --region $(REGION) --profile personal | docker login --username AWS --password-stdin $(REGISTRY)

build:
	docker compose build

push:
	docker compose push

deploy: login build push

monitor:
	docker compose build monitor && docker compose push monitor

tls-client:
	docker compose build tls-client && docker compose push tls-client

# --- Local development ---------------------------------------------------------
# Runs the tls-client API from an upstream release binary, so `cargo run` in monitor/
# has something to talk to on the default TLS_API_URL. No Go toolchain, no Docker.
# The version tracks the Dockerfile so there is only one place to bump; the binary is
# named after the release, so a bump downloads the new one instead of reusing a stale
# cache.

TLS_CLIENT_VERSION=$(shell sed -n 's/^ARG TLS_CLIENT_API_VERSION=v//p' tls-client/Dockerfile)
TLS_CLIENT_OS=$(shell uname -s | tr A-Z a-z)
TLS_CLIENT_ARCH=$(shell uname -m | sed 's/x86_64/amd64/')
TLS_CLIENT_ASSET=tls-client-api-$(TLS_CLIENT_OS)-$(TLS_CLIENT_ARCH)-$(TLS_CLIENT_VERSION)
LOCAL_DIR=.local/tls-client

tls-client-local:
	@test -n "$(TLS_CLIENT_VERSION)" || \
		{ echo "no ARG TLS_CLIENT_API_VERSION found in tls-client/Dockerfile"; exit 1; }
	@mkdir -p $(LOCAL_DIR)
	@test -x $(LOCAL_DIR)/$(TLS_CLIENT_ASSET) || { \
		echo "Downloading $(TLS_CLIENT_ASSET)"; \
		curl -fsSL --proto '=https' -o $(LOCAL_DIR)/$(TLS_CLIENT_ASSET) \
			https://github.com/bogdanfinn/tls-client-api/releases/download/v$(TLS_CLIENT_VERSION)/$(TLS_CLIENT_ASSET); \
		chmod +x $(LOCAL_DIR)/$(TLS_CLIENT_ASSET); \
	}
	@# The binary reads ./config.dist.yml from its working directory and refuses to
	@# start without it, so link the deployed config in as the base and layer the dev
	@# overlay on top with --config.
	@ln -sf ../../tls-client/config.yml $(LOCAL_DIR)/config.dist.yml
	@ln -sf ../../tls-client/config.dev.yml $(LOCAL_DIR)/config.dev.yml
	cd $(LOCAL_DIR) && ./$(TLS_CLIENT_ASSET) --config config.dev.yml
