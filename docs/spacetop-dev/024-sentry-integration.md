---
id: "024"
title: "Integrate Sentry for error reporting with dev/release environment mode"
status: design
source: feature request
started:
completed:
verdict:
score: 0.75
worktree:
issue:
pr:
---

Spacetop currently surfaces errors only through the terminal UI or process exit. Adding Sentry gives us automatic error capture in production releases while keeping dev sessions (cargo run / debug builds) free from noise.

Two concerns are coupled here: (1) wiring up the `sentry` crate and initialising it in `main`, and (2) suppressing or disabling telemetry in dev mode so engineers are not spamming the production project while iterating locally.

## DSN

The Sentry DSN for spacetop is:

```
https://6fcb5871f98e0c20535a6dace8c8f0c6@o1081482.ingest.us.sentry.io/4511284255916032
```

Sentry DSNs are client-side public keys (write-only, no read access), so it is safe to commit. The DSN belongs in `.env.example` so developers can copy it into their local `.env` and so the value is auditable in git. The build reads it at compile time via a `build.rs` that calls `println!("cargo:rustc-env=SENTRY_DSN=...")`, falling back to an empty string when the variable is absent, which disables the SDK.

## Environment mode

Use `cfg!(debug_assertions)` to detect dev mode at compile time:
- `cargo run` / `cargo build` (debug profile) → `debug_assertions` is **true** → set Sentry sample rate to 0.0 or skip init entirely.
- `cargo build --release` → `debug_assertions` is **false** → init Sentry with the baked-in DSN and a sample rate of 1.0.

This requires no runtime flag and no extra config file: the build profile already captures the intent.

## Acceptance criteria

**AC-1 -- Sentry initialises in release builds.**
Running a release binary that triggers an unhandled error sends an event to the Sentry project. Verified by: `cargo build --release`, trigger a known-bad state, check the Sentry dashboard for the event, or mock the transport in an integration test.

**AC-2 -- Sentry is silent in debug builds.**
Running `cargo run` does not initialise the Sentry client (sample rate 0.0 or guard short-circuits). Verified by: unit test asserting `is_dev_build()` returns true under `cfg(debug_assertions)`, or a compile-time constant check.

**AC-3 -- DSN is stored in `.env.example` and read via `build.rs`.**
`.env.example` contains the `SENTRY_DSN` key with the real DSN value. `build.rs` forwards the variable to the compiler via `cargo:rustc-env`. When `SENTRY_DSN` is unset, the binary compiles without error and Sentry is disabled.
Verified by: unset `SENTRY_DSN`, build succeeds, no Sentry events emitted.

**AC-4 -- Sentry release is tagged with the crate version.**
The Sentry client is initialised with `release: env!("CARGO_PKG_VERSION")` so issues can be grouped by release in the dashboard.
Verified by: inspect the event payload or integration test mock.
