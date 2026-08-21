---
title: Surface detached split-root state checkouts
status: verify
source: "Captain request plus spacedock-dev/spacedock#677 and #630"
kind: feature
risk: high
milestone: v1-maintenance
proof: Detached, attached, wrong-branch, and missing split-root fixtures plus parser, app, Ratatui, watcher, and git-sync guardrails
started: 2026-08-21T06:46:19Z
completed:
verdict:
score: 0.88
worktree: .worktrees/spacedock-ensign-support-detached-split-root-state
issue: spacedock-dev/spacedock#677
pr:
id: 076
---

Spacedock currently implements two storage backends, not three: default or `state: $inline` is single-root, while any supported relative `state:` path is split-root. A detached HEAD is a runtime disposition of a materialized split-root state checkout. The same split-root workflow can also be an attached holder, have no state checkout in this workspace, or have a checkout on the wrong branch.

Spacetop already resolves a present split-root directory by path and therefore can read entities from a detached checkout, but it does not model checkout disposition. A missing checkout becomes indistinguishable from a healthy empty workflow, and the `Y` sync result can describe the definition repository as current without explaining that detached entity state was not refreshed. The outcome of this task is trustworthy read-only inspection: load any available state snapshot, clearly say whether it is attached, detached, wrong-branch, or missing, and never imply that a non-holder workspace is healthy or current.

Upstream evidence:

- `spacedock-dev/spacedock#677` documents a materialized split-root checkout on detached HEAD that passes `entity_dir_present` despite not holding the state branch.
- `spacedock-dev/spacedock#630` documents an absent non-holder checkout that currently looks exactly like an empty workflow.
- Spacedock `internal/status/state.go` defines only `StateInline` and `StateSplitRoot`; detached, missing, wrong-branch, and remote availability are checkout facts within split-root.

## Scope

- Kind: feature with correctness and trust implications
- Risk: high because the change touches git topology detection, watcher/reload behavior, sync messaging, and stable TUI strings
- Milestone: v1-maintenance
- Touches: typed domain/app state, parser/root resolution, read-only git probes, watcher/reload, TUI chrome/status, sync messaging, docs, fixtures, and tests
- Non-goals: creating, attaching, repairing, committing, rebasing, or pushing a state checkout; invoking `spacedock state ready`; adding a new README `state:` sentinel or storage backend; changing Spacedock; mutating workflow markdown

## Acceptance criteria

**AC-1 -- Spacetop represents storage backend and checkout disposition as separate typed facts.** Default, absent, empty, and `$inline` declarations remain single-root; a contained relative path remains split-root. A split-root checkout is then classified at least as attached holder, detached, wrong-branch, or missing without UI code inferring these states from strings.
Verified by: pure classifier tests and real-git fixtures that fail if detached or wrong-branch is collapsed into attached, or if missing split-root falls back to single-root.

**AC-2 -- A materialized detached split-root checkout remains inspectable.** Spacetop loads active and archived entities from its resolved entity directory and supports preview and refresh exactly as it does for an attached checkout, while visibly identifying that the snapshot is detached and may not be current.
Verified by: a real detached-worktree fixture, parser/index assertions for active and archived entities, and Ratatui `TestBackend` assertions for the detached-state indicator.

**AC-3 -- Healthy attached split-root and single-root workflows keep their existing behavior.** An attached checkout on the resolved state branch has no false warning, single-root workflows require no state-branch probe, and existing selection, archive, preview, discovery, and worktree-overlay behavior remains unchanged.
Verified by: attached split-root and single-root regression fixtures plus existing parser, app, and rendering suites.

**AC-4 -- Non-holder states cannot masquerade as an empty healthy workflow.** A missing checkout and a materialized checkout on the wrong named branch produce distinct, actionable state diagnostics. If entity files are available they remain readable; if not, the empty list is paired with the diagnostic rather than presented as an ordinary zero-entity workflow.
Verified by: missing-directory and wrong-branch fixtures at the domain/app boundary and stable TUI string assertions.

**AC-5 -- Reload and filesystem watching preserve topology truth.** Workflow load, explicit reload, and workflow switching re-probe checkout disposition. Entity Markdown changes inside a materialized detached state directory still trigger the same reload path; appearance or disappearance of the declared state directory updates both the entity snapshot and its diagnostic without restarting Spacetop.
Verified by: watcher filtering/debounce tests, app reload tests, and the ignored real-notify smoke only if backend behavior changes.

