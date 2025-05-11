# ----------------Commands----------------
#
# Use ' ## some comment' behind a command and it will be added to the help message automatically

help: ## Show this help message
	@awk 'BEGIN {FS = ":.*?## "}; /^[a-zA-Z0-9_-]+:.*?## / {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST) | grep -v '^help:.*?## '

format-check: ## cargo fmt --check
	cargo fmt --all -- --check

format: ## cargo fmt
	cargo fmt

clippy: ## cargo clippy 
	cargo clippy -- -D warnings

check: ## cargo check 
	cargo check

test: ## Run all tests
	cargo test

test-simple_queries: ## Run simple_queries integration test
	AWS_REGION=us-east-2 \
	AWS_ENDPOINT=https://s3.us-east-2.amazonaws.com \
	cargo test --test simple_queries -p integration_tests -- --nocapture

test-parallel: ## Run parallel integration test
	AWS_REGION=us-east-2 \
	AWS_ENDPOINT=https://s3.us-east-2.amazonaws.com \
	cargo test --test parallel -p integration_tests -- --nocapture

all: format check test clippy  ## format, check, test, clippy.

# --------------Configuration-------------
#
#  .NOTPARALLEL: ; # wait for this target to finish
.EXPORT_ALL_VARIABLES: ; # send all vars to shell

.PHONY: docs all
.DEFAULT: help

MAKEFLAGS += --no-print-directory