---
title: Refine agent status running detection
status: verify
source: User report on 2026-06-18 that Codex and Claude Code sessions never match PID checks
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: Reproduce with real Codex and Claude Code session logs/process table, then prove the refined rule with core tests plus make lint
started: 2026-06-18T06:38:33Z
completed:
verdict:
score: 0.86
worktree: .worktrees/spacedock-ensign-refine-agent-status-running-detection
issue:
pr:
id: 067
mod-block: merge:pr-merge
---

Codex and Claude Code session attribution currently treats running as a live PID match, but local experiments show real agent session logs often do not contain a reusable PID or do not match ps by PID. This makes the running state and active-session marker effectively unreachable.

Shape this task before implementation. Analyze the actual session artifacts and process-table signals for both Codex and Claude Code, refine the product semantics for running, recent, and stale, and then implement the smallest reliable rule.

Acceptance criteria:

- AC-1: The task documents the observed failure mode with real local evidence from both Codex and Claude Code session artifacts, without leaking transcript content.
- AC-2: The design states exactly what qualifies as running, recent, and stale, including confidence requirements for the list marker.
- AC-3: Running can be reached for currently active Codex and Claude Code sessions without depending only on a JSON pid field.
- AC-4: The implementation preserves stale/recent preview metadata and does not turn mtime-only evidence into a list marker.
- AC-5: Core tests cover the refined detection rule and the old PID-only failure pattern.
- AC-6: README or nearby docs are updated if the user-facing semantics change.
- AC-7: Verification includes cargo test and make lint, or records why a command could not run.

## Plan

### Observed failure mode

Evidence gathered on 2026-06-18 from local artifacts and a redacted process-table
scan:

- Current and recent Codex JSONL artifacts under `~/.codex/sessions` expose
  `cwd`, timestamps, and event types, but the sampled files had no JSON `"pid"`.
- Recent Claude Code JSONL artifacts under `~/.claude/projects` expose `cwd`,
  `sessionId`, timestamps, and event types, but the sampled files had no JSON
  `"pid"`.
- Live CLI processes do expose explicit session identity in argv for resumed
  sessions: Codex as `codex resume <uuid>` and Claude Code as
  `claude --resume <uuid>`.
- GUI/helper/background agent processes also exist without a useful task-session
  identity. Those must not create active markers.

This confirms the captain's report: PID-only `running` detection is not a
trustworthy primary rule for real Codex or Claude Code sessions.

### Recommended semantics

Use this smallest reliable rule:

- `running`: a matched session artifact is tied to a live process by either a
  still-supported JSON `"pid"` value whose process is live, or an exact
  agent-specific session identifier match in the process table.
- `recent`: no live process identity matched, but the artifact mtime is within
  the existing recent window. This remains preview evidence only unless paired
  with a future explicit product decision.
- `stale`: no live process identity matched and the artifact is outside the
  recent window, or no activity time is available.
- List marker (`●`): keep the current confidence gate exactly as-is:
  `AttributionConfidence::Medium` or `High` plus `AgentSessionState::Running`.
  Mtime-only `recent` evidence must not show the marker.

Process identity requirements:

- Codex CLI: match a session artifact UUID extracted from the rollout filename
  against `codex resume <uuid>` exactly.
- Claude Code CLI: match the session artifact stem or JSON `sessionId` against
  `claude --resume <uuid>` exactly.
- Keep PID matching as a compatibility fallback, but do not depend on it.
- Do not mark running from bare `codex`, bare `claude`, app helper processes,
  GUI renderer/service processes, shell wrappers, workdir-only matches, mtime,
  file locks, open file handles, or agent names alone.

### Implementation plan

1. Update `crates/spacetop-core/src/session_activity.rs`.
   Add a typed process snapshot boundary to `ProcessProbe`, for example a method
   that returns command-line observations or answers `has_live_session(agent,
   session_key)`. Keep stdlib `ps` probing contained in `StdProcessProbe`.
2. Parse session identity before classification.
   For Codex, extract the UUID from the rollout filename when present. For
   Claude Code, prefer the JSON `sessionId` when available, then the file stem.
   Reuse the file stem only when it is UUID-like.
3. Replace the spike's broad `line.contains(key)` command check with
   agent-aware argument matching:
   Codex accepts `codex resume <uuid>` or an equivalent exact resume flag shape;
   Claude Code accepts `claude --resume <uuid>`.
4. Preserve the existing `extract_pid` behavior as a fallback. Keep PID and
   session-process probes cached per scan so large session directories do not
   run `ps` once per artifact.