**AC-6 -- Explicit sync remains read-only-safe and does not overclaim freshness.** Spacetop never checks out, attaches, commits, rebases, or pushes state. The `Y` action does not run `git pull` against a detached or wrong-branch state checkout and does not report the whole workflow as synced when only the definition repository was refreshed.
Verified by: `GitRunner` argv assertions, sync-status rendering tests, and the existing `no_write_git_calls` guardrail.

**AC-7 -- User-facing documentation names the two backends and the split-root dispositions accurately.** Nearby docs explain that detached is a split-root checkout condition, not a third `state:` mode, and give the user an actionable read-only interpretation of attached, detached, wrong-branch, and missing indicators.
Verified by: review of updated README/help text paired with the behavior tests above; prose alone does not satisfy AC-1 through AC-6.

## Proof plan

- Lowest test layer: pure typed classification first; parser/index tests for snapshot loading; real temporary git repositories for attached, detached, wrong-branch, and missing topology; app and Ratatui tests for diagnostics; watcher and git-sync tests for refresh and argv safety
- Required commands: `cargo fmt --all -- --check`; focused `cargo test -p spacetop-core` topology/parser/git tests; focused `cargo test -p spacetop` app/UI tests; full `cargo test`; `make lint`
- Manual check, if any: open a temporary split-root workflow whose state directory is a detached worktree and confirm entities remain readable while the detached diagnostic is visible
- Docs/policy update needed: update README and the AGENTS code map if a new topology module or typed boundary is added; keep `docs/development-policy.md` subordinate to AGENTS and current with any changed architecture

## Implementation plan

### Evidence and classifier contract

A disposable real-Git fixture at `/tmp/spacetop-detached-plan-probe-20260821`
materialized the same `spacedock-state/dev` commit four ways. Read-only probes
reported `spacedock-state/dev` for the holder, `HEAD` for the detached checkout,
`wrong-state` for the wrong branch, and no directory for the missing checkout.
Current `spacetop export` loaded one active and one archived entity from the
detached checkout, but returned ordinary empty `entities` and
`archived_entities` arrays for the missing checkout with no topology fact. This
confirms both the existing inspectability and the silent-empty gap.

1. In `crates/spacetop-core/src/domain/mod.rs`, add separate typed concepts for
   the two storage backends and split-root checkout disposition. Model
   `SingleRoot` separately from `SplitRoot { entity_dir, expected_branch,
   disposition }`; disposition must include `Attached`, `Detached`,
   `WrongBranch { actual_branch }`, and `Missing`. Add a fail-closed
   `ProbeFailed { reason }` variant for a present non-checkout or unexpected Git
   failure rather than mislabeling it healthy. Carry this typed state through
   `WorkflowSnapshot` and `WorkflowIndex`, with query accessors for app code.
2. In `parser/readme.rs`, parse optional `state-branch:` alongside `state:`.
   Preserve current backend rules and `resolve_entity_dir`: absent/blank/
   `$inline` and unsupported absolute or parent-traversing values stay
   single-root; a supported relative path is split-root. For split-root, resolve
   the expected branch exactly as Spacedock does: trimmed `state-branch:` wins,
   otherwise `spacedock-state/<definition-directory-basename>`.
3. Add `crates/spacetop-core/src/state_checkout.rs` for Git topology only. If
   the entity directory is absent, return `Missing` without invoking Git. For a
   present directory, run only `git -C <dir> rev-parse --show-toplevel` and
   `git -C <dir> symbolic-ref --quiet --short HEAD` through `GitRunner`.
   Canonicalize the reported top level before comparing it with the entity
   directory (important for `/tmp` versus `/private/tmp`); a matching expected
   branch is `Attached`, symbolic-ref exit 1 is `Detached`, and any other named
   branch is `WrongBranch`. A non-repository, parent-repository fall-through, or
   other probe error is `ProbeFailed`. Never fetch or inspect the network.
4. Have `parser/snapshot.rs` probe after README parsing but before scanning.
   Always scan a present entity directory regardless of non-holder disposition,
   so detached/wrong-branch active and archived entities remain previewable.
   `sources.rs` continues to resolve archives from the same entity directory;
   `index.rs` retains the topology fact rather than dropping it while building
   query state.

### App, UI, reload, and sync

