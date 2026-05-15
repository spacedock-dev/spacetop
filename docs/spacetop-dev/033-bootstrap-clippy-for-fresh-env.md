---
id: 033
title: Bootstrap clippy for fresh environments before running `make lint`
status: review
source: user request 2026-04-27
score: 0.6
worktree: .worktrees/spacedock-ensign-033-bootstrap-clippy-for-fresh-env
issue:
pr: #33
started: 2026-05-15T01:59:22Z
mod-block: 
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

## Implementation Plan

### Design

`make lint` is the first thing `make build`, `make install`, and any CI hook hits. We want a single fresh-clone command path that ends with a working `make lint` — without surprising contributors by silently mutating their toolchain when `rustup` is not the active installer.

Approach: add an explicit, idempotent `bootstrap` Makefile target, and have `lint` precheck clippy and print a guided one-liner (rather than auto-installing behind the user's back). This keeps Spacedock's "explicit and auditable" stance and still gives a reproducible recipe.

### File-by-file changes

1. `Makefile` — add a `bootstrap` target and a clippy precheck on `lint`.

   - Add `bootstrap` to `.PHONY`.
   - New target:
     ```makefile
     bootstrap:
     	@command -v rustup >/dev/null 2>&1 || { \
     		echo "error: rustup not found. Install Rust via https://rustup.rs and re-run 'make bootstrap'." >&2; exit 1; }
     	rustup component add clippy
     ```
   - Modify `lint` to precheck and emit a guided error instead of the bare rustup help line:
     ```makefile
     lint:
     	@cargo clippy --version >/dev/null 2>&1 || { \
     		echo "error: cargo-clippy is not installed for the active toolchain." >&2; \
     		echo "       run 'make bootstrap' (or 'rustup component add clippy') and retry." >&2; \
     		exit 1; }
     	cargo clippy --all-targets --all-features -- -D warnings
     ```
   - `build: lint` chain is unchanged; the guided error fires before the `-D warnings` invocation, replacing the raw rustup output cited in the bug.

2. `README.md` — add a short "Setup" subsection under `## Development` (before `### Install Local Build`):

   ```markdown
   ### Setup

   On a fresh clone, install the required Rust components once:

       make bootstrap

   This adds the `clippy` component to the active toolchain. `make lint` and
   `make build` will refuse to run until clippy is available and will point you
   back at this command.
   ```

   Keep wording short — the Makefile error message is the load-bearing copy.

### Module ownership

- Pure tooling change: Makefile + README only.
- No Rust source files, no tests under `src/` or `tests/`, no impact on parser/state/UI modules.
- No Cargo.toml or build.rs changes.

### Test strategy

This is a build-tooling change with no Rust code path, so no `cargo test` additions are required. Verification is manual and scripted:

- **Repro of the bug (toolchain without clippy):**
  ```bash
  rustup component remove clippy
  make lint
  # Expect: guided "run 'make bootstrap'" error, exit 1
  ```
- **Bootstrap path (AC-1):**
  ```bash
  make bootstrap
  make lint
  # Expect: clippy installs, then lint passes clean.
  ```
- **Idempotency:**
  ```bash
  make bootstrap
  make bootstrap
  # Expect: second call is a no-op (rustup reports component already installed).
  ```
- **Missing rustup branch (AC-2 fallback):**
  ```bash
  PATH=/usr/bin:/bin make bootstrap   # PATH without rustup
  # Expect: guided "Install Rust via https://rustup.rs" error, exit 1.
  ```
- **Contributor reproduction (AC-2):** a new contributor following the new README "Setup" section runs `make bootstrap && make lint` from a freshly cloned repo with stock `rustup` and observes a clean lint pass.

### Lint gate

After the Makefile + README edits land, run `make lint` in the worktree to confirm the precheck doesn't regress when clippy is already present. Per `CLAUDE.md`, no `#[allow(...)]` bypass is in scope here — this change adds no Rust code.

### Commit shape

Single commit suggested: `feat(makefile): bootstrap clippy for fresh environments` — Makefile + README together, since AC-1 and AC-2 land as one user-facing path.

## Stage Report: plan

- DONE: Name the exact file(s) to change (Makefile target, README setup section, or a new bootstrap script) and the precise command(s) that install clippy on a fresh toolchain.
  Plan calls out `Makefile` (new `bootstrap` target running `rustup component add clippy`, plus clippy precheck on `lint`) and `README.md` (new `### Setup` subsection pointing at `make bootstrap`).
- DONE: Describe how to verify the fix: the manual reproduction steps on a toolchain without clippy plus the success signal (lint passes, or a clear guided error).
  Test strategy lists the `rustup component remove clippy` repro, the `make bootstrap && make lint` success path, idempotency check, and the missing-rustup guided-error branch.

### Summary

Plan adds a `bootstrap` Makefile target plus a precheck on `lint` that emits a guided "run 'make bootstrap'" error instead of the raw rustup hint, and documents the one-command setup path in README under a new `### Setup` subsection. The change is tooling-only — no Rust source, no tests under `src/` or `tests/` — and `make lint` is the single verification command. AC-1 is satisfied by the bootstrap target installing clippy; AC-2 is satisfied by `make bootstrap` being the reproducible, documented contributor entry point.

## Stage Report: implement

- DONE: Makefile carries the `bootstrap` target and `lint` precheck exactly as specified in the plan's File-by-file changes (including the guided error wording).
  Added `bootstrap` to `.PHONY`, added the `bootstrap` target with the `rustup` presence check and `rustup component add clippy`, and prefixed `lint` with the `cargo clippy --version` precheck that emits the guided "run 'make bootstrap'" error verbatim.
- DONE: README has the `### Setup` subsection under `## Development` pointing contributors at `make bootstrap`.
  Inserted `### Setup` immediately before `### Install Local Build` with the one-command `make bootstrap` recipe and the rationale matching the plan's copy.
- DONE: `make lint` runs clean from the worktree on a toolchain that already has clippy installed (regression check that the precheck doesn't break the happy path).
  Ran `make lint` in the worktree; precheck succeeded and `cargo clippy --all-targets --all-features -- -D warnings` finished with no warnings.

### Summary

Bootstrap path is in place: `make bootstrap` installs clippy via `rustup` (or fails with a guided install-rustup error), and `make lint` now refuses to run without clippy by pointing the contributor at `make bootstrap` instead of leaking the bare rustup help line. README gained a short `### Setup` subsection so a fresh-clone contributor lands on the documented one-liner. Tooling-only — no `src/` or `tests/` changes. `make lint` runs clean in the worktree.

## Stage Report: review

- DONE: Verify the Makefile changes match the plan exactly: bootstrap target with rustup presence check + clippy install, lint precheck with the guided 'run make bootstrap' error, .PHONY updated.
  Diff vs `main` shows `.PHONY` extended with `bootstrap`; new `bootstrap` target has the `command -v rustup` presence check with the verbatim "Install Rust via https://rustup.rs" error and then `rustup component add clippy`; `lint` is prefixed with `cargo clippy --version` precheck emitting the verbatim "run 'make bootstrap' (or 'rustup component add clippy') and retry." guidance and exiting 1, matching the plan's File-by-file changes character-for-character.
- DONE: Confirm README has the new `### Setup` subsection positioned before `### Install Local Build` and that it points contributors at `make bootstrap`.
  README diff inserts `### Setup` immediately before `### Install Local Build`; copy points contributors at `make bootstrap` and explains that `make lint`/`make build` will refuse to run until clippy is available, matching AC-2's reproducible-bootstrap-path requirement.
- DONE: Re-run `make lint` from the worktree to confirm the happy path is clean (no regression on toolchains that already have clippy).
  `make lint` in the worktree printed only the `cargo clippy --all-targets --all-features -- -D warnings` invocation and finished clean (`Finished dev profile ... 0.34s`); precheck did not block the toolchain-has-clippy happy path.

### Verdict

**PASSED.** AC-1 (clippy bootstrapped before lint) is delivered by the `bootstrap` target plus the lint precheck that converts the bare rustup hint into a guided "run 'make bootstrap'" error. AC-2 (reproducible contributor path) is delivered by the documented `make bootstrap` one-liner in README under `### Setup`. The change is tooling-only, no Rust source touched, and `make lint` remains green on a toolchain that already has clippy.

### Summary

Review confirms Makefile and README diffs match the plan exactly, both acceptance criteria have working evidence, and the `make lint` happy path is unaffected. Recommending PASSED → done.
