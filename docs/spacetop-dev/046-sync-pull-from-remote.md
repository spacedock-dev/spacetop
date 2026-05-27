---
id: 046
title: Add a Sync action that runs `git pull` when the workflow root has a remote
source: captain
status: review
started: 2026-05-26T15:08:54Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-046-sync-pull-from-remote
issue:
pr: #44
mod-block: 
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

## Plan

### Locked design decisions

Three open questions from the spec are resolved here so the implement stage has no ambiguity:

1. **`--ff-only` vs merge.** Use `git -C {repo_root} pull --ff-only`. Non-fast-forward history is reported as an error in the status line; Spacetop never produces a merge commit. Rationale: matches the captain's "auditable in git" framing and keeps Spacetop strictly non-mutating for anything other than upstream content the user explicitly asked to pull.
2. **UX surface.** A status-line slot in the `OverviewState` (a new optional `sync_status: Option<SyncStatus>` field, distinct from `last_refresh_error`) rendered in the existing footer's left side. The footer (`src/ui/footer.rs`) gains a leading pill for sync status when present. States the pill renders:
   - `Syncing…` (in-flight; rendered with the default pill style)
   - `Synced (n new commits)` or `Synced (already up to date)` (success; default style)
   - `Sync failed: {short reason}` (error; rendered with the `broken_pill_style` red foreground that already exists for parse errors)
   - `Sync unavailable: {hint}` (no remote; default style, dim)
   No modal, no toast. The pill persists until the next sync attempt overwrites it (or the user dismisses with `Esc` — out of scope; keep it simple and let the next `Y` keypress replace it).
3. **What counts as "no remote configured".** Three distinguishable hints, all routed through one `SyncAvailability::Unavailable(reason)` enum variant so the helper does the classification and the UI just renders a string:
   - `not a git repository` — `git rev-parse --is-inside-work-tree` exits non-zero, or stdout is not `true`.
   - `no upstream for branch` — `git rev-parse --abbrev-ref --symbolic-full-name @{u}` exits non-zero (most common: branch exists but has no tracking config).
   - `no origin remote` — `git remote get-url origin` exits non-zero. Probed last; only relevant when the user is on an orphan/non-tracking branch but might still want guidance.
   Availability is a pure read-only probe — it never invokes `git pull`. Hidden-vs-disabled UI question: keep the keybinding always active; pressing `Y` when unavailable surfaces the reason string in the status pill. This is easier to discover than a hidden key.

### Module layout

- **`src/git_sync.rs`** (new top-level module under `src/`). Pure logic + a `Command`-runner seam, mirrored after `src/editor.rs`. No TUI dependencies. Public surface:
  - `pub enum SyncAvailability { Available, Unavailable(UnavailableReason) }`
  - `pub enum UnavailableReason { NotGitRepo, NoUpstream, NoOriginRemote }` (with a `fn hint(&self) -> &'static str` for the UI).
  - `pub enum SyncOutcome { UpToDate, Pulled { new_commits: u32 }, Failed { message: String } }`
  - `pub trait GitRunner { fn run(&self, repo_root: &Path, args: &[&str]) -> io::Result<GitCmdResult>; }` where `GitCmdResult { status: ExitStatus, stdout: String, stderr: String }`.
  - `pub struct StdGitRunner;` — production impl that shells out to `git -C {repo_root} {args...}`.
  - `pub fn probe_availability<R: GitRunner>(runner: &R, repo_root: &Path) -> SyncAvailability` — runs the three read-only probes above in order.
  - `pub fn sync<R: GitRunner>(runner: &R, repo_root: &Path) -> SyncOutcome` — `probe_availability` first; if `Unavailable`, returns `Failed { message: reason.hint().into() }`; else runs `git pull --ff-only` and parses stdout for the "Already up to date." string vs. the `Fast-forward` block with a commit count (parse `git rev-parse HEAD` before+after, diff via `git rev-list --count {before}..{after}`).
  - Only commands ever invoked through `GitRunner`: `rev-parse`, `remote get-url`, `pull --ff-only`, `rev-list --count`. AC-5 grep target is `"git push"`, `"git commit"`, `"git checkout"` returning zero matches in `src/`.

- **`src/app.rs`** — add `sync_status: Option<SyncStatus>` accessor + setter on `App` that delegate to the active `OverviewState`. Add a `request_sync()` method that just sets a `pending_sync: bool` flag (same drain pattern as `pending_open_file`). Add `take_pending_sync()` for the event loop.

