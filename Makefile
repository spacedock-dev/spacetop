BIN_NAME ?= spacetop
PREFIX ?= $(HOME)/.cargo/bin
BINDIR ?= $(PREFIX)

.PHONY: build install uninstall

build:
	cargo build --release

install: build
	install -d "$(BINDIR)"
	install -m 755 "target/release/$(BIN_NAME)" "$(BINDIR)/$(BIN_NAME)"

uninstall:
	rm -f "$(BINDIR)/$(BIN_NAME)"
