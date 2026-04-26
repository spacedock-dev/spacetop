BIN_NAME ?= spacetop
PREFIX ?= $(HOME)/.cargo/bin
BINDIR ?= $(PREFIX)

export SENTRY_DSN

.PHONY: build lint clean install uninstall

build: lint
	SENTRY_DSN="$(SENTRY_DSN)" cargo build --release

lint:
	cargo clippy --all-targets --all-features -- -D warnings

clean:
	cargo clean

install: build
	install -d "$(BINDIR)"
	install -m 755 "target/release/$(BIN_NAME)" "$(BINDIR)/$(BIN_NAME)"

uninstall:
	rm -f "$(BINDIR)/$(BIN_NAME)"
