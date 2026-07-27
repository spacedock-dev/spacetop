---
title: Detect entity activity and human-gated sessions
status: done
source: "Follow-up from task 067 and the 2026-07-27 three-state activity design refinement"
kind: feature
risk: medium
milestone: v1-maintenance
proof: Core activity-state tests with representative Codex and Claude session logs plus task-list rendering tests for status and handler
started: 2026-07-27T13:20:01Z
completed: 2026-07-27T15:26:16Z
verdict: passed
score: 0.84
worktree: .worktrees/spacedock-ensign-detect-entity-activity-state
issue:
pr: pr-merge:76
id: 069
mod-block:
archived: 2026-07-27T15:26:16Z
---

Show one concise activity status for each entity:

- `idle`: no observable worker or first-officer activity is handling the entity.
- `running`: an observable worker or first-officer action is handling the entity.
- `human-gate`: the first officer is waiting for a human to approve or reject.

`running` also carries a handler, rendered inside the status as
`running · worker` or `running · FO`. There is no separate handler column, and
the handler is not a fourth status. The other statuses render as `idle` and
`human-gate` without a handler suffix. `idle` describes agent activity, not
workflow completion; an entity can be unfinished while idle.

The intended lifecycle includes both first-officer windows:

```text
idle
  -> running · FO
  -> running · worker
  -> running · FO
  -> idle | human-gate | running · worker
```

The first `running · FO` covers preparing and dispatching the assignment. The
second covers the handoff window after a worker finishes, while the first
officer processes the result or updates the entity before dispatching another
worker.

Detection must work from existing local Codex and Claude session artifacts and
workflow filesystem events. It must not require a new Spacedock plugin hook or
assume that `spacedock status --boot --json` knows runtime state.

Observable evidence includes:

- A worker starts when a worker session accepts the entity's exact dispatch
  assignment.
- A worker stops handling the entity when its structured terminal/idle event is
  observed, such as Codex `task_complete` or Claude `idle_notification`.
- First-officer work is observable through a structured FO tool call scoped to
  the exact entity, including reading or updating its file, building its
  dispatch assignment, or spawning its worker. A matching filesystem change may
  support that evidence but must not by itself claim FO identity.
- A first-officer turn completion ends `running · FO` unless a worker has
  started or a human gate is pending.
- Silent reasoning before an observable action is not detectable and must not
  be guessed as running.

When evidence overlaps, display precedence is:

```text
human-gate > running · worker > running · FO > idle
```

The task-list marker should render `human-gate` with red or equivalent
high-salience styling. Running entities should identify their handler without
adding more status values.

The existing session-evidence fields should be simplified for this model:

- `Agent` becomes `Runtime` and continues to identify Codex or Claude.
- `Session` remains and identifies the currently relevant worker or FO session.
  It is empty for `idle`; historical sessions may remain available in details
  but must not look like current handlers.
- `Confidence` is removed from the user-facing activity display. Recognized
  structured events determine the state; scanner or parser uncertainty is
  reported separately rather than becoming another activity status.
- `Status` renders exactly `idle`, `running · worker`, `running · FO`, or
  `human-gate`, backed by only the three domain status values.
- `Latest` becomes `Updated`, meaning the time of the latest relevant activity
  or state-transition event.

Acceptance criteria:

- AC-1: The visible domain status has exactly three values: `idle`, `running`,
  and `human-gate`.
- AC-2: `running` carries a typed `worker` or `FO` handler and renders it inside
  the status as `running · worker` or `running · FO`; there is no separate
  handler column and the other statuses have no handler suffix.
- AC-3: Initial state and any state with no observable handler or pending human
  decision classify as `idle`.
- AC-4: A worker session accepting the entity's exact dispatch assignment
  classifies as `running · worker` until a structured terminal/idle event.
- AC-5: Observable FO actions scoped to the exact entity classify as
  `running · FO` before dispatch and during the worker-to-FO handoff window.
- AC-6: The shape or plan pins concrete Codex and Claude start, terminal/idle,
  FO-turn, and human-gate record patterns without leaking transcript content.
- AC-7: Detection works without changing the Spacedock plugin. General path
  mentions, process names, dispatch-file existence, and filesystem writes alone
  are insufficient to claim a handler.
- AC-8: `human-gate` requires evidence that the FO is waiting for an
  approve/reject decision; merely mentioning approval or occupying a
  gate-marked workflow stage is insufficient.
- AC-9: Rendering follows
  `human-gate > running · worker > running · FO > idle`, uses high-salience
  styling for `human-gate`, and exposes the compact fields `Runtime`,
  `Session`, `Status`, and `Updated` without a user-facing `Confidence` or
  `Handler` field.
