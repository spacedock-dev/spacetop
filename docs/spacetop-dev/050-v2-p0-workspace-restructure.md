---
id: "050"
title: "v2 P0: workspace restructure + WorkItem to Entity rename"
status: verify
source: "captain — v2 design + plan (this session)"
started: 2026-06-11
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-050-v2-p0-workspace-restructure
issue:
pr: "#49"
mod-block: merge:pr-merge
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

## Stage Report: implement (cycle 1 fix)

- DONE: Complete the two-crate workspace split: `spacetop-core` owns pure logic with no terminal UI dependencies, and `spacetop` owns CLI/TUI/app/main.
  Commits `f294625`, `d1bd7bf`, and `4d15624`; `cargo build --workspace` passed, and `make build` produced `target/release/spacetop`.
- DONE: Complete the `WorkItem` to `Entity` rename and update read-only/git-write guardrails so tests scan the new crate layout and preserve the `git pull --ff-only` boundary.
  Commit `b2500b4`; `rg -n '\bWorkItem\b' crates/*/src` returned no matches; `cargo test -p spacetop-core --test no_write_git_calls --test no_terminal_deps` passed 3/3.
- DONE: Update required docs and record current proof: `cargo test --workspace`, `make lint`, dependency/grep checks, smoke evidence, and any known failures with scope rationale.
  Commit `4d15624`; `cargo test --workspace --no-fail-fast` reported 361 passed, 8 known fixture/readme failures, 3 ignored; `make lint`, `make build`, dependency checks, and smoke run passed.

### Summary

P0 is implemented as a two-crate workspace with `spacetop-core` holding domain/parser/discovery/watcher/git sync/editor logic and `spacetop` holding CLI/TUI/app/main. The domain model is now `Entity`, `spacetop-core` has a cargo-tree terminal-dependency guard, and the git-write guard scans both crate source trees while preserving the single `git pull --ff-only` path.

One plan detail was adjusted during implementation: `git_sync_e2e.rs` remains a `spacetop` integration test because it exercises `App` and `apply_pending_sync`; moving it into `spacetop-core` would create the wrong dependency direction. The remaining 8 full-suite failures are unchanged workflow-fixture drift from the rejected verify baseline: tests still expect `design/review` and older README prose while `docs/spacetop-dev` now uses `shape/verify`, plus one archived fixture status expectation.

## Stage Report: verify (cycle 1)

Verdict: PASSED

- DONE: Verify AC-1 and AC-2 against the worktree: two-crate layout, workspace build, and `spacetop-core` has no terminal UI dependencies.
  `Cargo.toml` declares `crates/spacetop-core` and `crates/spacetop`; `find crates/.../src` confirmed the split; `cargo build --workspace`, `cargo tree -p spacetop-core --edges normal --prefix none`, and core guardrail tests passed.
- DONE: Verify AC-3 and AC-4 against the worktree: `Entity` rename is complete in live source and read-only/git-write guardrails scan the new crate layout with exactly one `git pull --ff-only` path.
  `rg -n '\bWorkItem\b' crates/*/src` returned no matches; `cargo test -p spacetop-core --test no_terminal_deps --test no_write_git_calls` passed 3/3, including the two-crate source scan and exactly-one `--ff-only` assertion.
- DONE: Verify AC-5 and AC-6 against the worktree: behavior evidence, required lint/test/smoke commands, known failures classification, and docs/policy updates are current and consistent.
  `make lint` passed; PTY smoke `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev` rendered and quit with `q`; stale-doc scans over `AGENTS.md`, `docs/development-policy.md`, `CLAUDE.md`, and `README.md` found no root-`src/...` or `WorkItem` drift; `cargo test --workspace --no-fail-fast` remains 361 passed, 8 failed, 3 ignored from unchanged README/archive fixture drift.

### Summary

The implementation satisfies all six ACs for this P0 restructure: the two-crate workspace is present, `spacetop-core` remains terminal-free, live source uses `Entity`, and the read-only git boundary is still guarded across both crate source trees. The only non-green evidence is the pre-existing workflow-fixture drift already recorded in the rejected verify baseline; the P0 diff does not touch `docs/spacetop-dev/README.md` or `_archive`, and the failing tests were moved without behavioral changes.
