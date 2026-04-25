# Spacetop

Spacetop is a Rust terminal UI for browsing [Spacedock](https://github.com/clkao/spacedock) workflow state.

Spacedock stores workflow progress as markdown files in git. A workflow directory typically contains a `README.md` that defines stages and gates, plus work item files with YAML frontmatter such as `id`, `title`, and `status`. Spacetop is intended to make those state files easier to inspect from the terminal.

## Goals

- Discover or open a Spacedock workflow directory.
- Parse workflow metadata and markdown work item frontmatter.
- Browse work items by status, stage, or file.
- Preview the selected item's markdown body and stage reports.
- Surface useful workflow signals such as pending gates, blocked items, stale items, and active work.

## Status

This repository is being initialized. The first implementation target is a read-only TUI that treats Spacedock markdown files as the source of truth and does not mutate workflow state.

## Expected Stack

- Rust
- `ratatui` for terminal UI rendering
- `crossterm` for terminal backend and input events
- `serde` and `serde_yaml` for structured metadata parsing

## Development

The Rust crate has not been scaffolded yet. Once it exists, the expected local workflow will be:

```bash
cargo fmt
cargo test
cargo run -- --workflow-dir /path/to/workflow
```

### Install Local Build

Use the provided Makefile targets to build and install a local release binary:

```bash
make build
make install
```

By default, install places the binary at `~/.cargo/bin/spacetop`.

To install to a different location, override `PREFIX`:

```bash
make install PREFIX=/usr/local/bin
```

To remove the installed binary:

```bash
make uninstall
```

## Safety

Spacetop should be read-only by default. Future write features should make state changes explicit and easy to audit through git.
