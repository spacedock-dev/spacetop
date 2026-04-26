---
id: "021"
title: "spacetop panics with IO error when workflow directory does not exist"
status: review
source: bug report
started: 2026-04-26T02:46:13Z
completed:
verdict:
score: 0.8
worktree: .worktrees/spacedock-ensign-021-scan-discovery-io-error
issue:
pr: #11
mod-block: merge:pr-merge
---

`spacetop` crashes with an unhelpful discovery IO error when run from a directory where the expected workflow path does not exist on disk. The user sees a raw OS error (2: No such file or directory) rather than a clear message about what path was scanned and what to do next.

## Error log

```
➜  recce-gtm git:(main) ✗ ~/.cargo/bin/spacetop
Error: failed to scan /Users/clkao/git/recce-gtm

Caused by:
    0: discovery IO error: No such file or directory (os error 2)
    1: No such file or directory (os error 2)
```

## Context

- `spacetop` is invoked with no arguments from `/Users/clkao/git/recce-gtm`.
- The error occurs during the workflow-directory discovery scan, before any UI is rendered.
- OS error 2 means the target path does not exist, but the message does not say which sub-path triggered the failure.

## Acceptance criteria

**AC-1 -- Graceful message when no workflow directory is found.**
When `spacetop` is run from a directory with no detectable workflow, it exits with a clear human-readable message (e.g. "No Spacedock workflow found in `<path>`. Run `spacedock commission` to create one.") and a non-zero exit code, rather than an unformatted `anyhow` chain.
Verified by: running `spacetop` from an empty temp directory and checking stderr output and exit code.

**AC-2 -- No raw OS error numbers in user-facing output.**
The final user-visible message must not contain `os error 2` or any raw errno string.
Verified by: grep on stderr output of the test scenario above.

## Problem statement

### Exact call path

`main()` calls `spacetop::run(cli)` → `decide_app(&cli, &cwd)` → `discovery::resolve_scan_root(cwd)` (walks up to the git root, falls back to cwd) → `discovery::discover_workflows(&scan_root)` (`src/discovery.rs`).

Inside `discover_workflows`, `WalkDir::new(root).follow_links(true).into_iter()` immediately yields an `Err` for the root path when that path does not exist on disk. The error handler at `discovery.rs:82-83` checks for symlink-cycle errors and passes everything else to `io_err = err.into_io_error()`, returning `Err(DiscoveryError::Io(io_err))`.

Back in `lib.rs:80-81`, the `?` operator propagates this through `.with_context(|| format!("failed to scan {}", scan_root.display()))`, producing the two-level anyhow chain the user sees.

### Why the raw errno leaks

`DiscoveryError::Io` is declared as `#[error("discovery IO error: {0}")]` with `#[from] io::Error`. The `Display` of `std::io::Error` always appends `(os error N)`. Because `run()` propagates via `anyhow::Result<()>` back to `main() -> anyhow::Result<()>`, the default anyhow reporter prints the full chain including that raw suffix. No filtering or wrapping intercepts it.

### Why the graceful path is never reached

`lib.rs:84` returns `Ok(DecideOutcome::ZeroWorkflows { scan_root })` only when `discover_workflows` returns `Ok(vec![])`. When the scan root does not exist, `discover_workflows` returns `Err(...)` instead, so the zero-workflows branch is bypassed entirely and the error propagates to the anyhow reporter.

### Fix target

In `discover_workflows` (`src/discovery.rs`), detect when the WalkDir root-entry error is `ErrorKind::NotFound` and return `Ok(vec![])` instead of propagating the IO error. This routes the nonexistent-directory case through the existing `ZeroWorkflows` branch in `lib.rs`, which already emits a clean message (added in a prior PR) without any OS errno.

## Stage Report: design

- DONE: Problem statement names the exact call path that fails and why the error leaks raw OS errno.
  Call path traced: `main` → `run` → `decide_app` → `discover_workflows` (WalkDir ENOENT on root) → `DiscoveryError::Io` → anyhow chain with raw `(os error 2)` suffix. See "Problem statement" section above.
- DONE: Acceptance criteria are verifiable with a concrete terminal command.
  AC-1: `mkdir /tmp/empty-test && cd /tmp/empty-test && spacetop`; check stderr for human-readable message and `echo $?` for non-zero. AC-2: pipe stderr through `grep "os error"`; expect no match.

### Summary

The bug is caused by `discover_workflows` propagating an ENOENT `io::Error` from WalkDir when the scan root does not exist, bypassing the `ZeroWorkflows` graceful branch and surfacing a raw anyhow error chain to the user. The fix belongs in `src/discovery.rs`: treat `ErrorKind::NotFound` on the root-entry error as an empty result rather than a hard failure. No new acceptance criteria were needed; the two existing ACs are concretely verifiable with a temp-directory test.

## Implementation Plan

### File and function to change

**File:** `src/discovery.rs`
**Function:** `discover_workflows` (line 62)

### Step-by-step plan

**Step 1 — Add `io::ErrorKind` import (already imported via `use std::io`; no change needed).**

`std::io::ErrorKind` is available through the existing `use std::io;` import at line 3.

**Step 2 — Detect `ErrorKind::NotFound` on the WalkDir root-entry error and return `Ok(vec![])`.**

