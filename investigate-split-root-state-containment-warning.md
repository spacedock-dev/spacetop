---
id: 077
title: Investigate split-root state containment warning
status: verify
source: "Captain report: State topology unverified: state directory resolves outside workflow definition directory"
kind: bug
risk: high
milestone: v1-maintenance
proof: Real filesystem and Git topology reproducer plus classifier and sync-call evidence
started: 2026-08-21T11:59:50Z
completed:
verdict:
score: 0.76
worktree: .worktrees/spacedock-ensign-investigate-split-root-state-containment-warning
issue:
pr:
mod-block: merge:pr-merge
---

Spacetop can report `State topology unverified: state directory resolves outside workflow definition directory: <path>` for a split-root workflow. The warning may be correctly rejecting a state path that symlinks into an unrelated repository, or it may be a false positive for a supported Spacedock checkout layout. Investigate the real topology first, then make the warning and sync behavior truthful without weakening the boundary that prevents Spacetop from pulling an unintended repository.

## Scope

- Kind: bug investigation with a conditional correctness fix
- Risk: high because the classification controls whether the explicit `Y` action may run `git pull --ff-only` against the state checkout
- Milestone: v1-maintenance
- Touches: split-root path resolution, canonical containment, Git checkout probing, app diagnostics, sync authorization, fixtures, and nearby documentation
- Non-goals: automatically relocating or repairing state checkouts; allowing arbitrary external repositories; checking out, attaching, committing, rebasing, or pushing workflow state; weakening fail-closed behavior before the affected topology is reproduced

## Acceptance criteria

**AC-1 -- The affected topology is reproduced and recorded as filesystem and Git facts.** The evidence identifies the README `state:` spelling, definition directory, lexical and canonical entity paths, relevant symlink chain, Git top level, branch disposition, and whether the same repository would be synced twice. User-specific path components are sanitized in durable reports.
Verified by: the smallest disposable filesystem/Git reproducer matching the reported warning plus read-only commands against an affected checkout when available.

**AC-2 -- The investigation distinguishes an intended safety warning from a classifier defect.** The conclusion explains exactly why canonical containment fails and categorizes the layout as an unsupported external escape, a supported Spacedock split-root layout, a path-alias/case issue, or another evidenced cause.
Verified by: comparison of the real topology with `split_root_entity_dir`, canonical containment, and Git top-level probe behavior; a prose guess is insufficient.

**AC-3 -- The expected user behavior remains safe and truthful.** Supported state snapshots remain inspectable with an accurate diagnostic; unsupported external targets remain non-pull-eligible; single-root or same-repository layouts cannot trigger a duplicate pull; and no topology is labeled healthy without a successful read-only probe.
Verified by: typed classifier assertions and GitRunner call recording for the diagnosed topology and its nearest safe counterexample.

**AC-4 -- Any confirmed product defect has a regression at the lowest practical layer.** If code changes are required, the real failure becomes a classifier/parser test and any changed sync eligibility has an exact state-root call assertion. If behavior is intentional, the diagnostic and documentation explain the remediation without implying that Spacetop will repair the checkout.
Verified by: focused tests that fail before the correction or stable user-facing string/documentation assertions for an intentional-warning conclusion.

**AC-5 -- Existing split-root safety and read-only contracts do not regress.** Attached, detached, wrong-branch, missing, probe-failed, external-symlink, reload, and no-write coverage remains green, and no Git write broader than the audited fast-forward pull is introduced.
Verified by: focused topology/sync/no-write suites, full `cargo test`, `cargo fmt --all -- --check`, `make lint`, and `git diff --check` when code changes.

## Proof plan

- Reproduce the warning with a disposable definition directory and materialized state checkout before deciding whether to change code.
- Capture lexical paths, canonical paths, symlink targets, `git rev-parse --show-toplevel`, and `git symbolic-ref --quiet --short HEAD` using read-only probes.
- Exercise the diagnosed topology through storage classification, workflow loading, the visible diagnostic, and sync call recording.
- Preserve the external-symlink rejection and cached-topology fail-closed regressions added by task 076.
- Run focused tests first; for a code change, run full `cargo test`, `cargo fmt --all -- --check`, required `make lint`, and `git diff --check`.

## Investigation evidence and decision

The captain supplied the affected target as `xxx`, so no original checkout was
available for read-only inspection. The smallest sanitized real-Git reproducer
uses `state: .spacedock-state`, definition `<tmp>/external-case/definition`, and
symlink `.spacedock-state -> ../external-state`. The lexical entity path is
`<tmp>/external-case/definition/.spacedock-state`; its canonical path and Git
top are `<tmp>/external-case/external-state`, outside the canonical definition
directory, while the branch is the expected `spacedock-state/definition`.
Definition and state Git tops differ, so this is not a duplicate pull; it is an
arbitrary external repository that must remain non-pull-eligible.

