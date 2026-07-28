---
title: Fix intermittent Codex and Claude activity detection
status: verify
source: "Live false-idle observation while dispatching task 074 on 2026-07-28"
kind: bugfix
risk: high
milestone: v1-maintenance
proof: Reproducing Codex and Claude session fixtures plus repeated scanner and task-list activity tests
started: 2026-07-28T06:55:46Z
completed:
verdict:
score: 0.92
worktree: .worktrees/spacedock-ensign-stable-agent-activity-detection
issue:
pr: "#79"
id: 075
mod-block: merge:pr-merge
---

Spacetop does not reliably detect active task handling from Codex and Claude Code session logs. The same dispatched worker can appear as `running · worker` in one scan and `idle` in another, or remain `idle` while it is actively processing the task.

A live example occurred while task 074 was in `plan`: the Codex planning worker `/root/spacedock_ensign_compact_copyable_slug_ids_plan` was active, but Spacetop displayed the task as `idle`. Similar intermittent false-idle behavior has been observed with Claude Code workers.

The activity display must remain stable while structured evidence says a worker or first officer is handling the task. It should return to `idle` only after the corresponding structured terminal or idle event, subject to the existing `human-gate > running · worker > running · FO > idle` precedence.

## Acceptance criteria

- **AC-1:** An exact dispatched Codex worker remains `running · worker` across repeated scans until its structured completion or idle event is observed.
- **AC-2:** An exact dispatched Claude Code worker remains `running · worker` across repeated scans until its structured terminal or idle event is observed.
- **AC-3:** The investigation reproduces the task 074 false-idle sequence or an equivalent sanitized session sequence and identifies which evidence is missed, dropped, or incorrectly invalidated.
- **AC-4:** Partial appends, delayed records, unchanged scans, cursor reuse, log rotation or truncation, and session-file ordering cannot transiently erase still-valid active evidence for either runtime.
- **AC-5:** Worker completion and first-officer handoff still transition correctly; the fix must not leave completed workers stuck as running or weaken exact task attribution and false-positive rejection.
- **AC-6:** Tests cover repeated scans and lifecycle transitions for both Codex and Claude Code, including an active-worker false-idle regression and task-list rendering of the resulting status.
- **AC-7:** Verification includes focused scanner tests, full `cargo test`, `make lint`, and a live or fixture replay showing stable status over multiple scan intervals.

## Investigation context

The defect is intermittent and affects the structured session-activity scanner rather than workflow frontmatter. Inspect session snapshot invalidation, incremental cursor summaries, record ordering, parent linkage, and reducer state retention before changing UI rendering.

## Plan

### Reproduced failure and root cause

The task 074 child rollout gives a concrete Codex sequence. Record 1 contains a
canonical child `agent_path`, exact non-empty `parent_thread_id`, and rollout id;
record 2 is `task_started`. Record 10 is the v2 `agent_message`, whose assignment
payload is encrypted, so `project_codex_response_item` drops it and
`child_matches` never becomes true. The parent independently records
`sub_agent_activity(kind=started)` with the exact child rollout id and agent path,
but `collect_codex_events` currently uses parent activity only for
`kind=interrupted`. The child's first dispatch-file read is also named
`exec_command`, while FO scoping accepts only `exec`/`Bash`.

A sanitized replay of that order produced:

| Sequence | Current result |
|---|---|
| Codex `session_meta` + `task_started` | `idle` |
| plus encrypted v2 `agent_message` | `idle` |
| plus a legacy assignment record | `running · worker` |
| unchanged rescan at byte cursor 540 | `running · worker` |
| partial append, length 554 and cursor still 540 | `running · worker` |
| truncate to the still-open meta/start records, cursor rebuilt at 293 | `idle` without a terminal event |

Current Claude artifacts expose three additional schema/lifecycle gaps:

- Dispatch basenames may be
  `{parent-session}-spacedock-ensign-{slug}-{stage}.md`, but
  `dispatch_markers` accepts only the unprefixed basename.
- `in_process_teammate` metadata now carries the worker name but can omit
  `agentId`, `parentSessionId`, and `parentToolUseId`; the sibling sidechain
  carries the non-empty `agentId` and its directory supplies the parent session.
  `claude_teammate_meta` requires the missing metadata `agentId`, so the join is
  discarded.
- `collect_claude_events` uses `find` for the first child assistant and first
  parent idle notification. A reused worker therefore transitions
  `running -> idle`, but an attributed follow-up teammate message plus a later
  assistant record does not reopen it.

The replay confirmed `running · worker -> idle` when valid legacy Claude metadata
was rewritten into the current shape, `running · worker -> idle` on a partial
metadata rewrite, and `running · worker -> idle -> idle` across an accepted
follow-up. These are parser and retained-evidence failures, not UI failures.

