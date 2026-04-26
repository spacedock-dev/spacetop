---
id: "026"
title: "Raw IO error still surfaces when scan root exists but a sub-entry is inaccessible"
status: design
source: bug report
started:
completed:
verdict:
score: 0.8
worktree:
issue:
pr:
---

Task 021 fixed the case where the scan root itself does not exist (WalkDir `NotFound` at depth 0). However the user still sees a raw `(os error 2)` when running spacetop from a directory that exists but contains an inaccessible sub-entry — for example a broken symlink, a deleted directory mid-walk, or a path that becomes unreachable during traversal.

## Reproduction

```
Error: failed to scan /Users/kent/Dev/InfuseAI/GitHub

Caused by:
    0: discovery IO error: No such file or directory (os error 2)
    1: No such file or directory (os error 2)
```

`/Users/kent/Dev/InfuseAI/GitHub` exists. The error originates from a sub-entry at depth > 0 — the 021 depth-0 guard does not intercept it, so it propagates as a raw `DiscoveryError::Io` → anyhow chain.

## Root cause

The 021 fix in `src/discovery.rs` `discover_workflows` returns `Ok(vec![])` only when `kind == NotFound && depth == 0`. A `NotFound` at depth ≥ 1 (broken symlink, concurrently deleted directory) still becomes `DiscoveryError::Io` and leaks the raw OS errno string.

## Fix direction

In `discover_workflows`, treat `NotFound` at any depth as a skippable entry rather than a fatal error — log or silently skip the offending path and continue the walk. Only errors that prevent the walk from continuing at all (e.g. permission denied on the root, or genuine IO failures unrelated to missing entries) should surface as hard errors.

Alternatively, use WalkDir's `.skip_current_dir()` on `NotFound` errors and continue iteration.

## Acceptance criteria

**AC-1 -- NotFound errors at depth > 0 are skipped, not fatal.**
Running spacetop from a directory containing a broken symlink or a concurrently deleted subdirectory completes the scan and shows available workflows rather than exiting with a raw error.
Verified by: unit test that creates a tempdir with a broken symlink and asserts `discover_workflows` returns `Ok`.

**AC-2 -- Genuine hard IO errors (e.g. permission denied on root) still surface.**
`PermissionDenied` on the scan root is still returned as `Err`, not silently swallowed.
Verified by: unit test using a chmod 000 directory (Unix only, skipped on Windows) asserting `Err` is returned.

**AC-3 -- No raw os error string in user-facing output for any skipped-entry scenario.**
Verified by: end-to-end test or manual check that stderr contains no `os error` substring when a sub-entry is missing.
