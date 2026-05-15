BIN_NAME ?= spacetop
PREFIX ?= $(HOME)/.cargo/bin
BINDIR ?= $(PREFIX)

export SENTRY_DSN

.PHONY: build bootstrap lint clean install uninstall

build: lint
	SENTRY_DSN="$(SENTRY_DSN)" cargo build --release

bootstrap:
	@command -v rustup >/dev/null 2>&1 || { \
		echo "error: rustup not found. Install Rust via https://rustup.rs and re-run 'make bootstrap'." >&2; exit 1; }
	rustup component add clippy

lint:
	@cargo clippy --version >/dev/null 2>&1 || { \
		echo "error: cargo-clippy is not installed for the active toolchain." >&2; \
		echo "       run 'make bootstrap' (or 'rustup component add clippy') and retry." >&2; \
		exit 1; }
	cargo clippy --all-targets --all-features -- -D warnings

clean:
	cargo clean

install: build
	install -d "$(BINDIR)"
	install -m 755 "target/release/$(BIN_NAME)" "$(BINDIR)/$(BIN_NAME)"

uninstall:
	rm -f "$(BINDIR)/$(BIN_NAME)"