This topology produces the exact diagnostic `State topology unverified: state
directory resolves outside workflow definition directory:
<tmp>/external-case/external-state`. The classifier stops before any Git probe,
the snapshot remains readable, and sync call recording shows no call at either
the lexical or canonical external-state root. A contained plain state directory
has the opposite hazard: its Git top equals the definition Git top, so a second
state pull would duplicate the definition pull; current parent-checkout
classification also rejects it.

The live supported `spacetop-dev` holder is a useful counterexample. Its README
spells `state: .spacedock-state`; the shell exposes a case alias
`/Users/<user>/dev/...`, but canonical definition and entity paths consistently
use `/Users/<user>/Dev/...`. The state path is not a symlink, its Git top equals
the canonical entity path, and it is attached to
`spacedock-state/spacetop-dev`. Definition and state share a Git common directory
but have distinct worktree tops and branches, so one pull per worktree is not a
duplicate pull.

Decision: the containment warning is intentional for the reproduced canonical
escape; no classifier defect or case-alias false positive was found. The gap is
that `ProbeFailed { reason: String }` collapses policy rejection and probe
failure, leaving diagnostics and sync authorization stringly typed. The
original `xxx` topology remains unclassified beyond this warning's exact
canonical-escape precondition.

## Typed contract and implementation plan

1. **AC-1/AC-2 — own topology facts in core.** In
   `crates/spacetop-core/src/domain/mod.rs`, replace the free-form unverified
   reason with a typed `StateTopologyProblem` carried by
   `StateCheckoutDisposition::Unverified`. At minimum distinguish
   `OutsideDefinition { resolved_state }`,
   `CheckoutRootMismatch { actual_top }`, path-resolution failures, Git
   top-level probe failures, and branch-probe failures. In
   `state_checkout.rs`, preserve the current order: lexical validation,
   canonical containment, exact Git-top equality, then branch disposition.
2. **AC-3 — make authorization explicit and duplicate-safe.** Add a core-owned
   `StateSyncEligibility` decision with `NotApplicable`,
   `Eligible { checkout_root }`, and `Blocked { problem }`. Only a freshly
   reprobed `Attached` checkout whose canonical Git top is distinct from the
   canonical definition sync root is eligible. Canonical escape, same-top
   parent fallthrough, detached, wrong-branch, missing, and every unverified
   variant are blocked; a linked worktree with a shared common Git directory
   remains eligible because its top and branch are distinct.
3. **AC-3 — consume the decision at the side-effect boundary.** In
   `crates/spacetop/src/lib.rs::apply_pending_sync`, keep definition-first sync
   and mandatory reload/re-probe, then match only `StateSyncEligibility` before
   calling `git_sync::sync`. Record exactly one definition-root pull plus one
   distinct state-root pull for eligible attached state; record no state-root
   call for escaped or otherwise blocked state, and never issue a second pull
   when the state Git top equals the definition root.
4. **AC-4 — render typed remediation, not raw classifier prose.** Map
   `StateTopologyProblem` in `app/overview.rs`; keep `ui/footer.rs` a thin
   renderer. Pin an escaped-state message that says the snapshot is readable
   but sync is blocked, and update `README.md` to tell users to materialize the
   state checkout at the declared contained path. State explicitly that
   Spacetop will not move, relink, or repair a checkout. Update `AGENTS.md` and
   `docs/development-policy.md` only where the typed disposition/sync contract
   wording changes.
5. **AC-1 through AC-4 — prove at the lowest layers.** Extend
   `state_checkout.rs` unit tests with typed assertions for external escape,
   `/tmp` canonical aliasing, case/path aliases, parent fallthrough, and the
   contained attached counterexample. Extend
   `tests/state_checkout_fixtures.rs` to assert the sanitized real symlink facts
   and readable entity body. Add Ratatui `TestBackend` assertions for the exact
   escaped diagnostic and `lib.rs` `RecordingGitRunner` assertions for pull
   roots/counts: external `0`, same-top `0`, contained attached `1` state pull.
6. **AC-5 — preserve the full safety matrix.** Run focused topology, fixture,
   UI, reload, sync-call, and `no_write_git_calls` tests first. Then run
   `cargo test`, `cargo fmt --all -- --check`, `make lint`, and
   `git diff --check`; retain attached, detached, wrong-branch, missing,
   probe-failed, external-symlink, cached-topology/reload, and exact
   `git pull --ff-only` coverage. No watcher production behavior changes are
   planned, so ignored watcher tests are not required unless implementation
   changes that boundary.

## Stage Report: plan

- DONE: Reproduce the exact warning with the smallest real filesystem and Git topology, recording sanitized lexical, canonical, symlink, repository, branch, and double-sync facts.
  A three-case disposable topology plus the live supported holder recorded every requested fact; the original reported path was unavailable because it was supplied as `xxx`.
