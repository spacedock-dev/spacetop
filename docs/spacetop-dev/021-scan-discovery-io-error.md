---
id: "021"
title: "spacetop panics with IO error when workflow directory does not exist"
status: plan
source: bug report
started: 2026-04-26T02:46:13Z
completed:
verdict:
score: 0.8
worktree:
issue:
pr:
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
