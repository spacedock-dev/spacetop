---
id: 033
title: Bootstrap clippy for fresh environments before running `make lint`
status: plan
source: user request 2026-04-27
score: 0.6
worktree:
issue:
pr:
started: 2026-05-15T01:59:22Z
---

On a fresh Rust environment, `make lint` fails because the active toolchain does not have `cargo-clippy` installed. The setup flow should bootstrap the required component or fail with a clearer guided path before the lint target runs.

Observed failure:

```text
cargo clippy --all-targets --all-features -- -D warnings
error: 'cargo-clippy' is not installed for the toolchain '1.82.0-x86_64-unknown-linux-gnu'.
help: run `rustup component add clippy` to install it
make: *** [Makefile:13: lint] Error 1
```

## Acceptance criteria

**AC-1 -- Fresh-environment setup covers clippy before linting.**
Verified by: on a Rust toolchain without `cargo-clippy`, the documented setup or install path adds the component before `make lint` is expected to pass.

**AC-2 -- The repo gives a reproducible bootstrap path for contributors.**
Verified by: README, Makefile, or setup command documents or automates the required clippy installation step in a way a new contributor can run directly.
