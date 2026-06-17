---
title: Agent session active marker
status: verify
source: docs/spacetop-dev/_artifacts/2026-06-14-agent-session-progress-survey.md; https://github.com/spacedock-dev/spacetop/issues/28
kind: feature
risk: medium
milestone: v2-later
proof: scanner fixtures, app result application tests, Ratatui task-row rendering tests, make lint
started: 2026-06-17T03:45:57Z
completed:
verdict:
score: 0.84
worktree: .worktrees/spacedock-ensign-065-agent-session-active-marker
issue: https://github.com/spacedock-dev/spacetop/issues/28
pr: "#68"
id: 065
mod-block: merge:pr-merge
---

Implement the narrow first slice from the 2026-06-14 agent session progress survey: show whether an active task appears to be handled by a currently running matched Codex or Claude Code session.

The first slice should stay local, read-only, and confidence-rated. It should answer the scanning question "is this task actively being handled right now?" without claiming authoritative ownership, lock state, waiting-for-user, or stuck/progress states.

## Scope

- Kind: feature
- Risk: medium
- Milestone: v2-later
- Touches: parser / app-state / UI / docs
- Non-goals: authoritative assignment metadata, waiting/stuck state inference, workflow markdown writes, transcript rendering, remote upload, Nerd Font-only UI

## Acceptance criteria

**AC-1 -- Running matched Codex sessions are visible in task rows.**
A task with high-confidence Codex worktree evidence and a running matched session renders a compact active marker in the task list.
Verified by: core scanner/correlation fixtures plus Ratatui task-row rendering tests.

**AC-2 -- Running matched Claude Code sessions are visible in task rows.**
A task with high-confidence Claude Code worktree evidence and a running matched session renders the same compact active marker in the task list.
Verified by: core scanner/correlation fixtures plus Ratatui task-row rendering tests.

**AC-3 -- Old or weak evidence does not mark active work.**
A task without a running matched session renders no active marker, even if old or low-confidence session evidence exists.
Verified by: correlation ranking tests covering stale, recently active, and low-confidence evidence.

**AC-4 -- Preview details identify the matched agent without leaking transcript content.**
When attribution exists, the preview/details area names Codex, Claude Code, or multi-agent, plus session identity or display name, confidence, run state, and latest activity; it does not render prompt, response, command output, or transcript body text.
Verified by: rendering tests and fixture assertions over derived structural evidence only.

**AC-5 -- Workflow loading remains resilient and read-only.**
Session scanner failures are non-fatal, workflow markdown is not modified, and `spacetop-core` remains terminal-free.
Verified by: scanner failure tests, existing no-terminal-deps guardrail, no workflow-write implementation path, and `make lint`.

## Proof plan

- Lowest test layer: scanner fixtures and correlation tests in `spacetop-core`, app worker/state tests for result application, Ratatui `TestBackend` tests for row and preview rendering.
- Required command: `cargo test` and `make lint`.
- Manual check, if any: optional local TUI check against `docs/spacetop-dev` with one running matched session.
- Docs/policy update needed: README or nearby docs only if the user-facing marker or configuration surface changes.

## Implementation Plan

1. Add a core-owned session attribution model in `crates/spacetop-core/src/domain/mod.rs` and a new `crates/spacetop-core/src/session_activity.rs` module. The typed boundary should expose `AgentKind`, `AgentSessionState`, `AttributionConfidence`, `AgentSessionEvidence`, `EntitySessionAttribution`, and `SessionScanReport`; callers receive structural facts only, never transcript text.

2. Implement local scanner inputs in `session_activity.rs` behind small filesystem/process traits. The scanner should inspect local structural evidence from known Codex and Claude Code session roots, git worktree paths, process liveness, timestamps, cwd/workdir metadata, and entity/worktree identifiers. It must not read prompt/response bodies into renderable data, mutate workflow files, call remote services, or require terminal crates.

3. Correlate scanner evidence to active `Entity` rows by stable workflow facts: entity slug/id, `Entity.worktree`, `Entity.worktree_source`, workflow-relative path, repo root, and discovered worktree roots. Ranking should prefer running sessions with direct worktree/cwd evidence, then recent structural references, and should reject stale or weak evidence below the active-marker threshold.

4. Store attribution results in `WorkflowIndex` as optional per-entity metadata, with query methods such as `session_attribution_for_entity_id` or an entity-details projection. Do not extend UI code to infer session state from raw vectors, filenames, or marker strings. Keep archived rows unmarked for the first slice unless a later task explicitly broadens scope.

5. Add an app-layer worker mirroring `history_worker.rs`: `crates/spacetop/src/app/session_activity_worker.rs` owns background scanning, request/result structs, and mpsc delivery. `OverviewState` should keep scan status, last scan error, and a refresh timestamp, and apply results only when the result's `workflow_dir` and `repo_root` match the active state.

6. Wire worker lifecycle in `crates/spacetop/src/lib.rs` near the existing history worker polling. Initial workflow load, explicit reload, workflow switch materialization, and watcher refresh should mark attribution as loading or stale and enqueue a scan. Scanner failures should become non-fatal app status, leaving workflow loading and task rendering intact.

