# SAPA-AI CRM API — build helpers
#
#   make run          → cargo run (dev build)
#   make build        → cargo build --release
#   make build-static → cargo build --release --target x86_64-unknown-linux-musl
#   make check        → cargo check
#   make test         → cargo test
#   make fmt          → cargo fmt
#   make clean        → cargo clean
#   make watch        → auto-reload dev server on file changes

BIN_NAME    := api_sapaai
NIGHTLY_RUSTC := $(shell rustup which --toolchain nightly rustc 2>/dev/null)
CARGO := RUSTC=$(NIGHTLY_RUSTC) rustup run nightly cargo

.DEFAULT_GOAL := run
.PHONY: run build build-static check test fmt clean watch help

MUSL_CFLAGS := -U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -DSQLITE_DISABLE_LFS=1 -U_LARGEFILE64_SOURCE -U_LARGEFILE_SOURCE

## run: run the API with a dev build
run:
	$(CARGO) run

## build: build optimized release binary (dynamic linking)
build:
	$(CARGO) build --release

## build-static: build fully statically linked release binary (musl)
build-static:
	CFLAGS_x86_64_unknown_linux_musl="$(MUSL_CFLAGS)" CFLAGS="$(MUSL_CFLAGS)" $(CARGO) build --release --target x86_64-unknown-linux-musl

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

## watch: auto-reload dev server on Rust/TOML file changes
watch:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		$(CARGO) watch -x run; \
	else \
		echo "cargo-watch is not installed."; \
		echo "Install it with: cargo install cargo-watch"; \
		echo "Then run: make watch"; \
		exit 1; \
	fi

## help: list available targets
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/^## /  /'

