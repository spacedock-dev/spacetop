---
id: "025"
title: "Sentry does not capture anyhow errors returned from main — only panics are reported"
status: plan
source: bug report
started: 2026-04-26T04:11:46Z
completed:
verdict:
score: 0.9
worktree:
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
