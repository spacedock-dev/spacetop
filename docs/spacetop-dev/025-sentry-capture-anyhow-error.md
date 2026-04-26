---
id: "025"
title: "Sentry does not capture anyhow errors returned from main — only panics are reported"
status: implement
source: bug report
started: 2026-04-26T04:11:46Z
completed:
verdict:
score: 0.9
worktree: .worktrees/spacedock-ensign-025-sentry-capture-anyhow-error
issue:
pr:
---

The Sentry integration initialised in task 024 uses the `panic` feature, which hooks `std::panic::set_hook`. When `spacetop::run()` returns an `anyhow::Err` — such as a scan IO error or a discovery failure — that error propagates through `main()` and is printed to stderr by Rust's default `Result` handler. It is never sent to Sentry.

This is why the user saw the "failed to scan" error in the terminal but no corresponding event appeared in the Sentry dashboard.

## Root cause

`src/main.rs` calls `spacetop::run(cli)` and returns its result directly. There is no explicit `sentry::capture_error()` call before returning, so non-panic failures are invisible to Sentry.

## Fix

Capture the error explicitly before returning from `main()`:

```rust
let result = spacetop::run(cli);
if let Err(ref e) = result {
    sentry::capture_error(e.as_ref() as &dyn std::error::Error);
}
result
```

The capture should be gated by the same `_sentry` guard being `Some` (i.e., only when Sentry was actually initialised) to avoid a no-op call in debug builds.

## Acceptance criteria

**AC-1 -- anyhow::Err returned from run() is sent to Sentry in release builds.**
When `spacetop::run()` returns an `Err`, `sentry::capture_error()` is called before `main()` returns. Verified by: unit test using a mock/test transport that records captured events, asserting one event is recorded when `run()` fails.

**AC-2 -- No Sentry call is made in debug builds or when DSN is unset.**
The capture path is guarded by `_sentry.is_some()`. Verified by: existing `dev_build_does_not_init_sentry` test continues to pass; no transport calls in tests that omit the DSN.

**AC-3 -- Successful runs produce no spurious Sentry events.**
When `run()` returns `Ok(())`, no event is captured. Verified by: unit test asserting zero events on success path.

## Stage Report: design

- DONE: Problem statement names the exact line in main.rs where capture_error() must be inserted and how it is guarded by _sentry.is_some().
  Line 28 of `src/main.rs` — `spacetop::run(cli)` — is the insertion point. The fix replaces the bare call with a result binding, calls `sentry::capture_error()` only when `_sentry.is_some()`, then returns the result.
- DONE: Acceptance criteria are verifiable without a live Sentry connection (mock transport approach is confirmed).
  AC-1 specifies a mock/test transport that records captured events; AC-2 and AC-3 verify zero-event paths, all runnable with `cargo test` without a real DSN.

### Summary

The entity file already contained a well-formed problem statement, root cause, and acceptance criteria. This design stage confirmed that line 28 of `src/main.rs` is the precise insertion point, that the `_sentry.is_some()` guard is already specified in the fix description, and that the three acceptance criteria are fully verifiable via a mock Sentry transport in unit tests — no live Sentry connection is required.

## Implementation Plan

### Step 1 — Enable the `test` feature for sentry in dev-dependencies

In `Cargo.toml`, add a separate sentry dev-dependency entry with the `test` feature so `sentry::test::with_captured_events` and `sentry::test::with_captured_events_options` are available in `#[cfg(test)]` blocks:

```toml
[dev-dependencies]
sentry = { version = "0.34", default-features = false, features = ["test"] }
tempfile = "3"
```

The `test` feature enables `sentry-core/test`, which provides the in-memory capture helpers without spinning up a real HTTP transport.

### Step 2 — Replace the bare `run()` call in `src/main.rs`

Replace line 28:

```rust
spacetop::run(cli)
```

with:

```rust
let result = spacetop::run(cli);
if let Err(ref e) = result {
    if _sentry.is_some() {
        sentry::capture_error(e.as_ref() as &dyn std::error::Error);
    }
}
result
```

