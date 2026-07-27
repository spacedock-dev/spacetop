# Spacetop Agent Session Detection - Proposal

**Status:** Superseded by `2026-07-27-spacetop-entity-activity-design.md`
**Date:** 2026-06-11
**Author:** brainstorming session (captain: Kent)

## Summary

Spacetop should detect which AI agent session is handling each active Spacedock
task without requiring Spacedock to write new metadata into workflow markdown.
The feature stays inside Spacetop and remains read-only: it derives ownership
signals from local Codex and Claude Code session data, correlates those signals
with workflow task worktrees, and renders a confidence-rated attribution in the
TUI.

The result is not an authoritative assignment system. It is an inspection layer:
"this task is likely handled by Codex session X" or "this task has Claude Code
and Codex evidence, latest activity came from Y." Spacetop should make the
evidence visible enough that a maintainer can trust the label or understand why
it is uncertain.

## Chosen Scope

Spacedock will not provide agent ownership metadata for this feature. Spacetop
must handle detection itself from local machine evidence.

This means:

- No required `agent`, `agent_session`, or `owner` fields in task frontmatter.
- No Spacedock dispatch changes.
- No workflow markdown writes.
- No requirement that Codex and Claude Code cooperate at runtime.
- Detection works best when agents operate in per-task worktrees, because the
  workflow task already records the `worktree:` path.

## Goals

1. Detect whether an active task is handled by Codex, Claude Code, both, or
   unknown.
2. Show the best matching session id, agent kind, latest activity time, and
   confidence for each detected task.
3. Keep detection read-only and local-only.
4. Avoid broadening Spacetop's approved git write behavior.
5. Make ambiguous attribution explicit instead of pretending it is reliable.
6. Keep parser and domain facts typed before rendering them in the TUI.
7. Support future headless export without coupling detection to terminal UI
   code.

## Non-Goals

- No authoritative assignment or locking.
- No task claiming, task stealing, or workflow-state mutation.
- No process control for Codex or Claude Code.
- No dependence on Spacedock adding metadata.
- No remote telemetry or upload of agent transcripts.
- No parsing of private transcript content beyond the minimum local signals
  needed for attribution.
- No cross-machine coordination. Detection describes the local workstation's
  observed sessions only.

## Data Sources

### Workflow Task State

Spacetop already parses each task entity and its frontmatter. The primary join
key is the task's `worktree:` field, for example:

```yaml
worktree: .worktrees/spacedock-ensign-051-v2-p1-index-query-api
```

For each active task with a worktree, Spacetop should canonicalize:

- workflow root
- repo root
- task slug
- task file path
- worktree path
- worktree branch, if available through read-only git commands

Tasks without a worktree can still be shown as `unknown`, with a low-confidence
fallback only if session text directly references the task file or slug.

### Codex Sessions

Codex sessions are stored as local JSONL logs under `~/.codex/sessions/...`.
The useful signals are structured:

- `session_meta.payload.id`
- `session_meta.payload.agent_nickname`, when present
- `session_meta.payload.agent_role`, when present
- `response_item` tool calls, especially `exec_command.arguments.workdir`
- `apply_patch` paths
- timestamps on each event

The strongest Codex evidence is a tool call whose `workdir` is inside the task
worktree, or an `apply_patch` path under that worktree.

### Claude Code Sessions

Claude Code sessions are stored under `~/.claude/projects/...`, with project
directories encoded from the working directory path. The useful signals are:

- session id from the transcript or process resume id
- cwd/project directory derived from the session storage path
- tool calls that reference worktree paths, file paths, or commands
- timestamps on transcript entries

The strongest Claude Code evidence is a session rooted in the task worktree or
a command/file operation that references the task worktree.

### Live Processes

Process inspection can add freshness, but it should not be the primary source
of truth. A process list can show `codex`, `codex resume <id>`, or
`claude --resume <id>`, but process arguments often do not reveal the task. Use
processes only to mark a matched session as likely running.

If process inspection is unavailable due to sandboxing or permissions, Spacetop
should still render session-log-based detection.

## Attribution Model

Introduce a core-owned detection model:

