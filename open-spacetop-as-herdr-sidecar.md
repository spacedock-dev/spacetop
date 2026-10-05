---
title: Open Spacetop as a Herdr sidecar plugin
status: plan
source: Captain request on 2026-10-02; https://herdr.dev/docs/plugins/
kind: feature
risk: medium
milestone: v1-maintenance
proof: Launcher integration tests and an isolated live Herdr shortcut/right-split scenario
started: 2026-10-05T01:33:53Z
completed:
verdict:
worktree:
issue:
pr:
mod-block:
id: 085
---

When an agent runs a Spacedock workflow under Herdr, let the user open Spacetop beside that agent with a keyboard shortcut. The right-side terminal pane must inspect the invoking agent's repository or worktree, without requiring the user to start another terminal or navigate to the project manually.

## Scope

- Deliverable home: a Herdr plugin packaged in the Spacetop repository, with its manifest, launcher, tests, and user setup documentation. This is a product integration tracked by the Spacetop development workflow.
- Touches: launch integration, packaging, tests, and docs. Reuse the existing TUI, workflow discovery/picker, and watcher wherever possible.
- Declare the supported platforms and the oldest Herdr version actually verified for the APIs used.
- Non-goals: a replacement TUI, automatic agent dispatch, workflow-state editing, Herdr core changes, marketplace publication, or automatic changes to the user's global plugin/keybinding configuration.

## Acceptance criteria

**AC-1 -- Shortcut opens the right-side inspector.** After the documented plugin registration and keybinding setup, invoking the shortcut from an agent pane opens a managed Spacetop terminal pane split to its right. The first open retains focus on the agent so work can continue there.
Verified by: an isolated live Herdr scenario showing the shortcut, target agent pane, resulting right-side layout, running Spacetop, and focus state. Opening an overlay or splitting the wrong pane would falsify this criterion.

**AC-2 -- The inspector reads the invoking project.** Launch context selects the invoking repository or worktree rather than the plugin installation directory. Paths containing spaces are handled as argv data. With several discovered workflows, Spacetop's existing selection behavior remains available. If an explicit workflow hint is supported, document its precedence and validation; do not claim Herdr automatically knows the current Spacedock workflow.
Verified by: launcher tests for repository/worktree context, paths with spaces, and missing or invalid context, plus a live project containing recognizable workflow data. Launching against the plugin directory or another project would falsify this criterion.

**AC-3 -- Repeated use reuses the sidecar.** Invoking the shortcut again in the same workspace selects the existing Spacetop sidecar instead of adding duplicate panes. After that sidecar closes, the next invocation opens a new one. A sidecar in a different workspace must not be selected accidentally.
Verified by: launch/reuse tests and repeated invocation in the live Herdr scenario, including closed-pane recovery and workspace isolation. An extra pane or a cross-workspace selection would falsify this criterion.

**AC-4 -- Read-only product boundaries are preserved.** The plugin does not edit Spacedock markdown, repair checkout topology, introduce new Git write operations, or persist config/session files in workflow directories. Existing explicitly initiated Spacetop sync and permitted user config/session paths retain their documented contract. Context is passed without interpolating project paths into executable shell text.
Verified by: negative launcher cases, before/after workflow-file and Git-state checks during open/reuse/close, and existing Spacetop write-safety guardrails. A workflow write, an unrequested Git mutation, or command execution from a crafted path would falsify this criterion.

**AC-5 -- Installation and failures are understandable.** Documentation provides reproducible registration/install steps, binary/runtime prerequisites, a configurable example shortcut, removal steps, platform/version support, and workflow-selection behavior. Missing Spacetop, unusable launch context, or Herdr API failures produce an actionable error rather than silently opening the wrong project or modifying user settings.
Verified by: documented setup exercised in an isolated environment and failure-path command results. A clean supported setup that cannot launch, or a silent wrong-project fallback, would falsify this criterion.

**AC-6 -- The integration has independent evidence.** The finished change has focused tests at the launcher boundary, a real Herdr scenario for the shortcut and pane behavior, and passing applicable repository checks. Each AC has evidence outside this task's own assertions.
Verified by: independent verification of the diff, command output, live scenario artifacts, cargo test, and make lint. Missing live proof or a failing required check would falsify this criterion.

## Proof plan

- Start with the riskiest path: a minimal plugin action opening a real right-side pane with the correct invoking-project cwd. Use an isolated Herdr session/config to avoid altering the captain's active layout or settings.
- Test the actual launcher with controlled Herdr/Spacetop command boundaries; verify argv, project context, failures, pane reuse, and workspace isolation.
- Exercise the installed plugin and shortcut on the declared supported Herdr version; retain layout/process evidence and before/after workflow state.
- Run cargo test and make lint for the completed code change. Run cargo fmt if Rust changes. Add no standing enforcement process or CI lane as part of this task.
- Update nearby product documentation with installation, keybinding, selection, support, and troubleshooting instructions.
- FO semantic judgments and acceptance review use the typesafe-ai skill with JEV, as requested by the captain. Obtain a task-specific paid-call budget before making those calls; prior approvals for tasks 079-084 do not cover this task.

## Reference

- Official Herdr plugin documentation: https://herdr.dev/docs/plugins/
- Official Herdr socket API: https://herdr.dev/docs/socket-api/
- Initial feasibility check: local Herdr 0.9.3 exposes plugin pane open with placement split, direction right, target-pane, cwd, and focus/no-focus options. Plugin commands start in the plugin directory and receive Herdr invocation context; this fact makes context selection part of the deliverable rather than an assumed cwd.
