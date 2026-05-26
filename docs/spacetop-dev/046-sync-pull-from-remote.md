---
id: 046
title: Add a Sync action that runs `git pull` when the workflow root has a remote
source: captain
status: plan
started: 2026-05-26T15:08:54Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

Workflow state lives in markdown under a git repo, and teammates push changes (new entities, status transitions, archive moves, mod-block updates) to the remote. Today a user inspecting a workflow in Spacetop has to drop to a terminal and run `git pull` themselves to see what landed since they opened the TUI. The captain wants a user-visible **Sync** action that, when triggered from inside Spacetop, runs `git pull` on the workflow root if the current folder is a git repo with a configured remote.

After a successful pull, the existing file watcher (and the live README reload from task 045 when that lands) picks up the new state without restart.

Likely touch points:

- `src/app/keys.rs` — bind a sync key (and surface a hint in the chrome).
- `src/ui/` — render a button / status pill that toggles between "Sync", "Syncing…", "Synced (n new commits)", and "Sync unavailable (no remote)".
- New small module (or addition under `src/app/`) — a `git_sync` helper that shells out to `git -C {root} pull --ff-only` (or equivalent) and reports a structured result. The helper must be testable without a TUI session.
- Read-only guardrail in `CLAUDE.md` — the safety statement says Spacetop must not mutate workflow files; document in this task body that `git pull` is an explicit user-initiated sync (not a Spacetop write), and is auditable in git history, which satisfies the spirit of the guardrail. Code-level: no `git push`, no `git commit`, no `git checkout` — pull only.

Open design questions (resolve in the `design` stage before planning):

- Fast-forward-only (`--ff-only`) vs. allow merge? Recommend `--ff-only` for safety; if there are local commits ahead, the sync fails with a clear message instead of silently creating a merge commit.
- How to surface the sync status: ephemeral toast at the bottom, persistent status line item, or a one-off modal? Probably a status line slot that fades.
- What counts as "no remote configured": missing `.git`, missing upstream on the current branch, or no `origin` remote? Treat all three as "Sync unavailable" with distinguishable hints in the status line.

## Acceptance criteria

Each AC names a property of the finished entity (not a stage action) and how it is verified.

**AC-1 — Sync runs `git pull` on the workflow root when a remote is configured.**
When the workflow root resolves to a git repository whose current branch has an upstream tracked branch, triggering the Sync action runs `git -C {root} pull --ff-only` (or the design-stage-decided variant) and surfaces the result (commits pulled, "already up to date", or an error message) in the UI.
Verified by: an integration test that constructs a temp git repo with an upstream remote (a bare repo cloned locally) and drives the sync action through the `App` state, asserting the post-sync filesystem contains the remote's commits.

**AC-2 — Sync is unavailable when there is no git repo or no upstream.**
When the workflow root is not a git repository, OR is a git repo with no upstream tracked branch, OR has no `origin` remote, the Sync action either is hidden / disabled OR surfaces a clear "no remote configured" message without invoking `git`. The exact UX is locked in the design stage.
Verified by: a unit / integration test that points Spacetop at a non-git directory and asserts the sync action reports unavailable (does not exec `git`).

**AC-3 — Sync failures do not crash the TUI.**
Non-zero `git pull` exits (network failure, non-FF history, repository lock contention) surface as a status-line error and the prior in-memory state is preserved. The UI does not panic and the watcher keeps running.
Verified by: an integration test that injects a failing `git pull` (e.g., simulate via a stub command runner, or point at an unreachable remote) and asserts the post-sync `App` state is non-panicking with the error captured.

**AC-4 — Successful sync reflects new state in the UI.**
After a successful pull that adds or removes entity files or mutates the README, the on-screen overview / picker / DAG reflects the new state — either via the existing watcher path or via an explicit reload after the pull. This AC cross-references task 045's live README reload work but does not block on it: if 045 has not shipped, the sync action MUST trigger an explicit re-parse of the affected workflow.
Verified by: an integration test that pulls a remote commit which adds a new entity file, and asserts the in-memory entity list contains the new entity after sync.

**AC-5 — Spacetop never writes to the workflow tree itself.**
The sync feature does NOT introduce any code path that calls `git commit`, `git push`, `git checkout`, or any other write that originates inside Spacetop. The only write that lands on disk is the pull's working-tree update, which is auditable in `git log`.
Verified by: a grep over the new code asserting only `git pull` (and read-only `git status` / `git rev-parse` style probes) appear; documented in the implementation stage report.

**AC-6 — `make lint` and `cargo test` remain clean.**
Verified by: running `make lint` and `cargo test` locally.