5. In `app/overview.rs`, expose the typed topology and map non-healthy
   dispositions to one stable diagnostic model. `Attached` and all single-root
   workflows emit no warning. Detached says the snapshot may be stale;
   wrong-branch names actual and expected branches; missing says no state
   checkout was loaded; probe failure says topology could not be verified. UI
   code must render this model, not reinterpret raw `state:` or Git strings.
6. Render a compact persistent state-warning pill in `ui/footer.rs` (and keep
   the header/list/preview layout unchanged). Pin exact detached, wrong-branch,
   missing, and probe-failed strings with Ratatui `TestBackend`; also pin the
   absence of a warning for attached and single-root fixtures. Empty missing
   workflows therefore remain empty but can no longer look healthy.
7. Keep the existing recursive watcher rooted at the definition/discovery root:
   supported state paths are contained below it, and `.spacedock-state` plus
   Markdown paths already pass the relevance filter. Add deterministic event
   relevance/debounce tests for state-directory create/remove and detached
   entity edits. Because `OverviewState::load`, `reload`,
   `reload_with_rediscovery`, and `materialize_active` all rebuild the index,
   each path must re-probe topology. Add app regressions for
   detached-to-attached, present-to-missing, and missing-to-present transitions,
   including selection/archive preservation. Run/add an ignored live-notify
   smoke only if production watcher behavior changes.
8. In `lib.rs::apply_pending_sync`, keep `git_sync.rs` as the only pull helper
   and orchestrate two repositories from typed topology. Pull the definition
   repository first. After a successful definition pull, reload/re-probe; for
   single-root, preserve today's success status. For an attached split-root
   checkout, run the same audited `git pull --ff-only` against `entity_dir`,
   reload, and report definition-plus-state success. For detached,
   wrong-branch, missing, or probe-failed state, never pull the entity directory;
   report a partial result such as “definition synced; detached state not
   refreshed,” never the existing whole-workflow success message. A failed
   definition pull stops before state sync; a failed attached-state pull reports
   partial/failure without discarding the readable snapshot.
9. Update `SyncStatus`, `ui/footer.rs`, and `ui/help.rs` with pinned composite and
   partial-sync messages. `GitRunner` recording tests must assert exact roots and
   argv, especially that detached/wrong/missing cases contain no state-root
   `pull` call. Keep `no_write_git_calls.rs` green: no checkout, switch, attach,
   commit, rebase, push, or workflow-Markdown write is introduced.

### Lowest-layer proof and documentation

10. Add pure classifier/parser tests for all backend declarations,
    `state-branch:` override/default derivation, the four required dispositions,
    canonical path comparison, parent-repository fall-through, and probe failure.
    Add a self-contained real-Git integration fixture under
    `crates/spacetop-core/tests/` that creates attached, detached, wrong-branch,
    and missing workspaces and proves detached/wrong snapshots still load active
    and archived entities while missing stays typed split-root.
11. Add app reload/switch tests, Ratatui diagnostic tests, watcher filter/debounce
    tests, and sync call-recording tests mapped directly to AC-1 through AC-6.
    Preserve the existing single-root, split-root parser, discovery, archive,
    preview, selection, worktree-overlay, and sync tests as regressions.
12. Update `README.md` with the two-backend/four-disposition explanation and the
    meaning of each warning. Update `AGENTS.md` and
    `docs/development-policy.md` for the new topology module and clarify that
    `Y` may fast-forward both the definition repository and a verified attached
    state checkout, but never repairs topology or runs any broader Git write.

Verification: `cargo fmt --all -- --check`; focused
`cargo test -p spacetop-core state_checkout`; focused split-root parser/index
tests; focused `cargo test -p spacetop` app/UI/sync tests;
`cargo test -p spacetop-core --test no_write_git_calls`; full `cargo test`; and
required `make lint`. Run `cargo test -- --ignored` only if watcher production
behavior or ignored watcher coverage changes.

## Stage Report: plan

- DONE: Exercise the smallest real detached split-root checkout path and use its evidence to confirm the two-backend, four-disposition taxonomy and the actual current Spacetop gap.
  A four-workspace real-Git fixture confirmed attached/detached/wrong-branch/missing; current export reads detached active+archive state but emits missing state as undiagnosed empty arrays.