In the error arm of the `for entry in walker` loop (lines 73-87), after the symlink-cycle check and before the unconditional `Err(DiscoveryError::Io(...))` return, add a check: if the IO error kind is `NotFound` **and** the failing entry has depth 0 (meaning WalkDir failed to open the root itself), return `Ok(vec![])` instead.

The change to lines 81-84 of `src/discovery.rs`:

```rust
// Before (current code):
if let Some(io_err) = err.into_io_error() {
    return Err(DiscoveryError::Io(io_err));
}

// After:
if let Some(io_err) = err.into_io_error() {
    // Root path does not exist: treat as empty, not a hard error.
    // This routes through the ZeroWorkflows branch in lib.rs.
    if io_err.kind() == io::ErrorKind::NotFound && err.depth() == 0 {
        return Ok(vec![]);
    }
    return Err(DiscoveryError::Io(io_err));
}
```

Note: `err.depth()` is accessible on the `walkdir::Error` before calling `into_io_error()`, so the depth check must be extracted before the `into_io_error()` call consumes the error. The corrected sequence:

```rust
if err.loop_ancestor().is_some() {
    continue;
}
let depth = err.depth();
if let Some(io_err) = err.into_io_error() {
    if io_err.kind() == io::ErrorKind::NotFound && depth == 0 {
        return Ok(vec![]);
    }
    return Err(DiscoveryError::Io(io_err));
}
continue;
```

**Step 3 — Add a regression test in `src/discovery.rs`.**

In the `#[cfg(test)] mod tests` block, add:

```rust
#[test]
fn nonexistent_root_returns_empty_not_error() {
    let result = discover_workflows(Path::new("/tmp/spacetop-nonexistent-test-dir-xyzzy"));
    assert!(result.is_ok(), "expected Ok, got {result:?}");
    assert!(result.unwrap().is_empty());
}
```

This test directly covers AC-1 (graceful path) and AC-2 (no raw OS error) at the unit level.

**Step 4 — Verify with the AC commands.**

AC-1 verification (run from an empty temp dir, check for human-readable message and non-zero exit):
```
mkdir /tmp/spacetop-ac-test && cargo run --bin spacetop -- 2>&1; echo "exit: $?"
```
Expected: stderr contains `spacetop: no Spacedock workflows found under ...`; exit code is non-zero (1).

AC-2 verification (no raw errno string in output):
```
cargo run --bin spacetop -- 2>&1 | grep "os error" ; echo "grep exit: $?"
```
Run from an empty/no-workflow directory. Expected: grep finds nothing (exit 1 from grep means no match = pass).

**Step 5 — Run the full test suite to confirm no regressions.**

```
cargo test
```

All existing tests must pass; the new `nonexistent_root_returns_empty_not_error` test must also pass.

### Module/file ownership notes

- Only `src/discovery.rs` needs to change. No changes to `src/lib.rs`, frontmatter, UI, or other modules.
- The `ZeroWorkflows` path in `lib.rs:84` and `run()` in `lib.rs:115-122` already emits the correct human-readable message; no changes there.

## Stage Report: plan

- DONE: Step-by-step implementation plan identifies the exact file and function to change in src/discovery.rs.
  Function is `discover_workflows` at line 62 of `src/discovery.rs`. The plan gives the exact code change: extract `depth` before consuming the error, check `io_err.kind() == ErrorKind::NotFound && depth == 0`, return `Ok(vec![])`.
- DONE: Plan includes the verification command from AC-1 and AC-2 to confirm the fix works.
  AC-1: `mkdir /tmp/spacetop-ac-test && cargo run --bin spacetop -- 2>&1; echo "exit: $?"` — check human-readable message and non-zero exit. AC-2: pipe stderr through `grep "os error"` — expect no match.

### Summary

The plan targets a single three-line change in `discover_workflows` (`src/discovery.rs` lines 81-84): extract the WalkDir error depth before consuming it, then short-circuit with `Ok(vec![])` when the kind is `NotFound` and depth is 0. This routes the missing-directory case through the already-correct `ZeroWorkflows` branch in `lib.rs` without touching any other file. A new unit test `nonexistent_root_returns_empty_not_error` covers both ACs at the unit level, and the two AC terminal commands verify the end-to-end binary behavior.

## Stage Report: implement

- DONE: The three-line change in discover_workflows correctly intercepts ErrorKind::NotFound at depth 0 and returns Ok(vec![]).
  Implemented in `src/discovery.rs` lines 81-89: extract `depth` before `into_io_error()`, guard on `kind() == NotFound && depth == 0`. Commit a72766c.
- DONE: cargo test passes including the new unit test nonexistent_root_returns_empty_not_error.
  146/146 tests pass; `discovery::tests::nonexistent_root_returns_empty_not_error` passes.
- DONE: Running spacetop from a nonexistent directory prints a human-readable message with no raw os error string.
  AC-1: `spacetop` from empty `/tmp/spacetop-ac-test` prints `spacetop: no Spacedock workflows found under ... Pass --workflow-dir <path>` and exits 1. AC-2: `grep "os error"` returns exit 1 (no match).

### Summary

The fix is a minimal change in `src/discovery.rs`: before consuming the WalkDir error with `into_io_error()`, extract the entry depth, then return `Ok(vec![])` when kind is `NotFound` and depth is 0. This routes the nonexistent-directory case through the existing `ZeroWorkflows` graceful branch, eliminating the raw `(os error 2)` suffix from user-visible output. A new unit test and both AC terminal commands confirm the behavior. No other files were modified.
