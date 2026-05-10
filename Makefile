.PHONY: login build push deploy monitor tls-client

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