- **`src/app/overview.rs`** — add `pub sync_status: Option<SyncStatus>` field (default `None`) and a `set_sync_status(SyncStatus)` setter. `SyncStatus` is a small enum: `InFlight | Succeeded { pulled: SyncPulled } | Failed { message: String } | Unavailable { hint: String }`. It is owned by `OverviewState` so a session with multiple workflows preserves per-workflow sync history when the user cycles.

- **`src/app/keys.rs`** — add `KeyCode::Char('Y')` (uppercase, shift-Y, matching the existing `D`/`P` uppercase convention) → `OverviewKeyAction::RequestSync`. Active only when `!state.preview_open()` (consistent with `D`/`s`). Lowercase `y` is deliberately not bound — uppercase makes the action visually distinct from navigation keys and matches "destructive-ish" actions in the existing layout.

- **`src/lib.rs`** (event loop) — add a drain step after the editor drain (step 6): if `app.take_pending_sync()` returns true, capture the active workflow's `repo_root`, set `SyncStatus::InFlight`, redraw once, then synchronously invoke `git_sync::sync(&StdGitRunner, &repo_root)` and set the resulting `SyncStatus`. After a successful pull with `new_commits > 0`, explicitly call `app.reload()` so AC-4 holds whether or not task 045's live README reload has shipped. Run synchronously on the main thread for v1 — `git pull` blocks the UI for a few seconds in the worst case, which is acceptable for a manually-triggered action and avoids introducing a worker thread or channel just for this. If captain pushes back in review we can move to a thread, but the spec doesn't require async.

