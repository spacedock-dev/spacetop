# Agent Session Progress Survey

**Date:** 2026-06-14
**Issue:** https://github.com/spacedock-dev/spacetop/issues/28
**Status:** Research survey

## Question

When Spacetop opens a Spacedock workflow, can it know which tasks are being
handled by running Claude Code or Codex sessions, and mark those tasks with a
specific icon like the existing worktree marker?

## Short Answer

Yes, but the first version should treat this as local, confidence-rated
inference rather than authoritative assignment.

Spacetop can already identify task worktrees from workflow markdown and merged
worktree copies. Codex and Claude Code both leave local session evidence that
can usually be correlated with those worktrees. If a live process is also
visible, Spacetop can upgrade "this session worked on the task" to "this
matched session is probably still running."

The list UI should answer one narrow question: is this task currently being
handled by a running matched session? If yes, mark it. If no, leave it unmarked.

It should not claim "owner" or "locked by" unless Spacedock later writes
explicit assignment metadata.

## Source Context

GitHub issue #28 asks Spacetop to show SpaceDock running progress: waiting for
user input, actively working, and stuck or not progressing. The linked Slack
thread frames Spacetop as an `htop`-style side pane for monitoring workflows
while first-officer or worker agents run in Claude Code or Codex inside tmux.
The concrete user pain is not knowing whether SpaceDock is busy, waiting, or
stalled.

The repository already contains a more specific design note:
`docs/superpowers/specs/2026-06-11-spacetop-agent-session-detection-design.md`.
That note proposes read-only local detection from Codex and Claude Code session
data, correlated with workflow task worktrees. This survey agrees with that
direction and narrows it against Issue #28.

## Current Spacetop Signals

### Workflow task data

Spacetop parses each active task into `spacetop_core::domain::Entity`. Relevant
fields already exist:

- `id`
- `title`
- `status`
- `worktree`
- `worktree_source`
- `path`

The `worktree` frontmatter field is the intended dispatch signal in the
Spacetop development workflow. The current workflow README describes it as the
worktree path while a dispatched agent is active.

### Worktree merge data

`crates/spacetop-core/src/parser/worktree.rs` already scans both:

- `<repo>/.worktrees/*`
- `<repo>/.claude/worktrees/*`

It merges task copies from those worktrees into the active snapshot. When a
row comes from a worktree copy or has divergent worktree content, it sets
`Entity.worktree_source`.

This gives agent-session detection a strong join key:

1. canonical workflow root
2. canonical repo root
3. task slug or file path
4. declared `worktree`
5. actual `worktree_source`, when present

### Existing UI marker path

`crates/spacetop/src/ui/list.rs` already renders a fixed-width marker column
for worktree-sourced rows. It shows `U+2387` (`⎇`) when
`item.worktree_source.is_some()`. Tests under `crates/spacetop/src/ui/tests/`
pin that behavior.

An agent-active marker should follow the same pattern: a typed fact computed
outside the UI, then a small fixed-width glyph in the task row. The UI should
not parse session logs or infer workflow schema rules.

### Existing non-blocking background pattern

`crates/spacetop/src/app/history_worker.rs` provides a useful precedent:
history loads after the workflow snapshot and reports back through a channel.
Agent detection should use the same shape. First render should not block on
scanning local session logs.

## External Agent Signals

### Codex

Codex sessions are locally recorded under `~/.codex/sessions/...` as JSONL
session logs. Useful structural signals include:

- session id from `session_meta`
- optional agent nickname or role
- tool-call working directories
- paths passed to file-edit tools
- timestamps on records

Strong evidence: a Codex tool call ran with `workdir` inside the task worktree,
or edited a path under the task worktree.

Medium evidence: the session referenced the task file path or slug together
with the workflow path, but did not operate directly inside the worktree.

Weak evidence: repo-root work plus a task id mention.

### Claude Code

Claude Code sessions are locally stored under `~/.claude/projects/...`, with
project/session paths derived from working directories. Useful structural
signals include:

- session id or resume id
- session project/cwd path
- tool calls or file operations under the task worktree
- timestamps on transcript entries

Strong evidence: the Claude Code session cwd is the task worktree, or a tool
operation touched files under that worktree.

### Live process list

Process inspection can answer a narrower question: is the matched session
probably still running? It cannot reliably identify task ownership by itself.
Process arguments may show `codex`, `codex resume <id>`, `claude`, or
`claude --resume <id>`, but they often omit the workflow task.

Use processes only as an additional freshness signal after log/worktree
correlation has found a session match.

## Confidence Model

Recommended model:

```rust
pub enum AgentKind {
    Codex,
    ClaudeCode,
}

pub enum AgentRunState {
    Running,
    RecentlyActive,
    Stale,
    Unknown,
}

pub enum AgentConfidence {
    High,
    Medium,
    Low,
}

pub struct AgentTaskAttribution {
    pub entity_id: String,
    pub agent_kind: AgentKind,
    pub session_id: Option<String>,
    pub display_name: Option<String>,
    pub confidence: AgentConfidence,
    pub run_state: AgentRunState,
    pub latest_activity_at: Option<String>,
    pub evidence: Vec<AgentEvidence>,
}
```

The exact type names can change, but the concepts should remain: agent kind,
session identity, confidence, running/freshness state, and bounded evidence.

## Task Row UI

Use one compact marker column near the existing worktree marker.

Recommended first-slice marker:

| Meaning | Unicode | ASCII fallback |
|---|---:|---:|
| Task is handled by a running matched session | `●` | `@` |
| No match | blank | blank |

Nerd Font icons can be supported as an enhanced theme, but they should not be
the default rendering contract. Many terminals do not use a patched Nerd Font,
and missing glyphs would render as tofu boxes. The default should remain
portable Unicode with ASCII fallback; a later config option can opt into a Nerd
Font glyph for users who want a richer icon.

