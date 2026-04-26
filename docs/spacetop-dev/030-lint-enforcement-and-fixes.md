---
id: "030"
title: "Add lint enforcement to CLAUDE.md/AGENTS.md and fix all clippy errors"
status: implement
source: feature request
started: 2026-04-26T07:07:02Z
completed:
verdict:
score: 0.9
worktree: .worktrees/spacedock-ensign-030-lint-enforcement-and-fixes
issue:
pr:
---

`make lint` now runs `cargo clippy --all-targets --all-features -- -D warnings` (enforced via the Makefile). Agents are not currently instructed to run it before completing a task, causing lint regressions to accumulate undetected (16 errors exist today). This task adds the lint enforcement rule to both AGENTS.md and CLAUDE.md and fixes every current clippy error.

## Current lint errors (16)

**src/domain/mod.rs — 14 × `clippy::excessive_precision`**
The oklch-to-sRGB conversion constants have more decimal digits than an f64 can represent. Clippy truncates them at compile time but flags it as excessive precision.

**src/ui/mod.rs — 1 × `clippy::collapsible_match`**
A nested `if let` inside a `match` arm can be collapsed into the outer match.

**src/ui/mod.rs (tests) — 1 × `clippy::useless_vec`**
A `vec![...]` literal is used where a slice `[...]` suffices.

## Changes required

### 1. AGENTS.md — add lint enforcement section

Append to the existing AGENTS.md:

```markdown
## Lint Gate

Before marking any task complete, run:

    make lint

This runs `cargo clippy --all-targets --all-features -- -D warnings`. All warnings are errors. Fix every clippy diagnostic before committing the stage report.
```

### 2. CLAUDE.md — create with lint enforcement

Create CLAUDE.md at the repo root with project context and the same lint gate rule:

```markdown
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
```

### 3. Fix all 16 clippy errors

- Truncate the 14 excessive-precision float literals in `src/domain/mod.rs`
- Collapse the nested `if let` in `src/ui/mod.rs`
- Replace the `vec![...]` with a slice in the test

## Acceptance criteria

**AC-1 -- make lint exits 0.**
`make lint` completes with no errors or warnings.
Verified by: `make lint 2>&1; echo "exit: $?"`

**AC-2 -- AGENTS.md contains the lint gate rule.**
`grep -A3 "Lint Gate" AGENTS.md` shows the `make lint` instruction.

**AC-3 -- CLAUDE.md exists and contains the lint gate rule.**
`grep -A3 "Lint Gate" CLAUDE.md` shows the `make lint` instruction.

**AC-4 -- All tests still pass.**
`cargo test` 174/174 (or current count) green.
