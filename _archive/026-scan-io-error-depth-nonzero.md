---
id: "026"
title: "Raw IO error still surfaces when scan root exists but a sub-entry is inaccessible"
status: done
source: bug report
started: 2026-04-26T04:11:46Z
completed: 2026-04-26T04:30:34Z
verdict: PASSED
score: 0.8
worktree: 
issue:
pr: #14
mod-block: 
archived: 2026-04-26T04:30:34Z
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

## Stage Report: design

- DONE: Problem statement names the exact code path in discover_workflows that still propagates NotFound at depth > 0.
  `src/discovery.rs` `discover_workflows` lines 83-89: the guard `io_err.kind() == io::ErrorKind::NotFound && depth == 0` (line 86) passes depth-0 NotFound to `Ok(vec![])` but lets any depth >= 1 NotFound fall through to `return Err(DiscoveryError::Io(io_err))` on line 89, which then propagates as the raw `"discovery IO error: No such file or directory (os error 2)"` chain seen in the reproduction.
- DONE: Acceptance criteria distinguish skippable NotFound entries from genuine hard errors (e.g. PermissionDenied on root).
  AC-1 defines skippable: NotFound at any depth (broken symlink, concurrently deleted sub-entry) → skip and continue walk, return Ok. AC-2 defines hard: PermissionDenied on root → still return Err. AC-3 requires no raw `os error` string surfaces to the user in any skipped scenario.

### Summary

The entity file already contained a complete problem statement, reproduction steps, root cause analysis, fix direction, and three acceptance criteria; no additional design content was needed. Confirmed the exact code path: `discover_workflows` in `src/discovery.rs` lines 83-89, where the `depth == 0` guard leaves depth >= 1 NotFound errors unhandled and propagates them as `DiscoveryError::Io`. The acceptance criteria cleanly separate skippable errors (NotFound at any depth) from hard errors (PermissionDenied on root), providing unambiguous guidance for the implement stage.

## Implementation Plan

### Fix: `src/discovery.rs` — `discover_workflows`, lines 83-89

Replace the current error-handling block inside the walker's `Err(err)` arm. The existing code is:

```rust
let depth = err.depth();
if let Some(io_err) = err.into_io_error() {
    if io_err.kind() == io::ErrorKind::NotFound && depth == 0 {
        return Ok(vec![]);
    }
    return Err(DiscoveryError::Io(io_err));
}
continue;
```

New logic:

```rust
let depth = err.depth();
if let Some(io_err) = err.into_io_error() {
    match io_err.kind() {
        // Root path does not exist → treat as empty (routes to ZeroWorkflows branch).
        io::ErrorKind::NotFound if depth == 0 => return Ok(vec![]),
        // Sub-entry disappeared mid-walk (broken symlink, concurrent delete) → skip.
        io::ErrorKind::NotFound => continue,
        // Hard error on root (e.g. PermissionDenied) → surface as error.
        _ if depth == 0 => return Err(DiscoveryError::Io(io_err)),
        // Hard error on sub-entry → surface as error (genuine IO failure).
        _ => return Err(DiscoveryError::Io(io_err)),
    }
}
continue;
```

The two `_` arms collapse to the same action and can be written as a single `_ =>` arm. Written out cleanly:

```rust
let depth = err.depth();
if let Some(io_err) = err.into_io_error() {
    if io_err.kind() == io::ErrorKind::NotFound && depth == 0 {
        return Ok(vec![]);
    }
    if io_err.kind() == io::ErrorKind::NotFound {
        // Broken symlink or concurrently deleted sub-entry — skip, keep walking.
        continue;
    }
    return Err(DiscoveryError::Io(io_err));
}
continue;
```

This is a minimal, surgical change: one new `if` block added before the existing `return Err(...)`. No other logic changes.

### Tests to add: `src/discovery.rs` `#[cfg(test)]` block

**AC-1 test — broken symlink at depth >= 1 (Unix only)**

```rust
#[cfg(unix)]
#[test]
fn broken_symlink_in_subtree_is_skipped_not_fatal() {
    use std::os::unix::fs::symlink;
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    // A real workflow so we can confirm the walk completes and finds it.
    write_workflow_readme(&root.join("docs/real"), "Real");
    // Broken symlink: target does not exist.
    symlink(root.join("nonexistent-target"), root.join("docs/broken-link")).unwrap();

    let result = discover_workflows(root);
    assert!(result.is_ok(), "broken symlink must not be fatal, got {result:?}");
    let found = result.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title.as_deref(), Some("Real"));
}
```

**AC-2 test — PermissionDenied on scan root (Unix only)**

```rust
#[cfg(unix)]
#[test]
fn permission_denied_on_root_surfaces_as_error() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("locked");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();

    let result = discover_workflows(&root);
    // Restore permissions before asserting so tempdir cleanup succeeds.
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err(), "PermissionDenied on root must return Err, got {result:?}");
}
```

