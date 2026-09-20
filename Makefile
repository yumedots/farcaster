PROJECT ?= $(CURDIR)
CARGO_TARGET_DIR ?= $(CURDIR)/target
GPUI_GHOSTTY_DIR ?= $(abspath ../gpui-ghostty)
LOG_LINES ?= 50
DEFAULT_FARCASTER_DATA_DIR := $(if $(XDG_DATA_HOME),$(XDG_DATA_HOME),$(HOME)/.local/share)/farcaster
LOG_FILE ?= $(if $(FARCASTER_DATA_DIR),$(FARCASTER_DATA_DIR),$(DEFAULT_FARCASTER_DATA_DIR))/logs/farcaster.log
TAIL_ARGS ?= -n $(LOG_LINES)
BUMP ?= patch
DEP_GRAPH := Cargo.toml Cargo.lock
INCREMENTAL_STAMP := $(CARGO_TARGET_DIR)/.incremental-stamp

DEFAULT_GOAL := build

.PHONY: build run test e2e debug release release-local release-debug release-preview release-publish bundle bundle-relaunch package logs fmt check check-flake clippy clean prune-incremental

$(INCREMENTAL_STAMP): $(DEP_GRAPH)
	@mkdir -p "$(dir $@)"
	@if [ -f "$@" ]; then \
		echo "dependency graph changed: pruning the local incremental cache"; \
		CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo clean -p farcaster; \
	fi
	@touch "$@"

build: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo build

run: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo run -- "$(PROJECT)"

test: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo test

e2e: | $(INCREMENTAL_STAMP)
	@HARNESS="$(HARNESS)" CASE="$(CASE)" CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" \
		sh scripts/e2e.sh

debug: | $(INCREMENTAL_STAMP)
	@DEBUG=true CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo run -- "$(PROJECT)"

release: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo run --release -- "$(PROJECT)"

release-local: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo \
		--config 'paths = ["$(GPUI_GHOSTTY_DIR)/crates/gpui-ghostty"]' \
		run --release -- "$(PROJECT)"

release-debug: | $(INCREMENTAL_STAMP)
	@DEBUG=true CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo run --release -- "$(PROJECT)"

release-preview: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo release "$(BUMP)" --package farcaster

release-publish: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo release "$(BUMP)" --package farcaster --execute

bundle: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" BUNDLE_FORMATS="$(BUNDLE_FORMATS)" PROJECT="$(PROJECT)" ./scripts/bundle.sh

bundle-relaunch: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" BUNDLE_FORMATS="$(BUNDLE_FORMATS)" PROJECT="$(PROJECT)" ./scripts/bundle.sh --relaunch

package: | $(INCREMENTAL_STAMP)
	@test -n "$(FORMAT)" || (echo "usage: make package FORMAT=app|dmg|appimage|deb|pacman" >&2; exit 1)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" BUNDLE_FORMATS="$(FORMAT)" ./scripts/bundle.sh

logs:
	@tail $(TAIL_ARGS) "$(LOG_FILE)"

fmt:
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo fmt

clippy: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo clippy --all-targets -- -D warnings

check: | $(INCREMENTAL_STAMP)
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo fmt --check
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo test
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo check
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo clippy --all-targets -- -D warnings

check-flake:
	@nix flake check

clean:
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo clean

prune-incremental:
	@CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" cargo clean -p farcaster