7. Render from typed metadata only. In `crates/spacetop/src/ui/list.rs`, add one fixed-width active marker column beside the existing worktree marker; use a compact ASCII-safe marker such as `* ` or `@ ` unless a separate ASCII fallback decision is made. In `crates/spacetop/src/ui/preview.rs`, add a metadata line that names Codex, Claude Code, or multi-agent, session display/id, confidence, run state, and latest structural activity.

8. Keep privacy and read-only guarantees explicit in implementation. Derived metadata may include session id/display name, agent kind, confidence, run state, matched worktree/cwd path, and latest activity timestamp; it must not include prompt text, responses, command output, transcript snippets, or workflow markdown writes. Scanner errors should be summarized as status strings, not as raw file contents.

9. Update user-facing docs only if the chosen marker or scanner behavior is visible beyond tests. Likely targets are `README.md` for the active marker and help/footer docs if a status indicator or keybinding is added; no policy change is needed for the read-only first slice.

10. Verification sequence for implementation: add scanner fixtures and correlation tests in `spacetop-core`; add app tests for applying matching, stale, and failed scan results; add Ratatui `TestBackend` tests in `crates/spacetop/src/ui/tests/task_list.rs` and `preview.rs`; run `cargo fmt`, `cargo test`, `make lint`, and keep `crates/spacetop-core/tests/no_terminal_deps.rs` passing.

## Stage Report: plan

- DONE: plan separates core session scanning/correlation, app background refresh state, and Ratatui rendering into owned modules with typed boundaries.
  Evidence: `Implementation Plan` steps 1-7 assign scanner/correlation to `spacetop-core`, refresh/application to `app/session_activity_worker.rs` and `OverviewState`, and rendering to `ui/list.rs` plus `ui/preview.rs`.
- DONE: plan names the lowest practical tests for scanner fixtures, attribution ranking, app result application, task-row marker rendering, and preview metadata rendering.
  Evidence: `Implementation Plan` step 10 names core scanner/correlation tests, app result application tests, and Ratatui `TestBackend` tests for task rows and preview metadata.
- DONE: plan de-risks privacy and read-only behavior by specifying local-only structural evidence, scanner failure handling, no transcript rendering, and no workflow markdown writes from the feature.
  Evidence: `Implementation Plan` steps 2, 6, and 8 limit evidence to local structural facts, make scanner failures non-fatal, and prohibit transcript content or workflow markdown writes.

### Summary

Planned the first active-session marker slice as a typed local attribution pipeline: core scans and ranks local Codex/Claude Code structural evidence, app state refreshes it in the background, and Ratatui renders only derived metadata. The plan preserves Spacetop's read-only contract and identifies the implementation proof path: scanner fixtures, ranking tests, app application tests, task-row rendering tests, preview metadata tests, `cargo test`, and `make lint`.

## Stage Report: implement

- DONE: implement typed core attribution for Codex and Claude Code local session evidence, including fixture-root scanners, confidence/run-state ranking, and no transcript-content retention.
  Evidence: commit `ffc2609` adds `spacetop_core::session_activity` and domain attribution types; core fixture tests cover running Codex, running Claude Code, and stale/weak evidence.
- DONE: implement non-blocking app attribution refresh state that applies results to the active workflow, treats scanner failures as non-fatal, and refreshes on workflow reload.
  Evidence: commit `ffc2609` adds `app/session_activity_worker.rs`, event-loop refresh hooks, matching/stale/failure app tests, and typed `WorkflowIndex` result application.
- DONE: render the active-session marker and preview metadata from typed state with Unicode/ASCII behavior, focused tests, cargo fmt, cargo test, and make lint evidence.
  Evidence: commit `ffc2609` renders ASCII `@ ` row markers and preview metadata; `cargo fmt`, `cargo test`, and `make lint` passed.

### Summary

Implemented the read-only first slice for active local agent-session attribution. Core scanning correlates structural Codex and Claude Code session evidence to active workflow entities, app state refreshes attribution in a background worker, and the TUI renders an `@ ` active marker plus privacy-preserving preview metadata from typed state only.

## Stage Report: verify

- DONE: verify every acceptance criterion has concrete implementation and test evidence, including Codex and Claude running markers, stale/weak no-marker behavior, preview metadata, scanner failure resilience, and core terminal-free safety.
  Evidence: inspected `session_activity`, `WorkflowIndex`, app worker/application, list/preview render paths, and matching core/app/Ratatui tests; AC-1 through AC-5 are covered.
- DONE: independently inspect the implementation diff and rerun or validate required proof commands, especially cargo fmt, cargo test, make lint, and the no-terminal-deps guardrail.
  Evidence: reviewed `main...HEAD`; `cargo fmt --check`, `cargo test`, and `make lint` passed; `cargo test` included `crates/spacetop-core/tests/no_terminal_deps.rs` 1/1 and `no_write_git_calls.rs` 2/2.
- DONE: decide PASSED or REJECTED with specific defects or approval notes, preserving privacy/read-only requirements and naming any missing evidence.
  Evidence: PASSED recommendation; no blocking defects found, no workflow-write path found, and targeted privacy/read-only scan found only test fixture writes.

### Summary

PASSED. The branch implements local, read-only active-session attribution through typed core metadata, applies it asynchronously in app state, and renders only structural marker/preview details without transcript content.
