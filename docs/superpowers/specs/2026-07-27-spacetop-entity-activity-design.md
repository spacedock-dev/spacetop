# Spacetop Entity Activity Detection

**Status:** Implemented
**Date:** 2026-07-27

This design supersedes the confidence- and liveness-based user-facing model in
`2026-06-11-spacetop-agent-session-detection-design.md`.

## Contract

Spacetop derives one read-only activity value for each entity from existing
local Codex and Claude Code session artifacts:

- `idle`
- `running · worker`
- `running · FO`
- `human-gate`

The display labels are backed by three domain states: `Idle`, `Running`, and
`HumanGate`. Only `Running` carries a typed `Worker` or `FirstOfficer` handler.
Display precedence is:

```text
human-gate > running · worker > running · FO > idle
```

The preview exposes only `Runtime`, `Session`, `Status`, and `Updated`.
Confidence and a separate handler field are not part of the activity display.

## Evidence boundary

Detection parses structured JSON fields and fails closed:

- A Codex worker needs canonical child `thread_spawn` metadata with a non-empty
  `parent_thread_id`, matching repo/worktree `cwd`, and `task_started`. The
  dispatch join may be the legacy exact child assignment or the parent
  session's exact `sub_agent_activity(kind=started)` record with matching child
  rollout id and agent path; encrypted v2 assignment prose is not evidence.
  Matching `task_complete` or an exact parent interruption stops it.
- A Claude Code worker needs a canonical parent `Agent` call, correlated
  teammate metadata and sidechain acceptance with matching repo/worktree
  `cwd`; the matched `idle_notification` stops it. The dispatch basename may
  carry the exact parent-session prefix. Current teammate metadata may omit
  `agentId`; in that case the sibling sidechain supplies the non-empty id and
  its directory supplies the parent session. Correlation remains scoped to the
  exact parent directory and tool-use call, with the existing unique
  parent/name fail-closed rule when the call id is absent.
- Reusable Claude workers can reopen after an idle transition only when the
  correlated parent observes a later teammate-message boundary and the same
  sidechain `agentId` emits a subsequent assistant record. Every matching idle
  envelope is retained, so repeated handoffs reduce in timestamp order.
- FO activity starts only after an exact entity/dispatch-scoped structured tool
  call. For Codex code-mode `exec`, only command arguments inside nested
  `tools.exec_command(...)` calls count; direct structured `exec_command`
  calls are also executable evidence. Module text and `text(path)` output do
  not count. The corresponding turn/end-turn closes it.
- A human gate requires an outstanding `request_user_input` or
  `AskUserQuestion` call scoped to that FO turn, a gate id/header, and both
  accept and reject option classes.

Ordinary prose, path mentions, process names, mtimes, dispatch-file existence,
workflow stages, and filesystem writes never create activity on their own.
Malformed records are reported separately; valid earlier transitions remain
usable. A whole-scan failure preserves the last successful activity snapshot.

## Refresh and privacy

The existing filesystem watcher and periodic session scan trigger rescans.
Session logs and workflow files remain read-only. Fixtures contain only
sanitized structural records needed to pin the Codex and Claude Code schemas;
prompt and transcript bodies are not exposed in the UI. JSONL artifacts are
streamed record by record without a size cutoff and projected directly into
typed, privacy-safe facts.

A successful workflow snapshot reload preserves the last published activity
and scanner diagnostic only for active entities whose id and source path both
match the prior snapshot. Removed entities, new entities, and reused ids at a
different path inherit no prior attribution. A workflow reload is not a
lifecycle event; only the correlated structured session evidence below can
start, change, or stop activity.

`SessionScanState` crosses the background/app boundary. It contains
`SessionFileCursor` values and a `SessionEvidenceStore` keyed by stable runtime
session identity (falling back to a typed source identity until a session id is
available). Unchanged files reuse their cursors, appends resume at the saved
byte offset, and a checkpoint rejects stale cursor reuse. Truncation, rotation,
deletion, or a transient malformed metadata rewrite never deletes previously
observed facts and never synthesizes a stop; only an exact structured terminal
or idle fact closes open activity.

Each scan inventories `(path, length, modified)` before and after reading. If
the inventory changes, that generation is rejected, the last published report
and scan state remain in place, and the background worker requests an immediate
rescan. This prevents visit order from publishing a parent stop without a
concurrent child restart.