### Implementation path and ownership

1. **Pin sanitized schemas first.** Add Codex v2 parent/child fixtures and modern
   Claude parent/meta/sidechain fixtures under
   `tests/fixtures/session-activity/`. Add failing incremental tests in
   `crates/spacetop-core/src/session_activity.rs` for the replay above before
   changing the scanner. Fixtures retain ids, paths, event kinds, timestamps,
   tool names, and envelope types only.

2. **Replace file-shaped cache state with typed scan state.** Keep the public
   facade in `crates/spacetop-core/src/session_activity.rs`, and extract focused
   `projection`, `codex`, `claude`, `state`, and `reducer` submodules. Introduce
   `SessionScanState`, containing per-file `SessionFileCursor` values plus a
   privacy-safe `SessionEvidenceStore` keyed by stable runtime/session identity.
   Project JSON into typed facts, never retained `serde_json::Value` records.
   `EvidenceOrder(timestamp, source identity, byte offset, kind)` makes reduction
   deterministic instead of depending on `WalkDir` order or vector insertion.

3. **Preserve evidence, not inferred UI labels.** The store retains observed
   dispatch identity and lifecycle facts across unchanged scans, partial writes,
   cursor resets, rotations, deletion, and transient per-file read/metadata
   failures. A malformed standalone Claude metadata rewrite reports an error but
   does not replace its last valid identity. Truncation resets only that file's
   cursor; it does not synthesize `WorkerStopped`. Facts are removed from the
   open reducer state only by an exact structured terminal/idle event. The
   visible `EntityActivity` enum and precedence remain unchanged.

4. **Make a scan generation coherent.** Inventory session paths and
   `(length, modified)` before and after reading. If a file appears, disappears,
   or changes during the scan, return an unstable-generation result, preserve
   the last published report/state, and request an immediate rescan. This prevents
   a parent stop from being published without a concurrently appended child
   restart merely because the two files were visited in the opposite order.

5. **Update Codex correlation without weakening attribution.** Project `cwd` from
   child and parent session metadata. Accept a worker only when the canonical
   child path, non-empty parent id, matching repo/worktree `cwd`, child
   `task_started`, and either the legacy exact assignment or the parent's exact
   `sub_agent_activity(kind=started, agent_thread_id, agent_path)` all agree.
   Continue to close only the matching child turn's `task_complete` or exact
   parent interruption. Recognize `exec_command` as an executable FO command,
   while keeping module text, generic path mentions, and encrypted prose
   insufficient.

6. **Update Claude correlation and lifecycle.** Accept the optional
   parent-session prefix only inside the exact dispatch directory and only when
   the remaining basename equals the canonical worker name/stage. Join the
   parent `Agent` call to the sibling meta/JSONL pair using parent directory,
   worker name, non-empty sidechain `agentId`, matching repo/worktree `cwd`, and
   `parentToolUseId` when present; without a call id, retain the existing
   unique-parent/name fail-closed rule. Emit every matching idle transition.
   After idle, reopen only after an attributed child teammate-message boundary
   followed by a later assistant record from the same `agentId`.

7. **Thread state through the existing background boundary.** Replace
   `previous_session_files`/`session_files` with `SessionScanState` in
   `crates/spacetop/src/app/session_activity_worker.rs` and
   `SessionActivityWorkerState` in `crates/spacetop/src/lib.rs`. Only a coherent
   successful generation replaces the cached state and
   `WorkflowIndex` report. `crates/spacetop/src/app/overview.rs`,
   `crates/spacetop-core/src/index.rs`, and `crates/spacetop/src/ui/list.rs`
   should require no behavior change; add application/rendering tests only.

8. **Keep the contract current.** Update
   `docs/superpowers/specs/2026-07-27-spacetop-entity-activity-design.md` with the
   v2 Codex parent-start join, prefixed Claude dispatch/meta schema, reusable
   worker lifecycle, and retained scan-state rule. Update the `session_activity`
   code-map paragraph in `AGENTS.md` if the module split lands. Add no dependency.

### Falsifiable proof matrix

