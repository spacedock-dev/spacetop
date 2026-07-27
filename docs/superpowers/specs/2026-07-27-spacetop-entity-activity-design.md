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
  `parent_thread_id`, the exact dispatch assignment, and `task_started`;
  matching `task_complete` stops it.
- A Claude Code worker needs a canonical parent `Agent` call, correlated
  teammate metadata and sidechain acceptance; the matched
  `idle_notification` stops it. Correlation is scoped to the exact parent
  session directory and tool-use call; reusable worker names never link
  activity across parent sessions.
- FO activity starts only after an exact entity/dispatch-scoped structured tool
  call. For Codex code-mode `exec`, only command arguments inside nested
  `tools.exec_command(...)` calls count; module text and `text(path)` output do
  not. The corresponding turn/end-turn closes it.
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
streamed record by record without a size cutoff, and each record is projected
to the structural fields needed by the reducer before it is retained. Each
file snapshot carries its safe byte cursor and projected summary: unchanged
files reuse the summary, appends resume at the cursor, truncations rebuild it,
and deletions drop it. A small checkpoint guards cursor reuse when bytes
immediately before the cursor changed, so a large or changed artifact cannot
silently clear real activity.