5. Leave `AgentSessionState`, `AgentSessionEvidence::is_active_marker`, and the
   UI labels unchanged unless implementation discovers a user-visible wording
   mismatch. The semantics change belongs in core detection, not rendering.
6. Update `README.md` only if the final code changes the current sentence about
   the active-session marker. If updated, keep it terse: the marker means a
   medium-or-better matched artifact has live PID or exact live session-id
   evidence.

### Tests

Lowest practical layer is core unit tests in
`crates/spacetop-core/src/session_activity.rs`:

- Codex artifact with no `"pid"` and a matching `codex resume <uuid>` process
  is `Running` and gets the active marker when attribution confidence is
  medium/high.
- Claude Code artifact with no `"pid"` and a matching `claude --resume <uuid>`
  process is `Running` and gets the active marker when attribution confidence is
  medium/high.
- The old PID-only failure pattern is covered: no JSON `"pid"` plus no matching
  process identity falls back to `Recent` or `Stale`, not `Running`.
- Mtime-only recent evidence does not set `has_active_marker()`.
- Bare `codex`, bare `claude`, app helper commands, and workdir-only process
  matches do not count as running.
- Existing PID-positive tests continue to pass.

No Ratatui rendering test is required unless the implementation changes
displayed strings or marker placement.

### Existing local spike

There is an uncommitted spike in
`crates/spacetop-core/src/session_activity.rs`. Keep the useful parts:

- adding a session-level probe beside PID probing;
- extracting a UUID-like key from Codex rollout filenames;
- caching session probe results per scan.

Tighten or replace these parts before implementation:

- broad `line.contains(key)` matching;
- broad agent command detection such as any command containing `codex` or
  `claude`;
- lack of Claude Code JSON `sessionId` parsing;
- tests that only mock `is_session_running` without proving argv parsing rejects
  helper/bare processes.

Do not revert unrelated user changes. If the implementer keeps the spike as the
starting point, first inspect the diff, then make the minimum edits above and
run the focused core tests before broader verification.

### Verification

For implementation:

```bash
cargo test -p spacetop-core session_activity
cargo test
make lint
```

If README text changes, also run a quick `rg` for stale marker wording:

```bash
rg -n "active-session|running local|session marker|●" README.md crates docs
```

The change must preserve Spacetop's read-first contract: it reads session files
and the process table only, does not mutate workflow markdown, does not broaden
git writes, and does not create any new workflow-state writer.

## Stage Report: plan

- DONE: Evidence-backed semantics for running, recent, stale cover Codex and Claude Code without relying only on JSON pid fields.
  Checked local Codex and Claude Code artifact keys plus a redacted process-table summary; both sampled artifact families lacked JSON pid while live resumed CLI processes exposed exact session IDs.
- DONE: Implementation plan names exact core/UI/docs files, the lowest practical tests, and how to handle the existing local spike safely.
  Plan targets `crates/spacetop-core/src/session_activity.rs`, optional `README.md`, and focused core tests before `cargo test` and `make lint`.
- DONE: Plan preserves the read-first contract and states what must not become an active-session marker.
  The plan keeps marker eligibility at medium-or-better running evidence and rejects mtime-only, bare agent, helper-process, workdir-only, and agent-name-only signals.

### Summary

Shaped the implementation around exact live session identity rather than PID-only
detection. The smallest reliable rule is PID as fallback plus exact Codex/Claude
resume-session matching, with recent and stale still driven by artifact activity
metadata and kept out of list-marker eligibility.

## Stage Report: implement

- DONE: Implemented exact live session identity detection in core.
  Evidence: `ProcessProbe` now captures process command lines once per scan, `RunStateClassifier` keeps PID/session caches, Codex matches exact `codex resume <uuid>` or `--resume=<uuid>` shapes, and Claude Code matches exact `claude --resume <uuid>` or `--resume=<uuid>` shapes.
- DONE: Preserved PID fallback, recent/stale metadata, and marker confidence rules.
  Evidence: PID-positive tests still pass, pidless sessions without a matching live resume command fall back to stale/recent, and mtime-only recent evidence does not set `has_active_marker()`.
- DONE: Covered false positives and verified the workspace.
  Evidence: focused tests reject bare agent commands, helper commands, workdir-only commands, shell wrappers, and prefix/suffix session-id matches; `cargo test -p spacetop-core session_activity`, `cargo test`, and `make lint` passed.

### Summary

Implemented read-only running detection for PID-less resumed Codex and Claude
Code sessions using exact session-id argv matching, while keeping JSON PID checks
as a compatibility fallback and keeping recent/stale evidence out of active
markers.
