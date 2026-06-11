---
id: "050"
title: "v2 P0: workspace restructure + WorkItem to Entity rename"
status: implement
source: "captain — v2 design + plan (this session)"
started: 2026-06-11
completed:
verdict:
score:
worktree:
issue:
pr:
---

Phase P0 of the SpaceTop v2 internals rebuild: convert the single `spacetop`
binary crate into a two-crate Cargo workspace — `spacetop-core` (pure logic,
zero terminal dependencies) and `spacetop` (bin: TUI + CLI) — and rename the
`WorkItem` model to `Entity`. Behavior-identical: no features, no async,
read-only contract preserved.

This is the foundation phase of the strangler migration; later phases (index +
query API, git history, capabilities, config, headless CLI) build on the core
boundary this phase establishes.

## Stage status

- **design** — complete. Spec: `docs/superpowers/specs/2026-06-11-spacetop-v2-design.md`
  (brainstormed, then revised after a 3-reviewer adversarial pass — see the spec's
  Review history).
- **plan** — complete. Plan: `docs/superpowers/plans/2026-06-11-spacetop-v2-p0-workspace-restructure.md`
  (7 tasks, TDD where applicable, green checkpoint at every commit).
- **implement** — pending. Execution will be driven externally via codex against
  the plan above; this entity was fired without dispatch at the captain's request.
- **review** — gate; awaits implementation.

## Acceptance criteria

Each AC names a property of the finished P0 and how it is verified. (Mirrors the
plan's "Definition of done".)

**AC-1 — Two-crate workspace exists.**
`spacetop-core` contains only `domain`, `parser`, `discovery`, `watcher`,
`git_sync`, `editor`; `spacetop` contains `cli`, `lib`, `app`, `ui`, `main`.
Verified by: `crates/spacetop-core/src/` and `crates/spacetop/src/` layout;
`cargo build --workspace` succeeds.

**AC-2 — Core links no terminal crate.**
`spacetop-core`'s resolved dependency tree contains none of `ratatui`,
`crossterm`, `termimad`, `ratskin`.
Verified by: `crates/spacetop-core/tests/no_terminal_deps.rs` passes (and was
demonstrated to fail when a terminal crate is temporarily added).

**AC-3 — Model renamed.**
The domain type is `Entity` everywhere; no `WorkItem` token remains in code.
Verified by: `grep -rn '\bWorkItem\b' crates/*/src` returns nothing.

**AC-4 — Read-only contract intact, scanning both crates.**
Verified by: `crates/spacetop-core/tests/no_write_git_calls.rs` passes scanning
both crate src trees; `--ff-only` appears exactly once.

**AC-5 — Behavior identical.**
All existing tests pass (relocated as needed) with no new failures; the two
known pre-existing fixture failures are unchanged.
Verified by: `cargo test --workspace`; `make lint` clean; smoke run
`cargo run -p spacetop -- --workflow-dir docs/spacetop-dev` renders as before.

**AC-6 — Docs track the change.**
`AGENTS.md`, `docs/development-policy.md`, `CLAUDE.md`, `README.md` describe the
two-crate layout and the `Entity` name.
Verified by: no stale single-crate `src/...` module paths or `WorkItem`
references remain in those files.

## Review notes

(To be filled at the review gate.)
