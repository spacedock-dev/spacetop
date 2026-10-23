---
title: Show local refresh and Git sync freshness
status: plan
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-23T08:00:58Z
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
id: 089
---

Help users understand how current the displayed information is without confusing local file refresh with shared Git updates.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: app-state / UI / git / watcher / docs
- Show the last successful local read separately from the last successful Git sync observed by Spacetop.
- Explain storage conditions that prevent sync, including partial definition-only sync.
- Non-goals: Background network polling, automatic sync, checkout repair, new Git write commands, or claiming the remote has no newer changes.

## Acceptance criteria

**AC-1 -- Local reads and Git sync have separate meanings.**
Users can identify when Spacetop last successfully read local files and when it last observed a successful Git sync. Unknown or never-observed times are explicit rather than invented.
Verified by: App-state scenarios for startup, local refresh, successful sync, and unknown sync history; a terminal walkthrough explaining both indicators.

**AC-2 -- Failure never advances a success timestamp.**
Failed reload or sync operations preserve the relevant prior success time and expose the failure. A local file change cannot claim that a Git sync occurred.
Verified by: Deterministic success-to-failure transitions and local-change-only scenarios.

**AC-3 -- Partial sync cannot look like full freshness.**
For split-root workflows, users can distinguish definition sync from state sync. Missing, detached, wrong-branch, or unverified state produces a clear reason when state sync is skipped.
Verified by: App and Git-sync scenarios for fully successful, partially successful, skipped, and failed sync outcomes.

**AC-4 -- Visibility does not broaden writes.**
No new background sync or checkout repair occurs. Explicit sync remains limited to the existing audited fast-forward path and eligible roots.
Verified by: Existing Git write guardrails and a walkthrough without any sync action, followed by an explicit sync in a disposable fixture.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.


## Implementation plan

### Target, confidence, risk, and next decision

The target is unchanged: show what Spacetop successfully read locally and what Git sync it observed, with separate definition/state outcomes. Confidence is high in the existing safety boundary and moderate in the presentation until someone reads the actual UI. The main risk is a true Git success being mistaken for a successful local reread or a complete split-root sync. The next decision is whether the implementation and independent evidence satisfy all four ACs; human comprehension remains a separate gate requirement.

### Ordered work and owned files

1. Add a small typed, session-only freshness model in new `crates/spacetop/src/app/freshness.rs`, declared by `crates/spacetop/src/app.rs`. Keep local-read success, definition-sync success, state-sync success, and latest attempt outcomes distinct. This is app observation state, not workflow metadata; no parser schema, persisted session schema, or new dependency is needed. Use explicit completion instants passed into deterministic update methods, with production `Instant::now()` only at operation boundaries. UI receives typed facts and formats elapsed ages; it does not infer success from `SyncStatus` messages.
2. Integrate the model in `crates/spacetop/src/app/overview.rs` and the app wrappers in `crates/spacetop/src/app.rs`. Record a local-read time only after an actual successful disk load is installed. Synthetic empty states, in-memory snapshot constructors, repaint, history/activity worker results, and filesystem notifications alone do not certify reads. Keep each workflow's observations in its existing `OverviewState`; `crates/spacetop/src/app/session.rs` should retain them by workflow identity during switching/rediscovery. Preserve prior observations when a load fails, including removed-workflow fallback paths, while displaying the error. An initially failed/synthetic load has no successful read time.
3. Wire per-root sync observations at the existing outcome branches in `crates/spacetop/src/lib.rs::apply_pending_sync`. Keep the current definition-first pull, mandatory local reload/re-probe, and verified distinct-root state eligibility sequence. Record each root's successful Git result before attempting the following local read. Preserve the relevant success time on failed, unavailable, skipped, or in-flight attempts. Keep `SyncStatus` as attempt feedback, with typed freshness updates separate from message construction. Do not add a pull, probe, background network operation, or topology repair just to display freshness.
4. Render a persistent freshness area beside the existing footer diagnostics using `crates/spacetop/src/ui/footer.rs` and layout allocation in `crates/spacetop/src/ui/mod.rs`; use additional rows at narrow widths so both local and Git meanings remain visible. Suggested labels are `Local read: 12s ago`, `Git definition: not observed this session`, and, for split-root workflows, `Git state: 3m ago; latest sync skipped`. For single-root storage state sync is `same checkout`, never an invented second success. Keep latest failures/skips prominent even when an older success exists. Put complete reasons, the local/Git distinction, session-only history, and the remote-newness limit in existing `?` help via `crates/spacetop/src/ui/help.rs`. Preserve key behavior and usable list/preview geometry; narrow displays may abbreviate ages but must retain both meanings and an accessible reason.
5. Add deterministic scenarios in `crates/spacetop/src/app/freshness.rs` and `crates/spacetop/src/app/tests.rs`; extend the existing `topology_sync_tests` in `crates/spacetop/src/lib.rs` and `crates/spacetop/tests/git_sync_e2e.rs`. Add Ratatui assertions in `crates/spacetop/src/ui/tests/chrome.rs` and existing help/overview test files only where strings or geometry change. Existing core `git_sync.rs`, `state_checkout.rs`, watcher, parser and guardrail files are boundaries to test, not planned production edits.
6. Update `README.md` with delivered labels, session-only ages, failure/partial examples, and explicit `Y` behavior. Update the app/UI ownership sentences in `AGENTS.md` and `docs/development-policy.md` only if needed for the new module. Add a reproducible walkthrough to new `docs/testing/freshness-walkthrough.md`: shell setup for disposable local bare remotes and clones, fixture README/entity content, failure triggers, exact candidate debug launch, and cleanup limited to that disposable fixture. Do not edit unrelated workflow tasks or Navigator design.
7. Run the commands and scenarios below in the later dedicated implementation worktree. Commit only its task files with `git commit -s`. Hand the candidate commit, test results, fixture recipe, and unresolved human evidence to fresh independent verification. Navigator separately runs the same debug binary in a visible Paseo terminal. PR publication and merge wait for evidence-bound gate approval.