- DONE: Produce an implementation plan with exact typed ownership across parser/domain, read-only git probes, app/UI diagnostics, reload/watcher behavior, sync messaging, and nearby documentation.
  The 12-step plan assigns domain, parser, new `state_checkout`, index, app, Ratatui, watcher, sync, README, AGENTS, and development-policy ownership with exact read-only probe and partial-sync semantics.
- DONE: Define lowest-layer failing tests and repository verification that cover every acceptance criterion while preserving the read-only boundary and existing single-root and attached split-root behavior.
  Pure classifier/parser tests, real-Git fixtures, app reload/switch, TestBackend, watcher debounce, GitRunner argv, and no-write guardrails map to AC-1..AC-7; full test and lint commands are named.

### Summary

Confirmed the product has two storage backends and that detached, wrong-branch,
missing, and attached are runtime dispositions of split-root state. The plan
keeps every available snapshot readable, adds fail-closed typed diagnostics,
re-probes on every load path, and makes `Y` sync truthful without introducing
checkout repair or any Git write beyond audited fast-forward pulls.

## Stage Report: implement

- DONE: Implement the typed two-backend/four-disposition topology model and read-only Git probes, thread it through parser/index/app reload paths, and keep detached or wrong-branch snapshots readable while missing and probe failures stay explicit.
  Commit `c339356` adds typed storage/disposition facts, canonical Git probes, `state-branch:` handling, readable non-holder snapshots, and reload re-probing.
- DONE: Render stable topology diagnostics and truthful partial/full sync outcomes, ensure only verified attached split-root state receives the existing fast-forward pull, and preserve watcher/reload behavior without adding topology-repair or workflow-state writes.
  Ratatui tests pin all diagnostics; GitRunner tests fail if non-holder state receives any Git call or attached state omits its audited fast-forward pull.
- DONE: Add lowest-layer classifier, real-Git, parser/index, app, Ratatui, watcher, and GitRunner/no-write coverage for AC-1 through AC-7; update README, AGENTS.md, and development policy; run focused tests, full cargo test, formatting, and required make lint, then commit the implementation branch.
  Focused topology, parser, watcher, reload, sync, real-Git, and no-write tests passed; full `cargo test` passed all unit/integration/doc tests, while `cargo fmt --all -- --check`, `make lint`, and `git diff --check` passed. Ignored watcher tests were not run because production watcher behavior and ignored coverage were unchanged.

### Summary

Implemented fail-closed split-root topology inspection while preserving every materialized state snapshot’s readability. Sync now distinguishes single-root success, full attached-state success, and definition-only partial outcomes without adding repair commands or broader workflow-state writes. The implementation is committed as `c339356`.

## Stage Report: verify

- DONE: Independently review commit c339356 against AC-1 through AC-4: verify the two-backend/four-disposition typed contract, fail-closed read-only probes, detached/wrong-branch snapshot readability, and stable actionable diagnostics without UI string inference.
  Classifier tests fail if facts collapse or probes stop failing closed; the real-Git fixture fails if detached/wrong-branch snapshots become unreadable; Ratatui tests pin all diagnostics and healthy-warning absence.
- FAILED: Independently verify AC-5 and AC-6: reload/watcher topology transitions and sync orchestration must re-probe state, preserve readable snapshots, pull only definition plus verified attached state with git pull --ff-only, and keep no-write guardrails intact.
  REQUEST CHANGES (High): `crates/spacetop/src/lib.rs:657-670` ignores `app.reload()` failure and can pull state using cached pre-definition-pull `Attached` topology. The fixture at `crates/spacetop/src/lib.rs:119-205` uses a nonexistent workflow root yet still expects that pull, proving the stale-verification fallback. On reload failure, return a partial/failure status without state Git calls and add a regression covering an invalidated or changed README.
- DONE: Verify AC-7 and completion evidence by reviewing docs and test mapping, running focused tests plus full cargo test, cargo fmt --all -- --check, make lint, and git diff --check; report a direct acceptance verdict and every actionable defect with file/line evidence.
  README, AGENTS, and development policy are accurate. Passed: `cargo test -p spacetop-core state_checkout`, `cargo test -p spacetop-core --test state_checkout_fixtures`, `cargo test -p spacetop --test state_topology_reload`, `cargo test -p spacetop topology`, `cargo test -p spacetop-core --test no_write_git_calls`, `cargo test -p spacetop-core watcher::tests`, full `cargo test`, `cargo fmt --all -- --check`, `make lint`, and both working-tree and commit `git diff --check`. Ignored watcher tests were skipped because production watcher behavior and ignored coverage did not change.

