# # ----------------Commands----------------
#
# # change the 20 value in printf to adjust width
# # Use ' ## some comment' behind a command and it will be added to the help message automatically




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

# Ensure cargo-nextest is installed
setup_nextest:
	@which cargo-nextest >/dev/null || cargo install cargo-nextest

test: setup_nextest ## Run tests single threaded due to s3 race conditions
	CARGO_NEXTEST_NO_CAPTURE=1 \
	cargo nextest run --test-threads=1


test: setup_nextest ## Run tests single threaded due to s3 race conditions
	AWS_REGION=us-east-2 \
	AWS_ENDPOINT=https://s3.us-east-2.amazonaws.com \
	CARGO_NEXTEST_NO_CAPTURE=1 \
	cargo nextest run --test-threads=1


test-tracer: setup_nextest ## Run tests single threaded due to s3 race conditions
	AWS_REGION=us-east-2 \
	AWS_ENDPOINT=https://s3.us-east-2.amazonaws.com \
	CARGO_NEXTEST_NO_CAPTURE=1 \
	cargo nextest run --test-threads=1  --workspace --exclude integration_tests --exclude tracer_ebpf_kernel --config 'target."cfg(all())".runner="sudo -E"' --no-fail-fast


test-simple_queries: setup_nextest ## Run Docker Based Tests(Simple Queries)
	AWS_REGION=us-east-2 \
	AWS_ENDPOINT=https://s3.us-east-2.amazonaws.com \
	CARGO_NEXTEST_NO_CAPTURE=1 \
	cargo nextest run --no-capture -p integration_tests  --test simple_queries

test-parallel: setup_nextest ## Run Docker Based Tests(Parallel)
	AWS_REGION=us-east-2 \
	AWS_ENDPOINT=https://s3.us-east-2.amazonaws.com \
	CARGO_NEXTEST_NO_CAPTURE=1 \
	cargo nextest run --no-capture  -p integration_tests --test parallel

	
all: format check test clippy  ## format, check, test, clippy.

# --------------Configuration-------------
#
#  .NOTPARALLEL: ; # wait for this target to finish
.EXPORT_ALL_VARIABLES: ; # send all vars to shell


.PHONY: docs all # All targets are accessible for user
	.DEFAULT: help # Running Make will run the help target

MAKEFLAGS += --no-print-directory # dont add message about entering and leaving the working directory
