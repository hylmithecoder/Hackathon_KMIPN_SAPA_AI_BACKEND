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
# `make` is expected to run inside shell.nix. Keep Cargo, rustc and the linker
# from that same Nix environment; mixing Nix Cargo with a rustup rustc leaves
# linker wrappers pointing at garbage-collected /nix/store paths.
CARGO := cargo
MUSL_CARGO := cargo-musl
MUSL_TARGET_DIR ?= target-musl

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
# Nix's musl cross-rustc defaults to `-crt-static` so its outputs can refer to
# Nix store libraries. Override that setting on the final binary link to make
# this artifact portable outside the Nix store.
build-static:
	CARGO_TARGET_DIR="$(MUSL_TARGET_DIR)" CFLAGS_x86_64_unknown_linux_musl="$(MUSL_CFLAGS)" CFLAGS="$(MUSL_CFLAGS)" $(MUSL_CARGO) rustc --release --target x86_64-unknown-linux-musl --bin "$(BIN_NAME)" -- -C target-feature=+crt-static

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
