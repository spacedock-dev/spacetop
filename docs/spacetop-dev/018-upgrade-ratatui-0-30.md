---
id: "018"
title: Upgrade ratatui to 0.30+ to unblock ratskin adoption
status: design
source: captain (unblocks 017 ratskin path)
started: 2026-04-25T16:03:12Z
completed:
verdict:
score:
worktree:
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