Note: The chmod 000 test must run as a non-root user to exercise the permission denial. If run as root, the OS bypasses the permission check and the test would be vacuous. Standard CI (GitHub Actions, non-root user) is fine. Add a `#[cfg(unix)]` guard; skip on Windows.

### Verification commands

```
# unit tests (covers AC-1 and AC-2)
cargo test -p spacetop discovery -- --nocapture

# full test suite to confirm no regressions
cargo test -p spacetop
```

### File ownership

Only `src/discovery.rs` needs to change. No other files, crates, or modules are affected. No dependency additions required.

### Module/crate notes

- `DiscoveryError::Io` and its `thiserror` derive remain unchanged.
- The `filter_entry` predicate (`is_pruned`) is unchanged — symlinks at the root of a pruned dir name are already excluded before they can generate errors.
- AC-3 (no raw `os error` in user-facing output) is satisfied automatically: if the error is never returned from `discover_workflows`, it never reaches the `anyhow` chain in `lib.rs` that formats the user-facing message.

## Stage Report: plan

- DONE: Plan specifies the exact change to discover_workflows in src/discovery.rs: skip NotFound at depth >= 1, surface PermissionDenied on root as hard error.
  Implementation Plan section above gives the exact before/after for lines 83-89 and explains the logic: add one new `if io_err.kind() == NotFound { continue; }` guard before the existing `return Err(...)`.
- DONE: Plan includes unit test commands for AC-1 (broken symlink fixture) and AC-2 (chmod 000, Unix-only).
  AC-1 test `broken_symlink_in_subtree_is_skipped_not_fatal` and AC-2 test `permission_denied_on_root_surfaces_as_error` are fully spelled out with code; verification command is `cargo test -p spacetop discovery -- --nocapture`.

### Summary

The plan makes a single minimal change to `src/discovery.rs` lines 83-89: add a `continue` branch for `NotFound` at depth >= 1, leaving depth-0 `NotFound` returning `Ok(vec![])` and all other IO errors (including `PermissionDenied` on root) returning `Err` as before. Two new `#[cfg(unix)]` tests cover both acceptance criteria directly, and AC-3 is satisfied implicitly because the skipped error never reaches the user-facing anyhow chain.

## Stage Report: implement

- DONE: discover_workflows in src/discovery.rs skips NotFound at depth >= 1 and continues the walk.
  Added `if io_err.kind() == io::ErrorKind::NotFound { continue; }` before the existing `return Err(...)` in the walker error arm; commit 0337e0b.
- DONE: NotFound at depth == 0 still returns Ok(vec![]) (existing 021 behaviour preserved).
  The depth-0 guard (`if io_err.kind() == NotFound && depth == 0 { return Ok(vec![]); }`) is unchanged; `nonexistent_root_returns_empty_not_error` test still passes.
- DONE: cargo test passes including new tests for broken-symlink AC-1 and PermissionDenied AC-2; full suite passes.
  151/151 tests pass including `broken_symlink_in_subtree_is_skipped_not_fatal` (AC-1) and `permission_denied_on_root_surfaces_as_error` (AC-2).

### Summary

Applied the planned minimal fix to `src/discovery.rs`: one new `if` block continuing on `NotFound` at depth >= 1, leaving the depth-0 empty-return and all other IO errors unchanged. Added two `#[cfg(unix)]` tests covering both acceptance criteria. Full suite (151 tests) passes. AC-3 is satisfied implicitly — the skipped error never reaches the anyhow chain that formats user-facing output.

## Stage Report: review

- DONE: NotFound at depth >= 1 is skipped (continue), not propagated — the 021 depth-0 path is preserved.
  Lines 86-93 of src/discovery.rs: depth-0 guard (`NotFound && depth == 0 → Ok(vec![])`) unchanged; new `if io_err.kind() == NotFound { continue; }` at lines 91-93 catches all remaining NotFound at any depth.
- DONE: PermissionDenied on root still surfaces as Err — hard errors are not swallowed.
  After both NotFound guards, `return Err(DiscoveryError::Io(io_err))` handles all other IO errors; `permission_denied_on_root_surfaces_as_error` test (chmod 000 dir) asserts `result.is_err()` and passes.
- DONE: New tests broken_symlink_in_subtree_is_skipped_not_fatal and permission_denied_on_root_surfaces_as_error pass; full suite passes.
  151/151 tests pass (cargo test output: "test result: ok. 151 passed; 0 failed"); both named tests appear as `ok`.

### Summary

The implementation is correct and minimal: four production lines added to `src/discovery.rs` plus two `#[cfg(unix)]` test functions. All three checklist items are verified. The fix satisfies AC-3 implicitly — `NotFound` at depth >= 1 issues `continue` before it can reach `DiscoveryError::Io`, so the raw OS errno string never appears in user-facing output. Approved.