| Claim | Lowest-layer proof and failure signal |
|---|---|
| Codex v2 starts exactly | Parent-start + child meta/start fixture is `running · worker`; mismatching any parent id, child id, agent path, cwd, or slug makes it `idle`. |
| Claude modern schema starts exactly | Prefixed dispatch + sibling meta/sidechain fixture runs; cross-parent, cross-cwd, duplicate no-call-id, mismatched name, and ordinary message controls remain `idle`. |
| Active evidence survives scans | For each runtime, scan the same state at least three times, then unchanged, partial-append, delayed-correlation, malformed-meta, truncate, rotate, delete, and visit-order permutations; every scan after the first proven start remains `running · worker`. A cursor reset or parse error causing `idle` fails the test. |
| Terminals still win | Append the exact Codex `task_complete`/interruption or Claude idle envelope and assert `idle` with terminal timestamp; append a correlated Claude follow-up plus assistant and assert running again. A deletion alone must not stop a worker. |
| Precedence and handoff remain intact | Shuffle deterministic typed events and prove `human-gate > worker > FO > idle`; worker stop reveals an open FO, FO stop yields idle, and a next worker replaces the old session id. |
| App and task list consume typed output | Apply the replay report through `OverviewState`, render with `TestBackend`, and assert the exact row shows the green marker and `running · worker`; applying the terminal report removes it. |
| Repository gates pass | Run `cargo fmt --check`, `cargo test -p spacetop-core session_activity`, `cargo test -p spacetop task_row_renders`, full `cargo test`, and `make lint`. |
| Replay is observable | Run the checked-in sanitized replay test with `--nocapture`; expected intervals are Codex `running, running, running, idle` and Claude `running, running, idle, running, idle`. |

## Stage Report: plan

- DONE: Reproduce and explain the intermittent false-idle path for both Codex and Claude Code, including cursor or record-order evidence.
  Live task 074 record shapes plus a sanitized executable replay identified encrypted Codex assignment/schema drift, Claude metadata/dispatch drift, first-record-only lifecycle reduction, and active evidence lost on cursor reset.
- DONE: Name the exact typed state, parser, and test boundaries that preserve active evidence without weakening attribution or lifecycle transitions.
  The plan specifies `SessionScanState`, `SessionFileCursor`, `SessionEvidenceStore`, deterministic typed evidence, runtime-specific parsers, coherent scan generations, background-state threading, and unchanged `EntityActivity` precedence.
- DONE: Define a falsifiable proof matrix covering repeated scans, partial or delayed log updates, completion handoff, full tests, lint, and replay evidence.
  The matrix names exact positive/negative joins, incremental mutations, lifecycle/precedence assertions, task-list rendering, commands, and expected replay sequences.

### Summary

The false-idle behavior is reproducible in the scanner and comes from both live
schema drift and discarding previously proven lifecycle evidence. The
implementation should retain typed evidence across scans, correlate current
Codex/Claude records with exact parent/session/cwd keys, and publish only a
coherent scan generation; no UI-state inference or workflow write is needed.

## Stage Report: implement

- DONE: Implement exact Codex and Claude correlation plus retained typed scan evidence so active workers survive repeated, partial, reordered, truncated, rotated, or transiently malformed scans until structured lifecycle events.
  Commit `11bc334` adds typed projections and evidence ordered by timestamp/source/offset/kind; exact start/stop tests fail if retained evidence disappears or any parent, child, path, cwd, name, or call join weakens.
- DONE: Thread coherent scan generations through the existing background/app boundary without UI inference or weaker attribution, and update the design/code-map documentation for the final module ownership.
  `SessionScanState` now crosses the worker boundary; a before/after inventory change rejects publication and requests an immediate retry, while AGENTS.md and the activity design name the facade and five owning submodules.
- DONE: Add sanitized regression fixtures and prove positive, negative, handoff, precedence, rendering, replay, full-test, formatting, and lint outcomes from the plan’s falsifiable matrix.
  Fourteen focused scanner tests cover v2/legacy starts, exact negatives, retained scans, coherent generation, Claude reuse, terminals, FO/gates, deterministic precedence, and observable Codex/Claude replay sequences.
- DONE: Focused task-list rendering proof.
  `cargo test -p spacetop task_row_renders_scanner_replay_then_clears_on_terminal_report` fails unless scanner output renders the green worker marker/status and an exact terminal report removes it.
- DONE: Repository gates.
  `cargo fmt --all -- --check`, full `cargo test` (387 app, 182 core, integration/doc tests), `make lint`, and `git diff --check` passed after commit `11bc334`.
- DONE: Replay is observable.
  `cargo test -p spacetop-core session_activity -- --nocapture` reports Codex `running, running, running, idle` and Claude `running, running, idle, running, idle`.

### Summary

Spacetop now retains privacy-safe typed lifecycle evidence and publishes only
coherent session-log generations, eliminating false idle during partial,
truncated, rotated, deleted, or transiently malformed scans. Codex v2
parent-start joins, modern Claude metadata/sidechain joins, reusable-worker
handoffs, and direct `exec_command` evidence remain exact and fail closed.

## Stage Report: implement (cycle 2)

