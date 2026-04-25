---
id: "018"
title: Upgrade ratatui to 0.30+ to unblock ratskin adoption
status: implement
source: captain (unblocks 017 ratskin path)
started: 2026-04-25T16:03:12Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-018-upgrade-ratatui-0-30
issue:
pr:
---

Task 017 surveyed termimad as a markdown renderer and found that `ratskin` 0.3.1 — a crate that bridges termimad's `MadSkin` styling to Ratatui `Vec<Line>` — is the right integration path. The blocker is that `ratskin` requires `ratatui ^0.30.0` and SpaceTop currently uses `0.29`. This task upgrades ratatui and all affected dependencies so that task 019 (ratskin adoption) can proceed.

## Acceptance criteria

**AC-1 -- ratatui is upgraded to 0.30 or later in Cargo.toml.**
Verified by: `grep ratatui Cargo.toml` shows version ≥ 0.30.

**AC-2 -- The project compiles cleanly after the upgrade.**
Verified by: `cargo check` exits 0 with no errors.

**AC-3 -- All existing tests pass after the upgrade.**
Verified by: `cargo test --lib` exits 0 with no failures.

**AC-4 -- Any breaking API changes from the ratatui 0.29 → 0.30 migration are resolved.**
Verified by: `cargo check` and `cargo test --lib` both clean; stage report notes any changed call sites.

**AC-5 -- ratskin 0.3.1 can be added to Cargo.toml without version conflicts.**
Verified by: `cargo add ratskin` (or manual Cargo.toml edit + `cargo check`) exits 0. ratskin itself does not need to be wired up in this task — just confirmed compatible.

## Stage Report: design

- DONE: Current ratatui version confirmed and the minimum version required by ratskin 0.3.1 is stated.
  `Cargo.toml` shows `ratatui = "0.29"`. ratskin 0.3.1 requires `ratatui ^0.30.0` (confirmed via crates.io API).

- DONE: Any breaking API changes between the current version and 0.30+ are identified so the plan stage knows what to fix.
  See breaking changes inventory below.

- DONE: Acceptance criteria are concrete and verifiable — each names a cargo command or file check.
  All five ACs already name a `cargo` command or `grep` file check; no additions needed.

### Breaking changes inventory (0.29 → 0.30)

The following changes from the ratatui BREAKING-CHANGES.md are relevant to SpaceTop source:

**1. `layout::Alignment` renamed to `layout::HorizontalAlignment`** (type alias provided)
- Impact: LOW. SpaceTop imports `Alignment` by name in `src/ui/mod.rs:7` and `src/ui/graph.rs:9`. The type alias preserves the old name so no code change is required, but the plan stage should note this in case the alias is ever removed.

**2. `List::highlight_symbol()` signature changed from `&str` to `Into<Line>`**
- Impact: NONE. SpaceTop calls `.highlight_symbol("> ")` at `src/ui/mod.rs:423` with a string literal, which implements `Into<Line>`.

**3. `Block::title()` now accepts `Into<Line>` instead of `Into<Title>`; `block::Title` type removed**
- Impact: NONE. SpaceTop calls `.title("Help")` at `src/ui/mod.rs:361` with a string literal; `block::Title` is not imported anywhere in the codebase.

**4. `Flex::SpaceAround` behavior changed** (old behavior moved to `Flex::SpaceEvenly`)
- Impact: NONE. SpaceTop does not use `Flex::SpaceAround`.

**5. `Marker` enum is now `#[non_exhaustive]`**
- Impact: NONE. SpaceTop does not pattern-match on `Marker` exhaustively.

**6. `Layout::init_cache()` and `Layout::DEFAULT_CACHE_SIZE` gated behind `layout-cache` feature**
- Impact: NONE. SpaceTop does not call `Layout::init_cache()`.

**7. Custom `Backend` trait now requires `Error` associated type and `clear_region()` method**
- Impact: NONE. SpaceTop uses only `CrosstermBackend` (provided by ratatui) and `TestBackend`; no custom backend implementation exists.

**8. MSRV bumped to 1.86.0**
- Impact: NONE. SpaceTop toolchain is Rust 1.97.0-nightly.

**Net assessment:** No source changes are required for the upgrade. The plan stage can bump `ratatui = "0.29"` to `ratatui = "0.30"` in `Cargo.toml` and run `cargo check` + `cargo test --lib` to confirm.

### Summary

SpaceTop currently uses `ratatui = "0.29"` in `Cargo.toml`. ratskin 0.3.1 requires `ratatui ^0.30.0`. The ratatui 0.29 → 0.30 breaking changes were reviewed against all ratatui call sites in `src/`; none of the breaking changes affect SpaceTop's code because the project uses string literals where signatures changed to `Into<Line>`, does not implement a custom backend, and does not use removed types (`block::Title`, `Flex::SpaceAround`, exhaustive `Marker` matching, or `Layout::init_cache`). The `Alignment` rename has a compatibility alias. The plan stage needs only a single Cargo.toml version bump with no source edits.

## Implementation plan

### Step 1 — Bump ratatui in Cargo.toml

File: `/Users/kent/Dev/InfuseAI/GitHub/spacetop/Cargo.toml`

Change:
```
ratatui = "0.29"
```
to:
```
ratatui = "0.30"
```

No other source files need editing (see breaking changes inventory in the design stage report).

### Step 2 — Verify the project compiles

```
cargo check
```

Expected: exit 0 with no errors. This satisfies AC-2 and AC-4.

### Step 3 — Verify all existing tests pass

```
cargo test --lib
```

Expected: exit 0 with no failures. This satisfies AC-3.

### Step 4 — Verify ratskin 0.3.1 compatibility (AC-5)

Add ratskin temporarily to check for version conflicts:

```
cargo add ratskin
cargo check
```

Expected: exit 0. Then remove ratskin from `Cargo.toml` (and `Cargo.lock` via `cargo rm ratskin` or manual edit) since wiring it up is out of scope for this task — that is task 019's job.

Alternatively, manually add `ratskin = "0.3"` to `[dependencies]` in `Cargo.toml`, run `cargo check`, then revert the addition.

### File ownership

- `/Users/kent/Dev/InfuseAI/GitHub/spacetop/Cargo.toml` — single line edit, owned by this task
- `/Users/kent/Dev/InfuseAI/GitHub/spacetop/Cargo.lock` — updated automatically by cargo, no manual edit needed
- No source files under `src/` require changes

## Stage Report: plan

- DONE: Plan is a single concrete step: bump ratatui in Cargo.toml to 0.30, run cargo check, run cargo test --lib.
  Four-step plan above; Step 1 is the single Cargo.toml change, Steps 2-3 are the two verification commands.
- DONE: Plan notes how to verify ratskin compatibility (AC-5) without wiring it up yet.
  Step 4 uses `cargo add ratskin && cargo check` then removes it; explicitly notes ratskin wiring is task 019's scope.
- DONE: File ownership is clear so the implement stage can start immediately.
  Only `Cargo.toml` needs editing; `Cargo.lock` updates automatically; no `src/` changes required.

### Summary

The plan distills to a single line change in `Cargo.toml` (`ratatui = "0.29"` → `ratatui = "0.30"`) followed by `cargo check` and `cargo test --lib`. The design stage's breaking-changes inventory confirmed no source edits are needed. AC-5 ratskin compatibility is verified by temporarily adding `ratskin` via `cargo add`, confirming `cargo check` exits 0, then removing it — ratskin wiring remains task 019's responsibility. File ownership is unambiguous: only `Cargo.toml` is modified.
