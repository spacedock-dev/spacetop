---
id: 077
title: Investigate split-root state containment warning
status: shape
source: "Captain report: State topology unverified: state directory resolves outside workflow definition directory"
kind: bug
risk: high
milestone: v1-maintenance
proof: Real filesystem and Git topology reproducer plus classifier and sync-call evidence
started:
completed:
verdict:
score: 0.76
worktree:
issue:
pr:
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