- DONE: Decide whether the warning is intentional or a classifier defect, and define a typed diagnostic and sync-eligibility contract that preserves external-repository fail-closed behavior and prevents duplicate pulls.
  The canonical-escape warning is intentional; the plan replaces stringly unverified state and implicit authorization with typed topology problems and an explicit distinct-root eligibility decision.
- DONE: Produce an implementation and proof plan mapped to AC-1 through AC-5, naming owned modules, lowest-layer fixtures/tests, user-facing diagnostic or documentation changes, sync call recording, and required full gates.
  The six-step plan names core/domain, classifier, app, UI, sync, fixture, documentation, call-recording, no-write, full test, format, lint, and diff gates.

### Summary

The warning correctly blocks a relative state path whose canonical target
escapes the workflow definition directory, while preserving readable state.
Implementation should keep that behavior and make its reason, remediation, and
sync eligibility typed, including an explicit same-Git-top duplicate-pull
guard and exact call-recording proof.

## Stage Report: implement

- DONE: Replace stringly unverified-state reasons with typed topology problems and an explicit sync-eligibility decision while preserving readable snapshots and fail-closed external-target behavior.
  Commit `da77411` adds `StateTopologyProblem`, `StateSyncEligibility`, and typed blockers; the real external-symlink fixture would fail if escaped content stopped loading or any Git probe preceded containment rejection.
- DONE: Consume the typed decision at the sync boundary so contained attached state receives exactly one distinct state pull, same-top state is never double-pulled, and escaped or otherwise blocked state receives no state pull; render actionable truthful diagnostics and update nearby documentation.
  RecordingGitRunner tests would fail on zero/multiple contained-state pulls, any external-state call, or a second same-top pull; the TestBackend test pins readable/blocked/materialize/no-repair guidance, and README/AGENTS/policy document the contract.
- DONE: Add lowest-layer real-topology, classifier, UI, and RecordingGitRunner regressions mapped to AC-1 through AC-5; keep the full topology and read-only safety matrix green, run cargo test, cargo fmt --all -- --check, make lint, and git diff --check, then commit only task-related deliverables.
  `cargo test --no-fail-fast`, `cargo fmt --all -- --check`, `make lint`, and `git diff --check` passed; topology tests cover attached, detached, wrong-branch, missing, typed probe failures, external symlinks, canonical aliases, reload, exact pull roots, and the no-write guard.

### Summary

Implemented the planned typed topology and sync-authorization contract in commit `da77411`, without broadening the audited Git write surface. Supported contained attached checkouts receive one canonical distinct-root pull; escaped, same-top, stale, or unverified state remains readable but fail-closed with truthful remediation.

## Stage Report: verify

- DONE: Independently attack the classification and typed contract across real external escape, supported contained linked checkout, same-top parent checkout, detached, wrong-branch, missing, and probe-failure cases; confirm readable snapshots remain separate from sync authorization (AC-1, AC-2).
  All 9 `topology_sync_tests` passed, real-Git fixtures passed, and a live `spacetop-dev` probe exported `SplitRoot` / `Attached` with a contained distinct top and expected branch.
- DONE: Verify exact side-effect behavior and user truthfulness: one distinct state pull only for eligible attached state, zero state pulls for escaped/same-top/blocked state, no false healthy status, and actionable materialize-without-auto-repair diagnostics (AC-3, AC-4).
  RecordingGitRunner, real-topology, and rendering regressions passed with no actionable findings; the no-write guardrail also passed.
- DONE: Cross-check AC-1, AC-2, AC-3, AC-4, and AC-5 against concrete diff/test evidence, inspect read-only guardrails and documentation, run the required focused and full verification including cargo test --no-fail-fast, cargo fmt --all -- --check, make lint, and git diff --check, then issue PASSED or REJECTED with actionable findings.
  AC-1 through AC-5 are evidenced. `cargo test --no-fail-fast` passed with 399 app tests, 197 core tests, and all enabled integration and documentation tests; 3 unchanged notify tests remained ignored. `cargo fmt --all -- --check`, `make lint`, and `git diff --check` passed.

### Acceptance-criteria evidence

- **AC-1:** The sanitized external symlink reproducer and live supported linked-checkout probe record state spelling, lexical and canonical paths, symlink behavior, Git top, branch, and duplicate-pull classification.
- **AC-2:** The external canonical escape is intentionally blocked, while the supported contained linked checkout remains attached; typed topology variants identify the actual classifier outcome without treating aliases as escapes.
- **AC-3:** RecordingGitRunner regressions prove exactly one distinct state pull for eligible attached state and zero state pulls for external, same-top, detached, wrong-branch, missing, or unverified state; rendering tests prevent a false healthy message.
- **AC-4:** Lowest-layer topology, sync-call, and TestBackend regressions cover the confirmed stringly-diagnostic and implicit-authorization defects; README guidance explains remediation without claiming automatic repair.
- **AC-5:** The full topology matrix, no-write guardrail, complete tests, formatting check, required lint, and diff check all passed.

### Verdict

PASSED. No actionable findings.