```rust
pub enum AgentKind {
    Codex,
    ClaudeCode,
    Unknown,
}

pub enum AgentConfidence {
    High,
    Medium,
    Low,
}

pub struct AgentSessionAttribution {
    pub entity_id: String,
    pub entity_slug: String,
    pub agent_kind: AgentKind,
    pub session_id: Option<String>,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub confidence: AgentConfidence,
    pub latest_activity_at: Option<DateTime>,
    pub evidence: Vec<AgentEvidence>,
    pub running: Option<bool>,
}

pub struct AgentEvidence {
    pub source: AgentEvidenceSource,
    pub path: Option<PathBuf>,
    pub timestamp: Option<DateTime>,
    pub summary: String,
}
```

The actual Rust types can be adjusted to match the existing core conventions,
but the concept should stay stable: a task can have zero, one, or many
attributions, each backed by evidence.

## Confidence Rules

### High Confidence

Use `High` when a session has direct worktree evidence:

- Codex `exec_command.workdir` equals or is under the task worktree.
- Codex `apply_patch` path is under the task worktree.
- Claude Code session cwd is the task worktree.
- Claude Code tool call reads, writes, or runs commands under the task worktree.

### Medium Confidence

Use `Medium` when a session has direct task evidence but not a worktree match:

- transcript references the task file path;
- transcript references the task slug and workflow path;
- command output from `git branch --show-current` matches the task branch;
- the session touched a known task artifact, such as the task plan file, but did
  not operate under the worktree.

### Low Confidence

Use `Low` when only weak textual evidence exists:

- task slug appears in the first prompt;
- process command includes a related branch or task id;
- session operates in the repo root and mentions the task id.

Low-confidence evidence should not produce a strong "handled by" label. Render
it as "possible" or put it behind a details view.

## Conflict Handling

Multiple sessions can match one task. Spacetop should not collapse them into a
single owner unless one session is clearly stronger.

Recommended display logic:

- If exactly one high-confidence attribution exists, show that agent/session in
  the task list.
- If multiple high-confidence attributions exist, show `multi-agent` with the
  latest active session and expose all matches in details.
- If one high-confidence and several lower-confidence attributions exist, show
  the high-confidence session and list the lower-confidence sessions in details.
- If only medium/low matches exist, show `possible Codex`, `possible Claude`, or
  `uncertain`.
- If no matches exist, show `unknown`.

This preserves honesty. A task can genuinely be handled by both Codex and
Claude Code if one agent implemented and another reviewed or continued work.

## Architecture

### Core Layer

Add the detection logic to `spacetop-core`, not the TUI:

- `agent_sessions` module for scanning local session stores.
- `AgentSessionSource` trait or equivalent small boundary for Codex and Claude
  scanners.
- typed session summaries with only the fields needed by Spacetop.
- correlation logic from task worktrees to session evidence.

The scanners should be pure enough to test with fixture directories. They
should accept explicit roots for tests and default to user-local paths in
production.

### App State

The app layer should hold detection state separately from workflow parsing:

- workflow snapshot/index loads immediately;
- agent-session detection can run after load and on refresh;
- failures are non-fatal warnings;
- stale detection can remain visible while a background refresh runs.

Detection should not block first render. Session logs can grow large, so a
background worker plus channel fits the existing watcher/history direction.

### UI

The task list should gain a compact agent indicator only when useful:

- `Codex Huygens`
- `Claude 55f39f9d`
- `multi-agent`
- `unknown`

The preview/details pane should show the evidence:

- agent kind
- session id
- nickname/role if known
- latest activity
- confidence
- evidence rows such as `exec_command workdir matched task worktree`

The UI must not show private transcript body text. Evidence summaries should be
derived from structural signals, not copied transcript content.

### Headless Export

If P5 headless/export is implemented, include attribution as optional query
data:

```json
{
  "agent_attributions": [
    {
      "agent_kind": "codex",
      "session_id": "019eb571-0a4f-7882-b8e6-8211d9d54693",
      "confidence": "high",
      "latest_activity_at": "2026-06-11T06:59:10Z"
    }
  ]
}
```

## Privacy And Safety

Agent session logs can contain sensitive prompt and transcript content.
Spacetop must treat them as local private data.

Rules:

- Read only from local session stores.
- Do not upload or transmit session data.
- Do not render full prompts, messages, or command output in the TUI.
- Store only minimal derived evidence in memory.
- Do not write derived attribution back into workflow markdown.
- Provide a setting or CLI flag to disable session detection if users do not
  want Spacetop reading agent logs.