### Timestamp and outcome rules

- History begins unknown on each process start. A successful initial disk load records local read only. Never derive Git time from file mtimes, commit dates, restored preferences, process presence, or another tab's last attempt. Label unknown history `not observed this session`, since an external sync may have occurred.
- Use monotonic elapsed ages with an injected instant in tests; a supplied time earlier than the observation must produce an explicit unavailable age rather than panic or fabricated freshness. The display describes elapsed time since observed completion, not a remote currency guarantee. No persisted timestamps or wall-clock formatting dependency is needed.
- A local read means the active workflow snapshot was successfully loaded from disk. Existing per-item parse warnings remain visible and do not become a claim that every item is valid. Lazy archive-read failure keeps its error visible and must not advance the local success record; synthetic archive/index replacement is not a disk read. If a displayed archived view cannot be refreshed, preserve the prior local success and expose incomplete read status rather than stamping the whole display as successfully refreshed.
- A successful local reread, including one triggered by watcher or tab activation, advances only local success. Failure keeps the last good snapshot/time and surfaces the current error. A watcher outage and an editor error are diagnostics, not successful reads or Git failures.
- `UpToDate` and `Pulled` are both observed Git successes for the root actually called. Failed availability probes, pull failures, blocked eligibility and in-flight status never advance that root's success time.
- Definition success followed by reload failure records definition success, preserves local-read success, skips state sync, and exposes the reload error. Do not use cached topology to attempt state sync.
- Definition success plus blocked/missing/detached/wrong-branch/unverified state records definition success only. Preserve any applicable prior state success with an explicit latest-skip reason; do not show a full-sync success. If state pull fails, record its failure separately from definition success.
- Both Git roots succeeding followed by a failed final reread records both true Git successes while local-read time remains at its last successful read. Show `Git synced; local read failed`; this is not complete display freshness. No aggregate success label may conceal this distinction.
- Bind state observations to the resolved state target and expected branch, and definition observations to their repository root. If a successful definition reload changes `state:` or `state-branch:`, do not carry the old target's state-success time onto the new one. Single-root/split-root transitions and workflow rediscovery must not transplant observations. If identity is unverified, retain any older observation only as explicitly previous-target history; the current target is unknown.

### Acceptance criteria and falsifiable evidence

| Unchanged criterion | Implementation boundary | Lowest-layer falsifiable proof | Terminal or human proof |
|---|---|---|---|
| AC-1 -- Local reads and Git sync have separate meanings. | Typed app observations; disk load/reload boundaries; footer/help. | Startup sets local only; an in-memory/synthetic state sets neither; local-only reload advances local only; successful explicit sync stamps only called roots; per-tab observations survive switching; restart shows unknown Git history. A mutation that stamps Git during local reload must fail these tests. | Show startup, local-only edit, explicit `Y`, and restart in a disposable fixture. A human explains what each age establishes and why unknown Git history is honest. |
| AC-2 -- Failure never advances a success timestamp. | Completion-only update methods; error paths in overview and sync orchestration. | Fixed instants T1/T2/T3 exercise success then failed reload, failed definition pull, failed state pull, skipped/in-flight attempts, and final-read failure after successful pulls. Assert exact unchanged relevant instants and visible errors, not just status strings. A mutation stamping a failed branch must fail. | Make fixture README unreadable/invalid after a good read; restore it, then make the disposable remote unavailable for a pull failure. Verify old successes remain alongside the failure. |
| AC-3 -- Partial sync cannot look like full freshness. | Fresh re-probe, typed root outcomes and target identities, per-root rendering. | Cover full success including no-op pulls; definition success/state failure; missing, detached, wrong-branch, and unverified skip; changed state target/branch; definition reload failure; final reread failure. Recording runner asserts exact called roots and no state calls after blocked/failed re-probe. A mutation that reports full success or reuses cached eligibility must fail. | Run attached split-root, missing-state, and failed-state-pull cases; inspect both root indicators and full reason in help. Ask the human which files were synced and whether displayed progress can be assumed current. |
| AC-4 -- Visibility does not broaden writes. | Existing explicit `Y` dispatch, audited core helper and eligibility checks; session-only app state. | Keep `no_write_git_calls` and `no_terminal_deps` guardrails. Existing/new recording-runner assertions prove zero pull without `Y`, only the definition pull for blocked state, and exactly definition then eligible distinct state pulls for full sync. No new timestamp persistence. | Browse, wait, resize, and change a local fixture file without `Y`; record no Git pull in fixture tracing. Then press `Y` against local disposable remotes and show only the permitted fast-forward pulls. Product use must not rewrite workflow markdown except via those pulls. |

