---
id: "050"
title: "v2 P0: workspace restructure + WorkItem to Entity rename"
status: implement
source: "captain — v2 design + plan (this session)"
started: 2026-06-11
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-050-v2-p0-workspace-restructure
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

### Feedback Cycles

- **cycle 1 (2026-06-11, verify -> implement):** Verify rejected because the
  current checkout does not contain the P0 implementation. The repo is still a
  single `spacetop` package, `crates/spacetop-core` does not exist, `WorkItem`
  still appears in live source, the no-write guardrail still scans only `src/`,
  and docs still describe the single-crate layout. Fix: complete the planned P0
  workspace restructure, rename `WorkItem` to `Entity`, update guardrail tests
  and docs, then provide current `cargo test --workspace`, `make lint`,
  dependency/grep checks, and smoke evidence.

## Stage Report: verify

Verdict: REJECTED

- FAILED: Verify the two-crate workspace layout and prove `spacetop-core` has no terminal UI dependencies.
  `find . -maxdepth 3 -type f ...` found only `./Cargo.toml`, `./src/lib.rs`, and `./src/main.rs`; `find crates ...` failed with `crates: No such file or directory`; `cargo tree -p spacetop-core` failed because no such package exists.
- FAILED: Verify the `WorkItem` to `Entity` rename and read-only/git-write guardrails match the task acceptance criteria.
  `rg -n '\bWorkItem\b' crates src tests` failed with missing `crates/` and reported live source references including `src/domain/mod.rs:168:pub struct WorkItem`; `cargo test --test no_write_git_calls` passed 2/2 but still scans only the old `src/` tree, not both crate source trees.
- FAILED: Verify required evidence is current: `cargo test --workspace`, `make lint`, relevant grep/dependency checks, smoke/docs checks, and any known fixture failures are explained.
  `cargo test --workspace` failed with 334 passed and 8 failed; `make lint` passed; `cargo build --workspace` passed only the current single package; `cargo run -- --workflow-dir docs/spacetop-dev` launched and rendered the old TUI, then quit with `q`.

### Summary

The current checkout does not contain the P0 implementation. It is still a single `spacetop` package with terminal dependencies in the root manifest, no `crates/spacetop-core`, and many `WorkItem` source tokens. Docs are also stale for this AC set: `AGENTS.md`, `docs/development-policy.md`, and `CLAUDE.md` still document `src/...` single-crate ownership, and `CLAUDE.md` still names `WorkItem`.