- AC-10: Tests cover the normal lifecycle, pre-dispatch FO activity,
  worker-to-FO handoff, immediate next-worker dispatch, overlapping evidence,
  false-positive mentions, and unavailable or malformed session artifacts.
- AC-11: Verification includes focused core tests, rendering tests, and
  `make lint`, or records blockers.

## Plan

### Outcome and boundaries

Replace the current inferred liveness model with a small structured-event
reducer. The reducer reads existing local Codex and Claude session artifacts,
attributes only exact entity-scoped events, and exposes one current activity
value per entity. Session logs and workflow files remain read-only. No Spacedock
plugin, workflow schema, status command, process hook, or markdown writer is
added.

The filesystem watcher and the existing two-second session scan remain refresh
triggers. A file change can cause a rescan and can corroborate the timestamp of
an already-scoped tool action, but a path mention, process name, dispatch-file
existence, mtime, or workflow-file write never creates worker or FO identity by
itself.

### Typed domain and display contract

In `crates/spacetop-core/src/domain/mod.rs`, replace the visible
`AgentKind`/`AgentSessionState`/`AttributionConfidence` model with:

```rust
pub enum AgentRuntime {
    Codex,
    ClaudeCode,
}

pub enum ActivityHandler {
    Worker,
    FirstOfficer,
}

pub enum EntityActivity {
    Idle {
        updated_unix: Option<i64>,
    },
    Running {
        handler: ActivityHandler,
        runtime: AgentRuntime,
        session_id: String,
        updated_unix: i64,
    },
    HumanGate {
        runtime: AgentRuntime,
        session_id: String,
        updated_unix: i64,
    },
}
```

`EntityActivity::status()` returns only `Idle`, `Running`, or `HumanGate`.
Only `Running` carries `ActivityHandler`; this makes an idle/human-gate handler
or a running state without a handler unrepresentable. `status_label()` renders
exactly:

- `idle`
- `running · worker`
- `running · FO`
- `human-gate`

`EntityActivity` also owns the current display facts:

- `Runtime`: `Codex` or `Claude Code` for running/human-gate; `—` for idle.
- `Session`: the current worker or FO session id/name for
  running/human-gate; `—` for idle. Historical sessions must not populate the
  current field.
- `Status`: one of the four labels above, backed by the three status variants.
- `Updated`: the latest relevant activity or state-transition timestamp,
  rendered with the existing relative-time formatter; `—` only when the entity
  has never had recognized activity.

`Confidence`, `Agent`, `Latest`, and a separate `Handler` field disappear from
the user-facing model. Parser/scanner uncertainty remains in
`SessionScanReport.errors` and never becomes a fourth status.

### Concrete event rules

All string inspection below is limited to known fields inside a parsed JSON
record. The parser extracts paths, canonical worker names, ids, event kinds,
tool names, and timestamps, then discards prompt/transcript text.

#### Codex

- **Worker start:** require one child rollout whose
  `session_meta.payload.source.subagent.thread_spawn` names a canonical
  `.../spacedock_ensign_<entity-slug>_<stage>` agent path and parent thread,
  whose structured user assignment contains the exact
  `/tmp/spacedock-dispatch/spacedock-ensign-<entity-slug>-<stage>.md` path, and
  whose `event_msg.payload.type == "task_started"` has appeared. The child
  rollout id is the displayed Session. A generic entity/workflow path mention
  without the child-session metadata and exact assignment is not a start.
- **Worker stop:** the same child rollout's
  `event_msg.payload.type == "task_complete"` for the open `turn_id` closes the
  worker. A parent `sub_agent_activity` record with
  `kind == "interrupted"` closes it only when `agent_thread_id` and
  `agent_path` match that exact child. Mere process disappearance or old mtime
  does not synthesize a stop.
- **FO start and scope:** `event_msg.payload.type == "task_started"` opens a
  candidate FO turn, but it remains invisible until the turn emits a structured
  `custom_tool_call`/`function_call` scoped to the exact entity. Accepted scopes
  are the exact entity state path, exact dispatch path, or a
  `collaboration.spawn_agent` call whose parsed `task_name` and assignment path
  both resolve to the entity. This covers reading/updating the entity, building
  its assignment, and spawning the worker without treating silent reasoning as
  activity.
- **FO stop:** `event_msg.payload.type == "task_complete"` for that FO
  `turn_id` closes `running · FO`. If a linked worker is still running, the
  worker already wins by precedence; when that worker completes while the
  scoped FO turn remains open (for example, while `wait_agent` is active), the
  visible state naturally returns to `running · FO` for handoff processing.
