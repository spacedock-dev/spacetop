---
title: Open Spacetop as a Herdr sidecar plugin
status: implement
source: Captain request on 2026-10-02; https://herdr.dev/docs/plugins/
kind: feature
risk: medium
milestone: v1-maintenance
proof: Launcher integration tests and an isolated live Herdr shortcut/right-split scenario
started: 2026-10-05T01:42:46Z
completed:
verdict:
worktree: .worktrees/spacedock-ensign-open-spacetop-as-herdr-sidecar
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

## Implementation plan

### Owned files and delivery boundary

- `plugins/herdr/herdr-plugin.toml`: package `spacetop.sidecar`, one `open` action and one `inspector` terminal entrypoint; argv commands use the installed `spacetop-herdr` helper on PATH. No startup/event hooks, automatic registration, or global config writes.
- `crates/spacetop/src/bin/spacetop-herdr.rs`: thin `open`/`pane` dispatch. `crates/spacetop/src/herdr_sidecar/{mod,context,client,launcher}.rs`: typed context/Herdr responses, pure launch decisions, and narrow subprocess boundary. Export the module from `lib.rs` only as needed by this binary/tests. Keep this integration out of core/parser/UI.
- `crates/spacetop/Cargo.toml`: set `default-run = "spacetop"` so adding the helper preserves existing `cargo run -p spacetop` commands. Reuse `serde`, `serde_json`, `clap`, and standard process/filesystem APIs; no new dependency or scripting runtime.
- `crates/spacetop/tests/herdr_sidecar.rs` plus `tests/fixtures/herdr/`: execute the real helper against controlled Herdr/Spacetop boundaries; assert exit codes, argv, context and side effects, rather than testing a second launcher implementation.
- `Makefile`: explicitly build/install both binaries; removal docs name both. `README.md` links `plugins/herdr/README.md`, which owns setup, support, selection, failure and removal instructions. Do not change workflow parsing, TUI, or release publication.

### Supported setup and API assumptions

Initial declared support is **macOS, Herdr 0.9.3 or later**, with 0.9.3 the oldest version actually exercised. Linux and Windows remain unverified and outside the initial manifest platform list; broaden only with equivalent real scenario proof. Installed Spacetop plus the new helper, Git, and a Herdr-managed local pane are prerequisites; source build additionally requires the existing Rust toolchain. No Node/Python runtime is required for the product.

The helper calls the injected `HERDR_BIN_PATH`, using inherited session/socket environment. Herdr 0.9.3 schema reports protocol 22. Required facts are `HERDR_PLUGIN_CONTEXT_JSON` (`workspace_id`, `tab_id`, `focused_pane_id`, `focused_pane_cwd`), `plugin pane open` with split/right/target/cwd/no-focus, `pane list --workspace`, display-only `pane report-metadata --token`, and `plugin pane focus`. Action cwd is the plugin directory; it is never a project fallback. Upstream APIs are early interfaces: fail visibly on missing methods or malformed responses, and do not upgrade/restart Herdr to recover.

### Steps and decisions