Suggested defaults:

- Enabled when local session stores are readable.
- Non-fatal warning if unreadable.
- Future config can allow `agent_detection = false`.

## Failure Modes

| Failure | Expected Behavior |
|---------|-------------------|
| Codex logs missing | show Claude matches if any; otherwise `unknown` |
| Claude logs missing | show Codex matches if any; otherwise `unknown` |
| session log parse error | skip bad record, show warning count |
| process listing denied | omit running marker; keep log-based attribution |
| task has no worktree | use only medium/low evidence; usually `unknown` |
| multiple sessions match | show `multi-agent` or strongest match with details |
| stale old session matches | rank by latest activity and show timestamp |
| same task id appears in another repo | require repo/worktree path match for high confidence |

## Ranking

When several attributions match, rank by:

1. confidence (`High` before `Medium` before `Low`);
2. latest activity time;
3. direct worktree path match before task-file text match;
4. running process marker, if available.

This keeps old but exact evidence from hiding a newer active session only when
the newer session is equally or nearly equally strong.

## Performance

Session directories can grow over time. The first version should avoid full
unbounded rescans on every refresh.

Recommended approach:

- scan only likely project/session directories where possible;
- for Codex, index recent JSONL files first and match `workdir` paths;
- for Claude Code, scan project directories whose encoded cwd starts with the
  repo root or task worktree path;
- cache per-session summaries by file path plus modified time;
- refresh attribution in the background.

The first implementation can use a simple bounded scan by recency, then add
caching if profiling shows visible delay.

## Testing Strategy

Lowest practical layer:

- Core scanner tests with fixture Codex JSONL logs.
- Core scanner tests with fixture Claude Code transcripts/directories.
- Correlation tests for task worktree to session attribution.
- Conflict/ranking tests for multi-agent tasks.
- App tests for non-blocking detection update and warning handling.
- Ratatui render tests for list/detail labels.

Guardrails:

- `spacetop-core` still has no terminal dependencies.
- No production path writes workflow markdown.
- Session parsing failures do not crash workflow load.

Manual verification:

- Run Spacetop against `docs/spacetop-dev`.
- Confirm current implement tasks show Codex matches when local logs are
  available.
- Confirm unavailable Claude/Codex logs degrade to `unknown` without crashing.

## Rollout Plan

### Phase 1 - Evidence Scanner

Add core scanners for Codex and Claude Code session summaries using fixture
logs. No UI yet.

### Phase 2 - Correlation

Correlate active workflow entities with session summaries through canonical
worktree paths and confidence rules.

### Phase 3 - App Integration

Run attribution as a non-blocking background refresh and store results in app
state.

### Phase 4 - UI

Render compact agent labels in the task list and evidence details in preview or
a dedicated details pane.

### Phase 5 - Export

Expose attribution through query/export surfaces after the headless API exists.

## Open Decisions

### How much UI surface should the first version expose?

Recommendation: compact label in the list plus evidence in details. Do not add
a new mode until users need cross-task filtering.

### Should detection be enabled by default?

Recommendation: yes for local-only read access, with a clear disable option once
config exists. The feature is an inspection tool, not telemetry.

### How far back should Spacetop scan by default?

Recommendation: start with recent logs plus all logs that contain direct
worktree path matches when cheaply searchable. Add a configurable time window
only if users report performance issues.

## Acceptance Criteria

1. Spacetop can label active task `051` as Codex-handled from local Codex session
   evidence when the matching logs are present.
2. Spacetop can label active task `056` as Codex-handled or multi-session from
   local Codex evidence when multiple matching sessions exist.
3. A task without any direct session evidence renders as `unknown`.
4. Claude Code evidence can be parsed from fixture session directories and
   matched to a task worktree.
5. Multiple matching agents are represented honestly as `multi-agent` or with a
   strongest match plus details.
6. Workflow markdown remains read-only.
7. Detection failures do not prevent workflow loading.
8. No private transcript body text is rendered as evidence.

## Review Notes

This proposal intentionally keeps the feature inside Spacetop. It trades
authoritative assignment for read-only local inference because the chosen
product constraint is that Spacedock will not write ownership metadata. The
design should be revisited only if Spacedock later grows explicit dispatch
metadata; until then, confidence-rated evidence is the honest model.
