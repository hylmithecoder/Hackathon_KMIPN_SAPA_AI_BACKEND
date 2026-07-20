# SAPA-AI CRM API — build helpers
#
#   make run     → cargo run (dev build)
#   make check   → cargo check
#   make test    → cargo test
#   make fmt     → cargo fmt
#   make clean   → cargo clean

BIN_NAME    := api_sapaai
NIGHTLY_RUSTC := $(shell rustup which --toolchain nightly rustc 2>/dev/null)
CARGO := RUSTC=$(NIGHTLY_RUSTC) rustup run nightly cargo

.DEFAULT_GOAL := run
.PHONY: run check test fmt clean help

## run: run the API with a dev build
run:
	$(CARGO) run

## check: type-check without producing a binary
check:
	$(CARGO) check

## test: run inline unit tests
test:
	$(CARGO) test

## fmt: format the source tree
fmt:
	$(CARGO) fmt

## clean: remove all build artifacts
clean:
	$(CARGO) clean

## help: list available targets
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/^## /  /'
