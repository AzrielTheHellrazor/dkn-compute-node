# load .env
ifneq (,$(wildcard ./.env))
		include .env
		export
endif

###############################################################################
.PHONY: launch #       | Run with INFO logs in release mode
launch:
		RUST_LOG=none,dkn_compute=info cargo run --release

.PHONY: run #          | Run with INFO logs
run:
		RUST_LOG=none,dkn_compute=info cargo run

.PHONY: debug #        | Run with DEBUG logs with INFO log-level workflows
debug:
		RUST_LOG=warn,dkn_compute=debug,ollama_workflows=info cargo run

.PHONY: trace #        | Run with TRACE logs
trace:
		RUST_LOG=warn,dkn_compute=trace,libp2p=debug cargo run

.PHONY: build #        | Build
build:
		cargo build

.PHONY: profile-cpu #  | Profile CPU usage with flamegraph
profile-cpu:
	  cargo flamegraph --root --profile=profiling --features=profiling

.PHONY: profile-mem #  | Profile memory usage with instruments
profile-mem:
	  cargo instruments --profile=profiling --features=profiling -t Allocations

.PHONY: version #      | Print version
version:
	  @cargo pkgid | cut -d@ -f2

.PHONY: ollama-cpu #  | Run Ollama CPU container
ollama-cpu:
		docker run -p=11434:11434 -v=${HOME}/.ollama:/root/.ollama ollama/ollama

###############################################################################
.PHONY: test #         | Run tests
test:
		cargo test

###############################################################################
.PHONY: lint #         | Run linter (clippy)
lint:
		cargo clippy

.PHONY: format #       | Run formatter (cargo fmt)
format:
		cargo fmt -v

###############################################################################
.PHONY: docs #         | Generate & open crate documentation
docs:
		cargo doc --open --no-deps

.PHONY: env #          | Print active environment
env:
		@echo "Wallet Secret: ${DKN_WALLET_SECRET_KEY}"
		@echo "Admin Public: ${DKN_ADMIN_PUBLIC_KEY}"

# https://stackoverflow.com/a/45843594
.PHONY: help #         | List targets
help:                                                                                                                    
		@grep '^.PHONY: .* #' Makefile | sed 's/\.PHONY: \(.*\) # \(.*\)/\1 \2/' | expand -t20