- **`src/ui/footer.rs`** — extend `status_footer_hints` to render the sync pill as the leading element when `state.sync_status.is_some()`, otherwise the existing broken-count / hints. Add `"Y: sync"` as a new hint in the regular hint chain when `!preview_open` and the workflow has any availability (i.e., probe is cached or unknown — keep it shown unconditionally so the user can discover it; pressing `Y` on a non-git dir teaches them why it's unavailable). Pinned tests in `src/ui/tests.rs` for footer hints get updated alongside the change.

- **`src/ui/help.rs`** — add the `Y` binding to the help popup body.

- **`CLAUDE.md`** — append a short note under "Safety: read-only by default" clarifying that `git pull --ff-only` is the one user-initiated exception, that all pulls are auditable in `git log`, and that no other write commands are ever invoked. (Single paragraph, ~3 lines.)

### Step-by-step implementation order

1. **`git_sync.rs` with `GitRunner` trait, `StdGitRunner`, and a `RecordingGitRunner` test double**, plus unit tests for `probe_availability` covering all three `Unavailable` branches and the `Available` happy path using the recording runner. No file I/O at this step.
2. **`git_sync::sync` happy + sad paths** with the test double: simulate `pull --ff-only` success with `Fast-forward` output (assert `Pulled { new_commits: N }`), `Already up to date.` (assert `UpToDate`), and a non-zero exit with stderr (assert `Failed { message }`). At this point the helper is fully covered without touching `git`.
3. **`SyncStatus` field on `OverviewState` + accessors on `App`** plus the `pending_sync` drain. Unit tests in `src/app/tests.rs` verifying that `request_sync()` sets the flag, `take_pending_sync()` consumes it once, and `set_sync_status` survives a `reload_from_snapshot` (status outlives content reloads).
4. **Keybinding in `src/app/keys.rs`** with a unit test (the existing test module already drives synthetic `KeyEvent` values).
5. **Footer + help-popup rendering** with a snapshot-style test in `src/ui/tests/` for each status variant (mirror existing `tests.rs` patterns; ratatui buffer assert on the pill label).
6. **`lib.rs` event-loop drain wiring**. Smoke-tested manually via `cargo run` after the integration tests land — no automated test for the run-terminal loop itself (existing code follows this pattern; the loop is glue).
7. **Integration test `tests/git_sync_e2e.rs`** that constructs a real git repo, exercises AC-1, AC-3, AC-4 end-to-end (gated on `git` being on `PATH` via a `which::which` check or just letting `Command::new("git").arg("--version")` failure skip the test with `eprintln!` + early return).
8. **`CLAUDE.md` note** in the same commit as step 7 so the documentation lands with the integration test that proves the guardrail.
9. **AC-5 grep evidence** captured in the implement-stage report (`grep -nE 'git (push|commit|checkout)' src/` should return zero matches outside test fixtures).

### Test strategy — AC mapping

| AC | Test location | Concrete fixture / assertion |
|----|--------------|------------------------------|
| AC-1 | `tests/git_sync_e2e.rs::sync_pulls_new_commits_from_upstream` | Build a temp dir with three subdirs: `bare/` (`git init --bare`), `upstream/` (clone of `bare`, push an initial commit containing a workflow `README.md` + one entity file), `working/` (clone of `bare`). Drive `App::request_sync()` against `working/`, run one tick of the event loop's sync drain (factor the drain body into a callable `apply_pending_sync(app: &mut App)` helper for testability), then push a new entity from `upstream/`. Call `apply_pending_sync` again. Assert: `App.sync_status()` is `Succeeded { pulled: Pulled { new_commits: 1 } }` AND the file `working/docs/.../{new-entity}.md` exists on disk. |
| AC-2 | `src/git_sync.rs` unit + `tests/git_sync_e2e.rs::sync_unavailable_on_non_git_dir` | Unit: feed `RecordingGitRunner` that returns non-zero for `rev-parse --is-inside-work-tree` → assert `Unavailable(NotGitRepo)`. Integration: point `App` at a temp dir with no `.git`, call `apply_pending_sync`, assert `sync_status()` is `Unavailable { hint }` matching `not a git repository`, and assert the `RecordingGitRunner` (injected via a test seam constructor on `App`) recorded zero `pull` invocations. The seam: `App::set_git_runner(Box<dyn GitRunner>)` for tests; production constructors use `StdGitRunner`. |
| AC-3 | `src/git_sync.rs` unit + integration `tests/git_sync_e2e.rs::sync_failed_pull_keeps_app_intact` | Unit: `RecordingGitRunner` returns non-zero exit on `pull` with stderr `"fatal: unable to access ..."`, assert `Failed { message }` carries the trimmed stderr. Integration: point a working repo at an unreachable remote (`git remote set-url origin file:///nonexistent/path.git`), call `apply_pending_sync`, assert `App` still functional (`handle_key(KeyCode::Char('j'))` advances selection), and `sync_status` is `Failed`. |
| AC-4 | `tests/git_sync_e2e.rs::sync_reflects_new_entity_in_overview` | Same fixture as AC-1. After successful sync, assert `app.as_session().unwrap().active_state().snapshot.items.iter().any(|i| i.id == "{new-id}")` — proves the explicit `reload()` after a successful pull updates the in-memory entity list. Independent of task 045's watcher path. |
| AC-5 | `tests/no_write_git_calls.rs` (new) | `let src = std::fs::read_to_string(".../src/...all files...")?;` walked via `walkdir`. Assert `!src.contains("\"push\"")`, `!src.contains("\"commit\"")`, `!src.contains("\"checkout\"")` when paired with `Command::new("git")` proximity (use a simple regex). Plus a positive assert that `"--ff-only"` appears exactly once. This is a guardrail test, not a behavior test — keeps AC-5 enforceable in CI. |
| AC-6 | `make lint && cargo test` | Run locally; record output excerpts in implement-stage report. |

### Notes & non-goals

- No async: v1 is synchronous. The pull blocks the event loop. Acceptable per locked decision; revisit only on review feedback.
- No retry: a failed pull surfaces the error and waits for the next `Y` press.
- No credential prompting: if `git pull` would prompt, it will fail with a non-zero exit and the error surfaces in the pill. Users running over SSH-key or PAT auth (the normal case in this codebase) are unaffected.
- Stable user-facing strings (`"Sync unavailable: not a git repository"`, `"Sync unavailable: no upstream for branch"`, `"Sync unavailable: no origin remote"`, `"Syncing…"`, `"Synced (already up to date)"`, `"Synced (N new commits)"`, `"Sync failed: {message}"`) are pinned by unit tests per the CLAUDE.md convention.
- The plan deliberately does NOT touch `src/watcher.rs` — AC-4 is satisfied by the explicit post-sync `reload()`, which is simpler than depending on the watcher seeing the file changes from a `git pull`.

## Stage Report: plan

- DONE: Plan separates a testable git_sync helper (shell-out + structured result) from App keybinding and UI status surface, so the helper runs without a TUI.
  See "Module layout" — `src/git_sync.rs` is a standalone module with `GitRunner` trait + `StdGitRunner` + pure `probe_availability` / `sync` functions; `App` only owns `pending_sync` + `sync_status`; UI lives in `src/ui/footer.rs`. Steps 1-2 cover the helper end-to-end with zero TUI dependency.
- DONE: Plan locks the three design-stage open questions (--ff-only vs merge, UX surface, what counts as 'no remote configured') into concrete commands, UI states, and error messages.
  See "Locked design decisions": (1) `git pull --ff-only` only, no merge commits; (2) status-line pill in the existing footer with four named states and exact label strings; (3) three `UnavailableReason` variants probed in order via `rev-parse --is-inside-work-tree`, `rev-parse --abbrev-ref @{u}`, and `remote get-url origin`.
- DONE: Test strategy maps each of AC-1..AC-5 to a concrete fixture (temp git repo with upstream, non-git dir, failing pull, post-pull entity-list check, grep-over-code for write absence).
  See "Test strategy — AC mapping" table: AC-1 uses bare+upstream+working clones; AC-2 uses both a recording-runner unit test and a non-git tempdir integration test; AC-3 uses an unreachable `file://` remote; AC-4 reuses the AC-1 fixture and asserts on `snapshot.items`; AC-5 is a `walkdir`-based source grep in `tests/no_write_git_calls.rs`.

### Summary

The plan threads a small testable `git_sync` module (mirroring the `editor.rs` seam pattern with a `GitRunner` trait + recording test double) under an `App`-owned `pending_sync` flag drained by the event loop, with a status pill in the existing footer. The three open design questions are locked: `--ff-only` only, a persistent pill in the footer with four named states and pinned label strings, and three distinct `UnavailableReason` variants probed in a fixed order. The integration tests build real git repos (bare + upstream + working clone) for AC-1/AC-4 and use an injected `RecordingGitRunner` test double for unit coverage of AC-2/AC-3, plus a `walkdir`-based AC-5 source-scan test so the read-only guardrail stays enforceable in CI.

## Stage Report: review

- DONE: Each acceptance criterion (AC-1..AC-6) has cited evidence in the implement stage report or in code/tests on the worktree branch; flag any AC without evidence.
  AC-1: `tests/git_sync_e2e.rs::sync_pulls_new_commits_and_reflects_them` builds bare+upstream+working clones, pushes a new entity, drives `apply_pending_sync(&StdGitRunner)`, asserts `Succeeded { new_commits >= 1 }`. AC-2: unit `probe_not_a_git_repo_short_circuits` (asserts argv + short-circuit) plus integration `sync_unavailable_on_non_git_dir` (real non-git tempdir, asserts hint `"not a git repository"`). AC-3: integration `sync_failed_pull_keeps_app_intact` points origin at `file:///definitely/does/not/exist.git`, asserts `Failed{message}` and that a follow-up `KeyCode::Down` does not panic; unit `sync_failed_pull_carries_first_stderr_line` asserts message extraction. AC-4: same fixture as AC-1 asserts `app.snapshot().items.iter().any(|i| i.id == "002")` post-sync — proves the explicit `apply_pending_sync` `app.reload()` on `new_commits > 0`. AC-5: `tests/no_write_git_calls.rs` two tests — `src_tree_does_not_reference_disallowed_git_write_subcommands` walks `src/` and asserts zero literal `"push"`/`"commit"`/`"checkout"` outside comments; `src_tree_references_ff_only_exactly_once` confirms exactly one `--ff-only` in `src/`. AC-6: `make lint` and `cargo test` reported below.
- DONE: Verify `make lint` and `cargo test` run clean on the worktree branch — report the exact commands and exit codes observed.
  `make lint` exit 0 (`cargo clippy --all-targets --all-features -- -D warnings`, finished cleanly). `cargo test` exit 0; per-binary results: lib 320 passed / 0 failed, app_smoke 4 passed, decide_app 10 passed, git_sync_e2e 4 passed, no_write_git_calls 2 passed, watcher_fs 3 ignored, doc-tests 0 — total 343 ran, 0 failed.
- DONE: Confirm read-only guardrail: grep the new code paths for any `git commit`, `git push`, `git checkout`, or other workflow-tree writes; only `git pull` + read-only probes should appear (AC-5).
  Independent grep across `src/`: only `Command::new("git")` site is `src/git_sync.rs:92` (production `StdGitRunner`); all argv goes through `GitRunner::run`. Static argv set is `["rev-parse", "--is-inside-work-tree"]`, `["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]`, `["remote", "get-url", "origin"]`, `["rev-parse", "HEAD"]`, `["pull", "--ff-only"]`, `["rev-list", "--count", &range]`. No `push`/`commit`/`checkout`/`fetch`/`merge`/`rebase`/`reset` literals appear in `src/` outside the documentation comment at `git_sync.rs:138` that explicitly states these commands are never invoked. The `"merge"` literal at `src/domain/mod.rs:284` is a stage-transition label, unrelated to git.

### Summary

PASSED. AC-1..AC-6 all have concrete, executable evidence in code and tests on the worktree branch. `make lint` and `cargo test` are both clean (340 passed across all targets, 3 watcher tests intentionally ignored). The AC-5 read-only guardrail holds: a single `Command::new("git")` invocation lives in `src/git_sync.rs` and routes through `GitRunner::run`, with the complete argv surface restricted to read-only probes plus `pull --ff-only`. The implementation tracks the plan closely — `GitRunner` seam with `RecordingGitRunner` test double, three ordered availability probes, per-`OverviewState` `sync_status`, `pending_sync` drain in the event loop, and a footer pill whose user-facing strings are pinned by `footer_sync_pill_labels_match_pinned_strings`. Recommend moving to captain gate.

## Stage Report: implement

- DONE: Sync action invokes `git pull` (per design-stage decision: --ff-only) on the workflow root when a remote is configured, and surfaces a clear status (commits pulled / up to date / error) in the UI (AC-1, AC-3, AC-4).
  `src/git_sync.rs` shells out to `git pull --ff-only` and parses `rev-list --count {before}..HEAD`; `lib.rs::apply_pending_sync` reloads the snapshot on `new_commits > 0`; footer pill labels pinned by `ui::tests::task_list::footer_sync_pill_labels_match_pinned_strings`; e2e `tests/git_sync_e2e.rs::sync_pulls_new_commits_and_reflects_them` builds a bare + upstream + working clone and asserts the new entity appears post-sync.
- DONE: Sync is unavailable with a clear, distinguishable message when there is no git repo, no upstream tracked branch, or no `origin` remote — without exec'ing git in those cases (AC-2).
  `git_sync::probe_availability` runs three read-only probes in order; unit tests (`probe_not_a_git_repo_short_circuits`, `probe_no_upstream_returns_no_upstream`, `probe_no_origin_remote_returns_no_origin`) assert each variant's argv and short-circuiting; integration `sync_unavailable_on_non_git_dir` asserts the same on a real non-git tempdir.
- DONE: No new code path calls `git commit`, `git push`, `git checkout`, or any other workflow-tree mutation; only `git pull` and read-only probes are introduced (AC-5).
  Guardrail test `tests/no_write_git_calls.rs::src_tree_does_not_reference_disallowed_git_write_subcommands` scans `src/` and asserts zero matches for `"push"`/`"commit"`/`"checkout"`; companion `src_tree_references_ff_only_exactly_once` confirms exactly one `--ff-only` reference in `src/`. `CLAUDE.md` updated to document the single sanctioned exception.

### Summary

Implemented in commit on `spacedock-ensign/046-sync-pull-from-remote`. New `src/git_sync.rs` module owns the `GitRunner` seam, `StdGitRunner` shell-out, `probe_availability` (three ordered read-only probes), and `sync` (pull + before/after `rev-parse` + `rev-list --count`). `App` gained a `pending_sync: bool` flag plus `request_sync`/`take_pending_sync`/`sync_status`/`set_sync_status`/`repo_root` accessors; `OverviewState` gained a per-tab `sync_status: Option<SyncStatus>` so a multi-workflow session retains per-workflow pill state. The event loop drains `pending_sync` between editor and watcher steps via `apply_pending_sync`, which reloads the snapshot on `new_commits > 0`. Footer renders a leading sync pill with stable pinned strings and adds a `Y: sync` hint; help popup gains the `Y` binding. `make lint` clean; `cargo test` 340 passed / 0 failed.
