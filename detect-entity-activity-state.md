---
title: Detect entity activity and human-gated sessions
status: plan
source: "Follow-up from task 067 and the 2026-07-27 three-state activity design refinement"
kind: feature
risk: medium
milestone: v1-maintenance
proof: Core activity-state tests with representative Codex and Claude session logs plus task-list rendering tests for status and handler
started: 2026-07-27T13:20:01Z
completed:
verdict:
score: 0.84
worktree:
issue:
pr:
id: 069
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