1. In the dedicated implement worktree, add the helper and typed context parser. Require nonempty IDs and an absolute existing `focused_pane_cwd`; confirm the target pane belongs to the supplied workspace/tab. Resolve its repository/worktree root with argv-backed `git rev-parse --show-toplevel` in that directory. A missing/invalid focused context or non-Git directory is an actionable failure; do not silently substitute workspace cwd or plugin cwd. Preserve the invoking worktree even when workspace root differs.
2. Resolve the Spacetop executable before opening: prefer a documented absolute `SPACETOP_BIN` override, otherwise the installed binary beside the helper/PATH; validate executable presence. Pass a canonical project directory to `--cwd` as one argv element and pass executable information through pane env as data. The pane entrypoint starts Spacetop with `--workflow-dir <project-root>` via `Command`, never shell text. No explicit workflow hint in this first version: existing discovery and multi-workflow selection remain authoritative.
3. Open the manifest entrypoint using explicit workspace/target pane, `--placement split --direction right --cwd <project-root> --no-focus`. Verify the response matches the requested workspace and new pane identity, then tag it with a stable display-only `spacetop_sidecar=v1` token. The pane entrypoint retains its terminal and streams Spacetop output; actionable startup errors remain visible/logged. Do not send commands into the invoking agent terminal.
4. Reuse by querying live panes in the invoking workspace and matching the owned sidecar token, rather than label/title alone or a persisted pane-id cache. Focus the existing plugin pane; a closed pane disappears from the query and the next invocation opens again. Validate token provenance with controlled plugin-owned focus results; a tagged ordinary pane, failed focus, duplicate match, mismatched context, or unreadable query must fail clearly rather than open duplicates. If a workspace's project differs from an existing sidecar's recorded launch project, fail with close-and-reopen guidance; do not silently show stale project data. Serialize overlapping open actions with a bounded lock under the absolute `HERDR_PLUGIN_STATE_DIR`, with explicit stale-lock/failure recovery documentation; no lock/state under workflow paths.
5. Add manifest/setup docs and tests before live acceptance. Installation is user-driven: build/install both binaries, `herdr plugin link <checkout>/plugins/herdr`, add a configurable `[[keys.command]]` with `type = "plugin_action"`, qualified command `spacetop.sidecar.open`, then reload config as documented. Document plugin trust, no automatic current-workflow inference, prerequisite errors, API errors, unsupported platforms, unlink/removal and optional lock cleanup. Validate exact config syntax in the clean scenario.
6. Run focused launcher tests, `cargo fmt --check`, `cargo test`, and `make lint`. Independently review the diff and repeat the real shortcut scenario before claiming acceptance; FO owns semantic JEV judgments. No new standing CI lane is part of this task.

### Isolated feasibility evidence (2026-10-05)

A temporary manifest/action was linked to a new headless Herdr **0.9.3** session and opened the installed real Spacetop through the requested API path. This is a feasibility spike, not product implementation or full AC acceptance: invocation used the action CLI from a shell pane; keyboard routing, real agent targeting, reuse logic and worktree/space cases still require implementation-stage live proof.

- Isolation: `HOME=/tmp/spacetop-085-clean/home`, `XDG_CONFIG_HOME=/tmp/spacetop-085-clean/config`, `HERDR_CONFIG_PATH=/tmp/spacetop-085-spike/config.toml`, named session `spacetop-085-clean`; socket `/tmp/spacetop-085-clean/config/herdr/sessions/spacetop-085-clean/herdr.sock`. Initial `plugin list` returned `No plugins installed.` All API calls explicitly selected that session. The owned session was stopped after proof.
- Discovery warning: `HERDR_CONFIG_PATH` by itself changes config only; an earlier owned named session inherited the user's plugin registry. It was stopped and proof rerun with temporary HOME/XDG roots. Never use config override alone as proof of plugin isolation. Sandbox denied server/socket access; the authorized isolated exercise succeeded through escalated commands. No captain active layout or global config was edited.
- `plugin action invoke spacetop.spike085.open`: exit 0; action context recorded `focused_pane_id="w1:p1"`, `focused_pane_cwd="/Users/kent/Dev/InfuseAI/GitHub/spacetop"`; actual action cwd was `/private/tmp/spacetop-085-spike`. `plugin log` contained only this action, `status="succeeded"`, `exit_code=0`.
- `plugin pane open --plugin spacetop.spike085 --entrypoint inspect --placement split --direction right --target-pane w1:p1 --cwd /Users/kent/Dev/InfuseAI/GitHub/spacetop --no-focus`: exit 0; response `plugin_id="spacetop.spike085"`, `entrypoint="inspect"`, pane `w1:p2`, `focused=false`, cwd equal to the project.
- `pane layout --pane w1:p1`: exit 0; `focused_pane_id="w1:p1"`, split `direction="right"`, ratio `0.5`, area `120x40`, caller rect `(0,0,60,40)`, inspector rect `(60,0,60,40)`. `pane process-info --pane w1:p2` reported `argv=["/Users/kent/.cargo/bin/spacetop"]` and project cwd. `pane read w1:p2 --source visible` showed `docs/spacetop-dev`, task `085 Open Spacetop as a Herdr sidecar plugin`, and the current shape/plan ribbon.
- `plugin pane focus w1:p2`: exit 0, `focused=true`; `plugin pane close w1:p2`: exit 0; subsequent `pane list --workspace w1` contained only `w1:p1`. Full local stdout/argv/process/layout records, temporary manifest and exercise source are `/tmp/spacetop-085-spike/{proof.json,herdr-plugin.toml,spike.py,exercise.py}`. The facts above are retained here because `/tmp` artifacts are transient. Product tests/docs must replace these machine-specific absolute executable paths with declared installed prerequisites.
- Remaining risk: generic pane list does not expose plugin ownership, only pane metadata; implementation must test the token/owned-focus reuse contract on live Herdr. Real shortcut dispatch and concurrent action behavior are also unproven. These are explicit verification obligations, not assumed host guarantees.