### Summary

Verdict: REQUEST CHANGES. AC-1 through AC-5 and AC-7 are supported, but AC-6 fails closed incorrectly when the mandatory post-definition reload/re-probe fails; cached topology must not authorize a state pull.

### Feedback Cycles

- Cycle 1: REQUEST CHANGES — independent verify reviewer; surface 30 files vs estimate undeclared (N/A); AC unchanged

## Stage Report: implement (cycle 2)

- DONE: Fix `apply_pending_sync` so a failed mandatory reload/re-probe after the definition pull fails closed: do not consult cached split-root topology, make no state-root Git call, and surface a truthful partial or failed sync result.
  Commit `90e35c1` returns before storage lookup when reload fails and reports post-state-sync reload failures truthfully.
- DONE: Replace the fail-open expectation and add regression coverage where the definition pull invalidates or changes state configuration, proving stale attached topology cannot authorize a state pull.
  `reload_failure_does_not_pull_cached_attached_state_checkout` and `changed_state_configuration_cannot_authorize_cached_attached_pull` fail if cached attached state receives any Git call. The attached happy path now requires a real re-probed checkout.
- DONE: Run focused sync/topology/no-write tests, the full test suite, formatting, lint, and diff checks; commit the correction and return a checklist-complete implementation report for correction cycle 1.
  `cargo test -p spacetop topology_sync_tests -- --nocapture` passed 3 tests; `cargo test -p spacetop --test git_sync_e2e` passed 4 tests; `cargo test -p spacetop-core --test no_write_git_calls` passed 2 tests. `cargo test` passed all enabled tests, including 392 `spacetop` library tests and 192 `spacetop-core` tests; 3 real-watcher tests remained intentionally ignored. `cargo fmt --all -- --check`, `make lint`, and `git diff --check` passed.

### Summary

Correction cycle 1 now fails closed at the mandatory post-definition reload boundary, so cached topology cannot authorize a state pull. Changed state configuration is honored before state sync, while a genuinely re-probed attached checkout retains the fast-forward sync path.

## Stage Report: verify (cycle 2)

- DONE: Independently verify correction commit 90e35c1 closes the prior High finding: every failed post-definition reload must return without state-root Git calls, changed state configuration must replace cached topology, and only a successfully re-probed attached checkout may receive git pull --ff-only.
  `lib.rs:725-742` returns before cached storage is read on reload failure; all three focused correction tests passed, including changed-path replacement and the real re-probed Attached happy path.
- FAILED: Re-review the complete c339356..90e35c1 implementation against AC-1 through AC-7, including typed topology, readable non-holder snapshots, diagnostics, watcher/reload transitions, sync truthfulness, docs, and no-write boundaries; report any remaining defect with file/line evidence.
  REQUEST CHANGES (High): `state_checkout.rs:31-43,69-95` accepts a contained-looking state symlink whose canonical target is an unrelated external Git repository as Attached; `lib.rs:738-742` may then pull it. A disposable real-Git export reproduced `state: .spacedock-state` symlinked outside the definition directory as `SplitRoot` / `Attached`. Medium proof gaps remain at `lib.rs:168-279`: correction coverage no longer records state-root calls for re-probed Detached, WrongBranch, or ProbeFailed cases, and `lib.rs:746-756` has no regression for reload failure after a successful state pull.
- DONE: Run the focused correction regressions and required completion gates including full cargo test, cargo fmt --all -- --check, make lint, and git diff --check; return a direct APPROVE or REQUEST CHANGES verdict with checklist-complete evidence.
  Passed: 3 focused topology-sync regressions, 6 focused topology/parser tests, real-Git topology fixture, state reload integration, no-write guardrail, full `cargo test` (392 spacetop + 192 core unit tests and all enabled integration/doc tests; 3 watcher tests intentionally ignored), `cargo fmt --all -- --check`, `make lint`, and commit plus working-tree `git diff --check`.

### Summary

Verdict: REQUEST CHANGES. Commit `90e35c1` closes the cached-topology reload defect, and all required gates pass, but AC-1/AC-6 still permit a symlinked split-root declaration to authorize a fast-forward pull in an unrelated external repository; the negative sync proof matrix is also incomplete.