- **Human gate:** in an exact-entity-scoped FO turn, an outstanding
  `response_item` `function_call` named `request_user_input` is a gate only when
  its parsed `questions` contain a gate id/header and mutually exclusive option
  labels from both an accept set (`approve`, `pass`, `accept`) and a reject set
  (`reject`, `bounce back`). It remains pending until the same `call_id` receives
  `function_call_output` or the turn is aborted. Approval words in ordinary
  messages or tool arguments are not gate evidence.

#### Claude Code

- **Worker start:** require a parent, non-sidechain FO record with an
  `assistant.message.content[]` `tool_use` named `Agent` whose structured
  `input.name` is the canonical `spacedock-ensign-<entity-slug>-<stage>` name
  and whose `input.prompt` carries the exact dispatch assignment. Correlate that
  call with the child `subagents/*.meta.json`
  (`taskKind == "in_process_teammate"` and matching `name`) and child JSONL
  (`isSidechain == true`, matching non-empty `agentId`). The first child
  `assistant` record after the assignment is the observable acceptance/start;
  the child `agentId` is the displayed Session.
- **Worker stop:** parse only the structured teammate envelope delivered to the
  linked parent session: an injected user record containing a
  `<teammate-message>` JSON object with
  `type == "idle_notification"`, matching `from`, and
  `idleReason == "available"`. This closes the matched worker. A worker's final
  text or `SendMessage` call alone is not terminal.
- **FO start and scope:** for a non-sidechain session, the first structured
  `assistant` `tool_use` in the current request whose known input field names
  the exact entity/dispatch path starts `running · FO`. Recognized examples are
  `Read`/`Edit`/`Write.input.file_path`, an exact path token in `Bash.input.command`,
  and the exact `Agent.input.name` + `input.prompt` dispatch pair. The linked
  `idle_notification` scopes the ensuing handoff request to the entity; the FO
  becomes visible when its first subsequent assistant/tool record appears, not
  from hidden reasoning.
- **FO stop:** an assistant record with
  `message.stop_reason == "end_turn"` closes the scoped FO request unless a
  worker or human gate remains pending.
- **Human gate:** an outstanding `assistant` content block with
  `type == "tool_use"` and `name == "AskUserQuestion"` is a gate only when a
  question header names a gate and its structured option labels contain both
  accept and reject result classes above. It remains pending until a `user`
  `tool_result` with the same `tool_use_id` arrives. Normal approval prose and
  an entity occupying a gate-marked workflow stage do not qualify.

The event shapes above were checked against redacted local schemas on
2026-07-27: Codex child rollouts exposed `thread_spawn`, `task_started`, and
`task_complete`; the parent exposed structured `spawn_agent` plus scoped tool
calls. Claude artifacts exposed `Agent` calls, sidechain `agentId`/teammate
metadata, `idle_notification`, `AskUserQuestion`/`tool_result`, and
`stop_reason == "end_turn"`. No transcript bodies are copied into this plan or
the future fixtures.

### Transition reducer and precedence

Add a pure reducer that consumes timestamped typed events and keeps open
workers, open scoped FO turns, and pending human gates per entity. At every scan
it selects:

```text
human-gate > running · worker > running · FO > idle
```

Detailed transitions:

1. No recognized open evidence starts at `idle`.
2. The first scoped FO tool action starts `running · FO`.
3. An accepted worker starts `running · worker`, hiding (not deleting) an
   already-open FO turn.
4. A worker terminal/idle event reveals the still-open FO turn for handoff; if
   no scoped FO turn is open, the entity becomes `idle`.
5. An immediate next accepted worker wins without exposing stale worker or FO
   identity.
6. An unresolved gate request wins over both worker and FO evidence.
7. Resolving the gate removes it, then the remaining worker/FO evidence is
   re-evaluated; if none remains, the entity becomes `idle`.
8. If multiple same-precedence sessions exist, choose the one with the latest
   relevant event timestamp, then runtime/session id as a deterministic
   tie-breaker.

`Updated` is the timestamp of the latest relevant event for the selected state:
gate request/result, worker start/activity/terminal, or scoped FO
action/end-turn. A transition back to idle retains the terminal/end-turn time.

Missing session roots produce initial idle. A malformed record is skipped and
reported while earlier valid transitions from the same file remain usable. A
whole-scan/root IO failure preserves the last successful activity snapshot and
surfaces a scan warning instead of clearing it into a false idle; before any
successful scan the default remains idle. A deleted/truncated session file
invalidates its cached facts and is reparsed or removed.

### Parser, scanner, app, and UI ownership