### Commands and proof capture

Run from the dedicated implementation worktree; these commands are future implementation/verification gates, not claims that the feature exists now:

```sh
cargo fmt
cargo test -p spacetop --lib freshness
cargo test -p spacetop --lib topology_sync_tests
cargo test -p spacetop --test git_sync_e2e
cargo test -p spacetop-core --test no_write_git_calls --test no_terminal_deps
cargo test
make lint
cargo build -p spacetop
```

Use `target/debug/spacetop --workflow-dir "$freshness_fixture/definition/workflow"` for the visible walkthrough after following the committed fixture setup. Keep HOME/XDG config/state isolated under that absolute disposable fixture. Capture candidate SHA, exact debug executable, terminal dimensions, actions, observed labels/errors, and exit status. At minimum test 120x30, 80x24 and 40x20, including preview/help; TestBackend checks must catch freshness text being hidden by long diagnostics, not merely assert a formatting helper's output. If watcher backend behavior changes despite the planned scope, also run `cargo test -- --ignored` and explain the reason; otherwise retain existing watcher mechanics.

Use Git tracing on the fixture process and a before/after fixture file/status record to distinguish ordinary local reads from explicit pulls. Setup may create fixture commits, branches, and remotes; these are harness operations, never new product Git commands. Capture the actual `Y` operation separately from setup. Do not exercise sync or failure injection on the user's real workflow repositories.

Human-comprehension evidence is still unresolved. Automated assertions and Navigator's terminal run prove state transitions and visible output, but do not prove the human understands the distinction. At the gate, record a human's own explanation of local age, per-root Git age, skipped/failed state sync, and why an observed sync does not prove the remote has no newer work. If that explanation is missing or incorrect, keep AC-1/AC-3 human proof open rather than substituting agent judgment.


## Stage Report: plan

- DONE: Map all four unchanged acceptance criteria to concrete implementation boundaries, tests, and terminal or human proof.
  The AC matrix maps each original criterion to app/orchestration/UI boundaries, a falsifying change, and terminal or human evidence; the original task prefix is byte-for-byte unchanged.
- DONE: Provide an ordered plan with exact owned Rust/UI/docs files and failure-safe timestamp and split-root outcome semantics.
  Seven ordered steps name exact files, session-only completion instants, per-root outcomes, failed-read preservation, and target-identity invalidation without changing sync eligibility.
- DONE: Record commands, falsifiable proof cases, and unresolved human evidence without broadening Git writes or scope.
  Commands and fixture proof cover all ACs; fresh independent verification, Navigator debug-terminal execution, and human explanation remain explicitly pending implementation.
- DONE: Exercise existing safety behavior used by the plan.
  At code commit `18c97ec`, `cargo test -p spacetop --lib topology_sync_tests -- --nocapture` passed all 9 tests: stale/blocked topology cannot trigger a state pull, eligible roots pull exactly, and final-read failure stays partial; allowing cached/duplicate state pulls or reporting final-read failure as full success would fail these assertions.
- DONE: Validate the task-only planning artifact.
  `spacedock status --workflow-dir docs/spacetop-dev --validate` returned VALID; `git diff --check` passed; comparison with the prior committed task confirmed the existing frontmatter, scope, ACs and proof plan remain unchanged.
- SKIPPED: Implementation checks and delivered-feature terminal/human proof.
  This stage changes only the assigned task body. Rust formatting, full tests, lint, fixture walkthrough and human comprehension are specified for later stages, not claimed complete here.

### Summary

The implementation plan separates local-read evidence from definition/state Git successes and preserves failures without inventing timestamps. Existing topology tests support reusing the audited orchestration boundary; the new freshness behavior and human interpretation still require implementation and independent proof before publication approval.