### Acceptance-to-proof map

| Criterion | Launcher/repository evidence | Required isolated live evidence and docs |
|---|---|---|
| AC-1 | Fake Herdr captures exact split/right/workspace/target/no-focus argv; wrong placement or absent target fails tests. | Register final manifest/keybinding, invoke from a real agent pane, retain shortcut input, layout JSON, process and screen evidence; caller stays focused on first open. Setup explains key choice. |
| AC-2 | Actual helper cases for repo, linked worktree, focused subdirectory, spaces/metacharacters, missing/relative/invalid context and contradictory IDs; argv recorder proves each path remains one argument. | Agent in recognizable temporary worktree plus multi-workflow fixture; inspector shows its data. Docs state focused project precedence and no workflow hint. |
| AC-3 | Live-query decision tests for matching workspace token, other workspace, stale/closed pane, duplicate matches, tagged non-plugin pane, failed focus, overlapping invocation and project mismatch. | Repeat shortcut without pane-count increase, focus existing sidecar, close it, reopen; second workspace never selects first workspace's pane. Document per-workspace reuse and recovery. |
| AC-4 | Reject crafted paths and context without executing shell text; before/after hashes and Git status/HEAD/index in fixtures stay unchanged; run `cargo test -p spacetop-core --test no_write_git_calls --test no_terminal_deps`. | Snapshot workflow bytes and Git state around open/reuse/close; exclude deliberate test fixture setup and normal absolute user session persistence. No sync key during proof. Docs preserve explicit Y sync and XDG boundaries. |
| AC-5 | Real helper failures for missing binary, unavailable socket, nonzero API result, malformed JSON and unsupported context assert nonzero exits with next action; installation tests ensure both binaries are delivered. | Fresh isolated setup exercises docs, logs and failures. No global settings changed automatically; removal steps unlink plugin and remove helper/config entry. |
| AC-6 | Focused launcher tests, full `cargo test`, `make lint`, formatting and independent diff review; no success claim based solely on this plan. | Retain final raw scenario outputs and independent verifier's per-AC conclusion; no replacement of missing shortcut evidence with CLI spike evidence. |

## Stage Report: plan

- DONE: Name the owned plugin files, implementation steps, supported platforms, and Herdr API/version assumptions.
  The implementation plan names the manifest, Rust helper/modules, launcher tests, Makefile/Cargo setup and documentation; macOS/Herdr 0.9.3 is the verified initial support boundary.
- DONE: Exercise the riskiest isolated Herdr right-split/context path and retain concrete evidence or an explicit blocker.
  Real Spacetop opened as w1:p2 at x=60 beside w1:p1, project cwd matched, first focus stayed w1:p1, plugin action log exited 0; retained context/layout/process/screen and cleanup facts above plus local proof.json.
- DONE: Map all six acceptance criteria to launcher tests, live scenario evidence, repository checks, and setup/failure documentation.
  The per-AC table names falsifiable command/argv cases, full test/lint gates, required real-agent shortcut/reuse/worktree proof and actionable setup/failure docs.
- SKIPPED: Product implementation and completed-change Rust checks.
  This stage edits only the authorized entity body; implementation belongs to the subsequent dedicated worktree. No Rust/product file changed, so cargo formatting/test/clippy are deferred to that change.

### Summary

The riskiest host path works on macOS with Herdr 0.9.3: an action receives the invoking project separately from plugin cwd, and a real Spacetop pane can open to the right without taking focus. The plan keeps integration in a separate Rust helper and manifest, preserves read-only boundaries, and makes keyboard routing, ownership-safe reuse, spaces/worktrees and concurrency explicit implementation proof obligations. This stage does not claim the six product acceptance criteria are already passed.