Why state glyphs instead of agent initials:

- The row marker should answer the scanning question first: is this task
  actively handled?
- `C` and `X` are too cryptic, especially because `C` could mean Claude Code,
  Codex, current, or complete.
- The exact agent identity fits better in the preview/details line, where
  Spacetop can render `agent: Codex ...` or `agent: Claude Code ...` without
  making the list row noisy.
- The ASCII fallbacks still work under `SPACETOP_ASCII=1`.

Keep `◐` as a deferred optional state only if users later need to distinguish
"recently handled here" from "not handled here." It is not required for this
feature.

If product direction later requires agent-specific labels, prefer labeled
badges in a wider details/table layout rather than one-letter glyphs:

- `Codex`
- `Claude`
- `multi`

The first TUI slice should prefer a binary task-row signal plus a details line.

Example row shape:

```text
  implement  063  ⎇ ● Add agent session indicators
```

Here `⎇` means the visible task row is sourced from a worktree copy, while `●`
means high-confidence agent evidence matched this task and the matched session
appears active. The preview line names whether that session is Codex, Claude
Code, or both.

## Preview Details

The preview header should show an additional compact line only when attribution
exists:

```text
agent: Codex Huygens, running, high confidence, 2m ago
```

For multiple matches:

```text
agent: multi-agent, latest Codex Huygens running, high confidence
```

Evidence details should be structural, not transcript text:

- `Codex exec workdir matched task worktree`
- `Claude Code cwd matched task worktree`
- `session touched task file`
- `process resume id matched session`

Do not render prompts, model responses, command output, or private transcript
body text.

## Mapping To Issue #28 States

Agent-session attribution is enough to support part of Issue #28, but not all
of it.

| Issue state | Can session detection infer it? | Notes |
|---|---|---|
| working | Yes, when high-confidence matched session has recent activity or a live process | Best first target. |
| waiting for user input | Partially | Codex/Claude logs may show the last turn ended awaiting input, but this is tool-specific and can be brittle. Better if Spacedock records explicit gate/wait state later. |
| stuck | Weakly | Can infer stale active worktree plus no recent session activity, but cannot distinguish blocked from paused without Spacedock metadata or explicit agent status. |

Recommendation: implement agent-active markers first as a concrete "working"
signal, then separately design first-officer progress states if the product
needs waiting/stuck accuracy.

## Architecture Recommendation

### Core

Add a core-owned module, likely `spacetop-core/src/agent_sessions.rs`, with:

- scanner traits for Codex and Claude Code session sources
- fixture-friendly roots instead of hard-coded home directories in tests
- session summaries that keep only derived structural facts
- correlation from `WorkflowIndex` active entities to attributions
- ranking and conflict handling

Keep `spacetop-core` terminal-free.

### App

Store attribution state on `OverviewState` or adjacent app state, similar to
history state:

- workflow loads immediately
- agent scanner runs in the background
- results apply only if they still match the active workflow dir
- failures become non-fatal warnings
- reload schedules a new attribution scan

### UI

Render from typed app/core state:

- task list marker column
- preview metadata line
- optional help/footer legend only if the marker is visible

Tests should use Ratatui `TestBackend`, following the worktree marker tests.

## Privacy And Safety

Agent logs are local private data. The feature must stay read-only and
local-only.

Rules:

- Do not write session attribution into workflow markdown.
- Do not upload or transmit session data.
- Do not render transcript content.
- Do not require reading logs if the user disables the feature.
- Skip unreadable or malformed session records.
- Treat process inspection as optional.

This preserves Spacetop's read-first product contract.

## Open Decisions

### Option A - Recommended: local inference from session logs plus processes

| Pros | Cons | Choose this when |
|---|---|---|
| Works without changing Spacedock; preserves read-only workflow markdown; can show task-level active markers soon | Not authoritative; waiting/stuck remain partial inferences | The next goal is to show "this task appears actively handled by Codex/Claude" |

### Option B - Spacedock writes explicit agent status metadata

| Pros | Cons | Choose this when |
|---|---|---|
| Best accuracy for waiting/working/stuck; no private transcript scanning needed | Requires Spacedock write-path design and audited metadata updates | The product needs authoritative progress states, not just local observation |

### Option C - Hybrid local inference plus future explicit metadata

| Pros | Cons | Choose this when |
|---|---|---|
| Delivers useful local indicators now and can prefer explicit metadata later | More states to reconcile; requires clear precedence rules | The team wants quick task markers now but expects Spacedock protocol support later |

## Recommended Next Step

Implement Option A as a narrow first slice:

1. Add fixture-based Codex and Claude Code session scanners in
   `spacetop-core`.
2. Correlate active entities by canonical worktree path.
3. Add high/medium/low attribution ranking.
4. Add a background app worker for attribution refresh.
5. Render a fixed-width task marker and preview `agent:` line.
6. Add tests for scanner fixtures, correlation ranking, app result application,
   and Ratatui task-row rendering.

Do not try to solve waiting/stuck in the first slice. Treat those as a second
design problem unless Spacedock gains explicit progress metadata.

## Acceptance Criteria For First Slice

- A task with high-confidence Codex worktree evidence and a running session
  renders a `●` marker.
- A task with high-confidence Claude Code worktree evidence and a running
  session renders a `●` marker.
- A task without a running matched session renders no marker, even if it has
  old or low-confidence session evidence.
- Preview/details identify whether the matched session is Codex, Claude Code,
  or both.
- Preview shows agent kind, session id or display name, confidence, run state,
  and latest activity without transcript content.
- Session scanner failures do not prevent workflow loading.
- Workflow markdown remains read-only.
- `spacetop-core` remains terminal-free.