1. **Structured parser and reducer (`spacetop-core`).**
   - Add `serde_json = "1"` to `crates/spacetop-core/Cargo.toml`; parsing JSONL
     structurally is the proven complexity/correctness benefit and avoids the
     current substring JSON parsing.
   - Keep root walking/cursor orchestration in
     `crates/spacetop-core/src/session_activity.rs`.
   - Add `crates/spacetop-core/src/session_activity/events.rs` for the
     privacy-safe internal event enum,
     `session_activity/codex.rs` and `session_activity/claude.rs` for
     runtime-specific record parsing, and `session_activity/reducer.rs` for the
     pure transition/precedence logic.
   - Replace the whole-file `MAX_SCAN_FILE_BYTES` skip with streaming JSONL
     parsing. Cache a per-file byte cursor plus safe parsed summary; append-only
     scans parse new complete lines, while truncation/replacement reparses from
     byte zero. Never cache raw transcript text.

2. **Domain/index/query (`spacetop-core`).**
   - Put `AgentRuntime`, `ActivityHandler`, `EntityActivity`, and report/error
     types in `crates/spacetop-core/src/domain/mod.rs`.
   - Update `crates/spacetop-core/src/index.rs` to index one current activity
     value per entity and expose it through `crates/spacetop-core/src/query.rs`;
     remove active-marker decisions based on raw evidence vectors.

3. **Background/app state (`spacetop`).**
   - Update `crates/spacetop/src/app/session_activity_worker.rs` to pass the
     per-file cursors/summaries between scans.
   - Update `crates/spacetop/src/app/overview.rs`, `crates/spacetop/src/app.rs`,
     and `crates/spacetop/src/lib.rs` only for typed report application,
     last-success preservation, and the existing poll/rescan lifecycle. The UI
     must not parse records or infer precedence.

