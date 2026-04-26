# Spacetop

Rust TUI for browsing Spacedock workflow state.

## Lint Gate

Before marking any task complete, run:

    make lint

All clippy warnings are treated as errors (`-D warnings`). Fix every diagnostic before committing.

## Commands

| Command | Purpose |
|---------|---------|
| `make build` | Release build (also runs lint) |
| `make lint` | Run clippy only |
| `make install` | Build and install to `~/.cargo/bin` |
| `cargo test` | Run all tests |