This satisfies:
- AC-1: `capture_error` is called before `main()` returns when `run()` fails.
- AC-2: The call is guarded by `_sentry.is_some()`, so it is never reached when `_sentry` is `None` (debug builds or missing DSN). The existing `dev_build_does_not_init_sentry` test remains unmodified.
- AC-3: The `if let Err` guard means no event is captured on `Ok(())`.

### Step 3 — Add unit tests in `src/main.rs`

Add three tests inside the existing `#[cfg(test)] mod tests` block.

Because the production init path is guarded by `cfg!(debug_assertions)` (which is `true` in test builds), the production guard cannot be exercised directly in unit tests. The tests instead use `sentry::test::with_captured_events` / `with_captured_events_options` to construct a temporary hub with an in-memory transport, call the capture logic directly, and assert on the number of captured events.

```rust
#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    #[test]
    #[cfg(debug_assertions)]
    fn dev_build_does_not_init_sentry() {
        assert!(cfg!(debug_assertions));
    }

    // AC-1: an error from run() is forwarded to Sentry.
    #[test]
    fn capture_error_on_run_failure() {
        let events = sentry::test::with_captured_events(|| {
            let result: anyhow::Result<()> = Err(anyhow!("scan IO error"));
            if let Err(ref e) = result {
                sentry::capture_error(e.as_ref() as &dyn std::error::Error);
            }
        });
        assert_eq!(events.len(), 1, "expected exactly one captured event");
    }

    // AC-3: a successful run produces no Sentry events.
    #[test]
    fn no_capture_on_run_success() {
        let events = sentry::test::with_captured_events(|| {
            let result: anyhow::Result<()> = Ok(());
            if let Err(ref e) = result {
                sentry::capture_error(e.as_ref() as &dyn std::error::Error);
            }
        });
        assert_eq!(events.len(), 0, "expected no events on success");
    }

    // AC-2: when _sentry is None the capture branch is never entered.
    #[test]
    fn no_capture_when_sentry_not_initialised() {
        // Simulate _sentry = None (no guard alive, no hub active).
        // with_captured_events installs a temporary hub; we deliberately do NOT
        // call capture_error because the guard check prevents it.
        let events = sentry::test::with_captured_events(|| {
            let _sentry: Option<sentry::ClientInitGuard> = None;
            let result: anyhow::Result<()> = Err(anyhow!("error that must not be sent"));
            if let Err(ref e) = result {
                if _sentry.is_some() {
                    sentry::capture_error(e.as_ref() as &dyn std::error::Error);
                }
            }
        });
        assert_eq!(events.len(), 0, "expected no events when sentry is not initialised");
    }
}
```

### Verification commands

```
# Run all tests (debug build — existing guard test passes, new tests exercise capture logic)
cargo test

# Run only the new tests by name
cargo test capture_error_on_run_failure
cargo test no_capture_on_run_success
cargo test no_capture_when_sentry_not_initialised

# Confirm the existing guard test still passes
cargo test dev_build_does_not_init_sentry
```

Expected outcome: all four tests in `src/main.rs` pass; `cargo test` exits 0.

### Files touched

| File | Change |
|------|--------|
| `src/main.rs` | Replace bare `spacetop::run(cli)` with result-binding + capture block; extend `mod tests` with three new tests |
| `Cargo.toml` | Add `sentry` dev-dependency entry with `features = ["test"]` |

No other files need to change.

## Stage Report: plan

- DONE: Plan specifies the exact code change in src/main.rs replacing the bare run(cli) call with a result-binding + capture_error block.
  Implementation Plan Step 2 above shows the verbatim replacement and explains which AC each line satisfies.
- DONE: Plan describes the mock transport test setup for AC-1, AC-2, and AC-3 with concrete cargo test commands.
  Implementation Plan Step 3 lists three tests using `sentry::test::with_captured_events`; verification commands section gives exact `cargo test` invocations for each AC.

### Summary

The plan requires two file changes: `Cargo.toml` gains a sentry dev-dependency with the `test` feature to unlock `sentry::test::with_captured_events`, and `src/main.rs` has line 28 replaced with a result-binding block that calls `capture_error` only when `_sentry.is_some()`. Three unit tests cover AC-1 (error captured), AC-2 (no capture when guard is None), and AC-3 (no capture on success), all runnable with `cargo test` without a live Sentry DSN.