- DONE: Implement exact Codex and Claude correlation plus retained typed scan evidence so active workers survive repeated, partial, reordered, truncated, rotated, or transiently malformed scans until structured lifecycle events.
  Commit `ee26e92` preserves RFC3339 nanoseconds in the causal key while leaving visible `updated_unix` values at whole seconds; Claude same-second reopen and Codex parent-stop/child-restart tests fail if cross-file fractional ordering is lost.
- DONE: Thread coherent scan generations through the existing background/app boundary without UI inference or weaker attribution, and update the design/code-map documentation for the final module ownership.
  The correction stays inside typed projection/reduction and lifecycle correlation; no app/UI inference or scan-generation boundary changed.
- DONE: Add sanitized regression fixtures and prove positive, negative, handoff, precedence, rendering, replay, full-test, formatting, and lint outcomes from the plan’s falsifiable matrix.
  Sixteen focused session-activity tests now include `.100/.200/.300Z` Claude and Codex orderings plus a real old-log rename and replacement scan; the full suite, formatting check, `git diff --check`, and `make lint` pass.

### Summary

Cycle 2 fixes the verify-stage false idle caused by collapsing distinct
sub-second records onto one Unix second. It also replaces the prior claimed
rotation coverage with an exercised rename-and-replacement scan while
preserving whole-second timestamps at the domain/UI boundary.

## Stage Report: verify (cycle 2)

- DONE: Independently falsify the exact Codex and Claude joins and retained-evidence reducer across repeated scans, partial writes, truncation, rotation, malformed metadata, terminal events, reuse, FO handoff, and precedence.
  Verification first rejected `11bc334`: Claude idle `.100Z` -> attributed follow-up `.200Z` -> same-agent assistant `.300Z` incorrectly stayed idle; `ee26e925` fixes and permanently pins that case, Codex same-second stop/restart, real rotation, exact negative joins, retained scans, terminals, handoff, gates, and precedence.
- DONE: Audit coherent-generation publication, privacy-safe typed state, split module ownership, and unchanged UI/read-only boundaries against the implementation diff and task acceptance criteria.
  The diff keeps before/after inventory rejection and last-state preservation at the worker boundary, retains typed structural projections without transcript text, splits projection/state/runtime/reducer ownership under the core facade, changes no production UI inference, and passes terminal-free/read-only guardrails.
- DONE: Run the focused scanner and rendering replay proofs plus formatting, full cargo tests, lint, and diff checks; report exact evidence for every acceptance criterion or reject with actionable defects.
  `cargo test -p spacetop-core session_activity -- --nocapture` passed 16/16 with the expected Codex and Claude replay sequences; the focused Ratatui scanner replay passed; `cargo fmt --all -- --check`, `git diff --check`, full `cargo test`, and `make lint` all exited 0.
- DONE: AC-1 and AC-2 stable exact workers.
  Repeated Codex and Claude scans retain `running · worker`; exact terminal/idle facts close them, and sub-second Claude reuse reopens only after the attributed boundary plus later same-agent assistant.
- DONE: AC-3 reproduced false-idle and identified lost evidence.
  Live task-074 structure confirms parent start `.782Z` and child start `.837Z`; sanitized v2/modern fixtures reproduce the encrypted-assignment and retained-lifecycle gaps without transcript bodies.
- DONE: AC-4 incremental and ordering stability.
  Tests exercise unchanged cursor reuse, partial append, truncation, rename-and-replacement rotation, deletion, malformed Claude metadata, coherent-generation rejection, and same-second cross-file ordering without transiently erasing active evidence.
- DONE: AC-5 lifecycle and attribution safety.
  Exact task completion/interruption and Claude idle/reuse transitions pass; mismatched parent, path, cwd, cross-parent metadata, duplicate no-call-id dispatch, unlinked worker, and module-text controls stay idle.
- DONE: AC-6 scanner and rendering coverage.
  Both runtimes have repeated lifecycle regressions, and `task_row_renders_scanner_replay_then_clears_on_terminal_report` fails unless scanner output renders the green worker marker/status and the terminal report clears it.
- DONE: AC-7 repository and replay verification.
  Full results were 387 `spacetop` unit tests, 184 `spacetop-core` unit tests, all integration/doc tests, 0 failures; watcher backend smokes remained intentionally ignored because watcher behavior did not change.

### Summary

Verification found one high-severity residual false-idle in the first
implementation, routed it with a failing real-shape regression, and confirmed
the correction in `ee26e925`. The final branch satisfies all acceptance
criteria and repository gates; judgment: approve for the PR merge flow.

### Feedback Cycles

- Cycle 1: REJECTED — verify; surface 1 lifecycle-ordering defect vs estimate undeclared (n/a%); AC unchanged