4. **Ratatui (`spacetop`).**
   - In `crates/spacetop/src/ui/list.rs`, replace the boolean green-dot marker
     with a fixed activity column that renders the exact status label. Style
     `idle` dim, both running labels as active, and `human-gate` bold red (or the
     theme's equivalent highest-salience red).
   - In `crates/spacetop/src/ui/preview.rs`, replace the current
     `agent/session/confidence/state/latest` line with exactly
     `Runtime`, `Session`, `Status`, and `Updated`. Use em dashes for the two
     current-handler fields on idle; do not fall back to historical evidence.
   - Keep transcript bodies and parser diagnostics out of the task row/preview
     metadata. Existing scan warnings remain the separate diagnostic surface.

5. **Current documentation.**
   - Update `README.md` active-session wording and `AGENTS.md` Current Product
     Shape/Code Map to describe structured three-state activity and the
     read-only boundary.
   - Add a short supersession note to
     `docs/superpowers/specs/2026-06-11-spacetop-agent-session-detection-design.md`
     so its old confidence/recent/stale proposal remains historical rather than
     looking current. Do not rewrite historical workflow artifacts.

### Lowest-layer fixtures and proof

Add sanitized fixtures under `tests/fixtures/session_activity/` containing only
ids, paths, timestamps, tool/event names, and placeholder text:

- Codex child assignment/start/complete and interrupted-worker records.
- Codex FO pre-dispatch/spawn/wait/handoff/end-turn records.
- Codex pending/resolved gate records.
- Claude `Agent` parent + sidechain worker meta/JSONL + matching
  `idle_notification`.
- Claude scoped FO/end-turn and pending/resolved `AskUserQuestion` records.
- Malformed JSON, truncated append, deleted file, unrelated path mention, and a
  streamed fixture larger than the old 1 MB whole-file cap.

Tests at the lowest practical layer:

- `session_activity/codex.rs` and `claude.rs`: exact positive record shapes plus
  rejection of ordinary text mentions, wrong slug/stage, mismatched
  call/session ids, dispatch-file existence, process names, and gate-stage or
  approval prose.
- `session_activity/reducer.rs`: initial idle, pre-dispatch FO, normal
  FO→worker→FO→idle lifecycle, worker-to-FO handoff, immediate next worker,
  human-gate resolution, precedence overlap, deterministic same-level choice,
  and Updated timestamps.
- `session_activity.rs`: incremental append, truncation/deletion, unavailable
  roots, malformed records, scan error accumulation, and large-file streaming.
- `index.rs`/`query.rs`: one query-owned current activity value per entity and
  idle default.
- `crates/spacetop/src/app/tests.rs`: matching/stale worker results, preserving
  the last successful state on scan failure, and applying a later transition.
- Ratatui `TestBackend` tests in
  `crates/spacetop/src/ui/tests/task_list.rs` and
  `crates/spacetop/src/ui/tests/preview.rs`: all four exact labels, worker vs FO
  suffix, red/high-salience human gate, idle current fields empty, field labels
  exactly `Runtime`/`Session`/`Status`/`Updated`, and no
  `Confidence`/`Handler`/transcript leakage.

### Required live-log falsification spike

Before the implementation commits to the reducer, run one schema-only live
capture for each runtime and store only a redacted shape summary in the
implementation report:

1. Codex: observe one FO scoped tool → child `task_started` → child
   `task_complete` → FO continuation/end sequence and verify the parent/child
   ids, `agent_path`, and `turn_id` linkage.
2. Claude: observe one `Agent` call → sidechain child first assistant event →
   matching parent `idle_notification` → FO `end_turn` sequence and verify
   `input.name`, meta `name`/`agentId`, and notification `from` linkage.
3. For both runtimes, inspect an outstanding gate tool call before its result is
   appended. Confirm call-id correlation and the gate header/options shape. If
   a runtime does not emit the pinned shape, fail closed for that shape (never
   infer `human-gate`) and adjust its parser/fixture before proceeding; do not
   add a Spacedock hook.

The spike should use `jq` projections that print only record types, key names,
tool/event names, ids, timestamps, canonical agent names, and dispatch
basenames. It must not print prompts, responses, tool bodies, or gate question
text.

### Implementation and verification sequence

1. Capture the redacted live sequences and freeze sanitized fixtures.
2. Add the typed domain plus pure reducer tests.
3. Add Codex/Claude structured parsers and streaming cursor tests.
4. Replace index/query and app-worker plumbing; prove scan failures preserve
   the last successful state.
5. Update list/preview rendering and Ratatui tests.
6. Update README, AGENTS, and the historical-design supersession note.
7. Run:

```bash
cargo fmt
cargo test -p spacetop-core session_activity
cargo test -p spacetop task_list
cargo test -p spacetop preview
cargo test
make lint
```

No real watcher-backend behavior changes, so `cargo test -- --ignored` is not
required unless implementation changes watcher filtering rather than only the
session poll/parser path.

## Stage Report: plan

- DONE: Pin concrete Codex and Claude structured-event rules for idle, running · worker, running · FO, and human-gate without Spacedock plugin changes.
  The plan maps exact Codex child/turn/tool/gate records and exact Claude Agent/sidechain/idle/end-turn/gate records into a fail-closed reducer. Path mentions, processes, mtimes, dispatch existence, workflow gate stages, and approval prose are explicitly non-signals.
- DONE: Name exact domain, parser, app, and UI ownership plus transition precedence and the Runtime/Session/Status/Updated display contract.
  Domain types live in `domain/mod.rs`; runtime parsers and reducer under `session_activity/`; index/query own lookup; the background worker carries cursors; Ratatui renders exact labels and the four compact fields. Precedence is `human-gate > running · worker > running · FO > idle`.
- DONE: Specify the lowest-layer fixtures, tests, commands, and any live-log spike needed to falsify risky attribution and handoff assumptions.
  The plan names sanitized Codex/Claude fixtures, parser/reducer/scanner/index/app/TestBackend cases, focused and full verification commands, and a required schema-only live spike for parent-child, handoff, and pending-gate correlation.

### Summary

Planned a domain-first replacement for confidence/PID/mtime inference: parse
only structured Codex and Claude session events, reduce them to the three valid
activity states with typed worker/FO handling, and render current
Runtime/Session/Status/Updated facts without transcript content. The proof path
starts with a redacted live linkage spike, then freezes sanitized fixtures and
tests parser, reducer, incremental scan, app, and Ratatui behavior before full
`cargo test` and `make lint`.

## Stage Report: implement

- DONE: Implement the typed three-state reducer and structured Codex/Claude event parsing so exact worker, FO, handoff, and human-gate transitions fail closed without plugin changes.
  Commit `efcb0ef` replaces liveness inference with `EntityActivity` and a precedence reducer; 12 focused core tests would fail if canonical dispatch/session correlation, terminal events, handoff, gate balance, malformed-record handling, or false-positive rejection regressed.
- DONE: Integrate current activity through index/app/UI and render only Runtime, Session, Status, and Updated with exact running · worker / running · FO labels and precedence.
  Index/app preserve the last successful snapshot on scan failure; Ratatui tests would fail if handler text moved outside Status, the four compact fields changed, or human-gate lost its bold red marker and label.
- DONE: Add sanitized fixtures and lowest-layer parser/reducer/scanner/app/Ratatui coverage, update current docs, and finish with cargo test plus make lint evidence.
  Sanitized Codex and Claude start/stop/FO/gate fixtures contain structural records only; final `cargo test` passed 374 spacetop and 180 core unit tests plus integration/doc tests, and `make lint` passed with warnings denied.

### Summary

Implemented structured, read-only entity activity detection with exact three-state
domain semantics and deterministic precedence across worker, first-officer, and
human-gate evidence. Updated the list, preview, app failure behavior, fixtures,
and current product/design docs without changing the Spacedock plugin or workflow
write boundary.

## Stage Report: verify

- DONE: Independently challenge all eleven acceptance criteria, especially exact structured event linkage, fail-closed false positives, FO/worker handoff, and human-gate precedence without plugin changes.
  Verdict: REJECTED. AC-1/2/3/8/9 pass and AC-11 gates pass; AC-4/5/6/7/10 fail on dropped large logs, an unparsed live Codex FO tool shape, cross-session Claude linkage, and missing falsification coverage.
- DONE: Review the implementation diff for typed domain/parser/index/app/UI ownership, read-only boundaries, and exact Runtime/Session/Status/Updated rendering with no visible Confidence or Handler field.
  Commit `efcb0ef` keeps typed state in core, preserves read-only boundaries, and passes exact field/label styling tests, but the structured scanner has the blocking linkage defects below.
- FAILED: Preserve worker activity when a session artifact grows beyond the scanner threshold.
  `session_activity.rs:289-310` silently skips files over 4 MB, so a running entity becomes false-idle; this machine has 11 Codex and 8 Claude artifacts already above that limit. Stream JSONL or retain a safe parsed summary and test large append/truncation/deletion.
- FAILED: Recognize the current Codex structured FO tool-call shape before dispatch.
  Live schema projection shows `custom_tool_call` name `exec` with string input, while `session_activity.rs:800-874` only scopes object fields; the fixture uses `function_call` `Read`, so normal pre-dispatch entity reads/build steps do not produce `running · FO`. Pin and parse the live nested shape without transcript content.
- FAILED: Correlate Claude worker start/idle evidence to the exact parent dispatch.
  `session_activity.rs:604-680` keys teammate metadata globally by reusable worker name and accepts a same-name idle notification from any parent file, allowing one session to start or stop another session's worker. Preserve parent-session/call linkage and add a two-parent same-name false-positive test.
- DONE: Run the required focused and full test/lint gates, report reproducible evidence, and issue an explicit PASSED or REJECTED verdict with actionable defects.
  `cargo fmt --check`, 12 focused core tests, 32 task-list tests, 71 preview tests, full `cargo test` (374 app, 180 core plus integration/doc tests), and `make lint` all passed; the verdict remains REJECTED because passing tests do not cover the failures above.

### Summary

REJECTED pending three scanner fixes: remove the false-idle large-file cutoff,
support the observed Codex FO call schema, and keep Claude worker lifecycle
events linked to their exact parent dispatch. Domain/UI rendering and all
required test/lint gates pass, but the current evidence reducer is not yet
fail-closed or complete for representative runtime artifacts.

### Feedback Cycles

- Cycle 1: REJECTED — verify; surface 3 blocking scanner defects vs estimate not declared (n/a%); AC unchanged
- Cycle 2: REJECTED — verify; surface 3 blocking Codex linkage/action-extraction and scan-cursor defects vs estimate not declared (n/a%); AC unchanged
- Cycle 3: REJECTED — verify; surface 2 blocking live-exec parsing and same-size rewrite-proof defects vs estimate not declared (n/a%); AC unchanged

## Stage Report: implement (cycle 2)

- DONE: Remove the 4 MB false-idle cutoff by streaming or preserving safe parsed state, with large append/truncation/deletion regression coverage.
  Commit `d31c169` removes the size skip and reads JSONL incrementally through
  `BufReader`, projects only reducer fields, and discards transcript text.
  A greater-than-4-MB regression observes a running worker, appends its terminal
  event, truncates it to an open worker, then deletes it.
  This test would fail if large files were skipped, appended terminal records
  were missed, a truncated file reused stale state, or deletion preserved stale
  activity.

- DONE: Parse the observed Codex custom_tool_call exec string/nested shape for exact entity-scoped FO activity without retaining transcript content.
  The Codex projection now accepts `custom_tool_call` records named `exec` (and
  the equivalent Claude `Bash` shape), extracts raw, JSON-string, and nested
  command inputs, and scopes only an exact entity path or validated Spacedock
  dispatch marker.
  Two sanitized fixtures pin raw-string and nested-object `exec` inputs; a
  projection test proves unrelated transcript text is not retained.
  These tests would fail if the observed call shape stopped opening
  `running · FO`, if substring path collisions became accepted, or if transcript
  bodies leaked into the parsed record set.

- DONE: Scope Claude worker start and idle events to the exact parent dispatch, including a two-parent same-name false-positive test.
  Claude teammate metadata now carries parent session and optional parent call
  identity, child evidence must live under that parent's `subagents` directory,
  and idle notifications are accepted only from the exact parent session.
  Missing call identity falls back only for a unique parent/name dispatch, while
  ambiguous same-parent names fail closed for start and name-only idle evidence.
  The two-parent fixture would fail if parent B's reused worker name or idle
  notification affected parent A; two same-parent tests reject ambiguous calls.

### Verification

`cargo test -p spacetop-core session_activity` passed all 18 focused tests.
Final `cargo test` passed 374 app and 186 core unit tests plus all integration
and doc tests; the three real watcher-backend tests remain intentionally
ignored. `make lint` passed with all warnings denied, and `git diff --check`
passed before commit.

### Summary

Cycle 2 closes all three verifier blockers without changing domain or UI
semantics: large structured logs are streamed without a false-idle cutoff,
observed Codex `exec` calls are recognized with bounded structural projection,
and Claude lifecycle evidence is correlated by parent session and dispatch call
with ambiguous name-only cases rejected.

## Stage Report: verify (cycle 2)

- DONE: Independently challenge all eleven acceptance criteria, especially exact structured event linkage, fail-closed false positives, FO/worker handoff, and human-gate precedence without plugin changes.
  Verdict: REJECTED. AC-1/2/3/5/8/9 pass and AC-11 gates pass; AC-4/6/7/10 still fail on missing Codex parent linkage, whole-program `exec` attribution, and the unimplemented scan cursor.
- DONE: Re-review implementation commit d31c169 against the three blocking findings from the prior verify pass.
  The 4 MiB cutoff is removed, current `exec` inputs now produce FO activity, and Claude same-name workers are parent/call scoped; the new focused regressions for those cases pass.
- FAILED: Require Codex worker evidence to carry its structured parent-thread linkage.
  `session_activity.rs:418-428` projects `parent_thread_id`, but `session_activity.rs:696-714` never reads it; a child-shaped file with canonical path, assignment, and `task_started` claims `running · worker` even when the parent link is absent. Require the non-empty parent id and add a negative fixture.
- FAILED: Parse the live Codex code-mode `exec` shape without treating a general path mention as an action.
  Live projection is a JavaScript module with nested `tools.exec_command` and `text(...)` calls, while `session_activity.rs:563-571,1203-1206` retains and searches the whole module string. An exact entity path in non-executing `text(path)` therefore starts `running · FO`; extract only nested command arguments and test this false-positive shape.
- FAILED: Use the specified per-file cursor and safe summary for periodic large-log scans.
  `session_activity.rs:363-407` reparses and collects every record on every scan, and `previous_session_files` is never read despite a two-second poll. The configured roots here contain 1,595 artifacts / 617.9 MiB, so the current implementation repeatedly parses the whole corpus; cache byte offsets/projected summaries and prove unchanged scans avoid rereads while append/truncate/delete stay correct.
- DONE: Review the implementation diff for typed domain/parser/index/app/UI ownership, read-only boundaries, and exact Runtime/Session/Status/Updated rendering with no visible Confidence or Handler field.
  Cycle 2 preserves the previously accepted typed domain, index/app integration, read-only boundaries, exact labels, and high-salience human-gate rendering.
- DONE: Run the required focused and full test/lint gates, report reproducible evidence, and issue an explicit PASSED or REJECTED verdict with actionable defects.
  `cargo fmt --check`, 18 focused core tests, 32 task-list tests, 71 preview tests, full `cargo test` (374 app, 186 core plus integration/doc tests), `git diff --check`, and `make lint` all passed; coverage does not exercise the three failures above.

### Summary

REJECTED again: commit `d31c169` closes the three original test cases, but the
scanner still accepts unlinked Codex workers and non-action `exec` path mentions,
and it ignores the required cursor while reparsing a large local corpus every
two seconds. Domain/UI behavior and all required gates remain sound.

## Stage Report: implement (cycle 3)

- DONE: Require non-empty Codex parent_thread_id linkage for worker evidence and add a negative unlinked-child fixture.
  Commit `ff6dfb8` now reads the projected `parent_thread_id` and requires it to
  be non-empty alongside the canonical agent path, exact dispatch assignment,
  and structured `task_started`.
  The new `codex-worker-unlinked` fixture has every worker-shaped field except a
  parent id and must remain idle. It would fail if detached child-shaped logs
  could again claim `running · worker`.

- DONE: Extract only executable nested command arguments from current Codex code-mode exec modules so non-action text(path) mentions fail closed.
  Code-mode projection retains only `cmd`/`command` arguments parsed from
  balanced `tools.exec_command(...)` calls, including the observed nested input
  envelope; it does not retain or search the JavaScript module body.
  Positive raw and nested fixtures execute an entity-scoped command. The
  negative fixture executes only `pwd` while sending the entity path to
  `text(...)`, and must remain idle.
  These cases would fail if current code-mode calls stopped opening FO activity
  or any non-executing module text could scope the entity.

- DONE: Implement and test per-file byte cursors with safe projected summaries so unchanged scans avoid full rereads while append, truncate, and delete stay correct.
  Each session snapshot now carries exact modification metadata, file length,
  safe byte cursor, line count, a small cursor checkpoint, projected records,
  and parse errors.
  Unchanged files reuse that summary with no parse. Appends verify the checkpoint
  and resume at the cursor; same-size rewrites and truncations rebuild from byte
  zero; deletions discard the snapshot.
  Instrumented tests prove read starts of `0 → prior cursor → 0` for the large
  append/truncate sequence, no read for the unchanged scan, and no stale state
  after deletion. They would fail if the two-second poll reparsed unchanged
  files or cursor invalidation missed a rewrite.

- DONE: Preserve accepted domain/UI behavior and complete focused tests, full cargo test, cargo fmt check, git diff check, and make lint.
  No domain, index, app, or UI semantics changed. Exact
  Runtime/Session/Status/Updated rendering and human-gate precedence remain
  covered by the repository suite.
  `cargo fmt --all -- --check`, all 20 focused session-activity tests, final
  `cargo test` (374 app and 188 core unit tests plus integration/doc tests),
  `git diff --check`, and `make lint` with warnings denied all passed.

### Summary

Cycle 3 closes the remaining scanner gaps: Codex workers require their parent
link, code-mode FO attribution comes only from nested executable command
arguments, and periodic scans reuse privacy-safe projected summaries through
validated per-file byte cursors.

## Stage Report: verify (cycle 3)

- DONE: Independently challenge all eleven acceptance criteria, especially exact structured event linkage, fail-closed false positives, FO/worker handoff, and human-gate precedence without plugin changes.
  Verdict: REJECTED. AC-1/2/3/4/7/8/9 pass and AC-11 gates pass; AC-5/6/10 still fail because the new Codex `exec` fixture does not match the live JavaScript argument syntax, and same-size rewrite invalidation remains unproved.
- DONE: Re-review implementation commit `ff6dfb8` against the three cycle-2 blockers.
  Non-empty Codex parent linkage and the per-file projected-summary cursor substantially close two blockers, and the non-action `text(path)` negative is correctly scoped; the positive `exec` regression is synthetic rather than representative of current live calls.
- FAILED: Parse the actual current Codex code-mode `exec` argument shape while continuing to ignore non-executing module text.
  `session_activity.rs:748-770` extracts a balanced `tools.exec_command(...)` argument but accepts it only through `serde_json::from_str`; live modules use JavaScript object literals such as `{cmd: "...", workdir: "..."}`, while `tests/fixtures/session-activity/codex-fo-exec/rollout.jsonl:3` uses JSON-shaped `{"cmd":"..."}`. Add a bounded parser for string-valued `cmd`/`command` properties in the actual JavaScript object-literal shape and retain the `text(path)` false-positive regression.
- FAILED: Prove the dispatched same-size rewrite case and make invalidation robust to unchanged metadata.
  `session_activity.rs:385-388` reuses the prior summary when length and modification time match, while the new regression coverage at `session_activity.rs:2127-2285` proves append, truncate, delete, and unchanged reuse but never rewrites the file at equal length. Add a same-size rewrite test that proves a byte-zero reparse; if equal-length replacement can preserve the observed timestamp, strengthen the cached fingerprint/checkpoint so stale activity cannot survive.
- DONE: Review the implementation diff for typed domain/parser/index/app/UI ownership, read-only boundaries, and exact Runtime/Session/Status/Updated rendering with no visible Confidence or Handler field.
  Cycle 3 changes only scanner/cache code, fixtures, and design documentation; the previously accepted typed domain, index/app integration, read-only boundaries, exact labels, and high-salience human-gate rendering remain intact.
- DONE: Run the required focused and full test/lint gates, report reproducible evidence, and issue an explicit PASSED or REJECTED verdict with actionable defects.
  `cargo fmt --all -- --check`, 20 focused session-activity tests, 32 task-list tests, 71 preview tests, full `cargo test` (374 app and 188 core unit tests plus integration/doc tests), `git diff --check`, and `make lint` all passed; those suites do not cover the two failures above.

### Summary

REJECTED: commit `ff6dfb8` fixes structured Codex parent linkage and removes
unchanged full-log rereads, but live Codex FO activity still fails because the
extractor accepts JSON object syntax rather than the current JavaScript object
literal. Verification also still lacks the explicitly requested same-size
rewrite regression and robust proof against equal metadata.
