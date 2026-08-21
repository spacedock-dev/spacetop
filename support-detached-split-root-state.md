---
title: Surface detached split-root state checkouts
status: plan
source: "Captain request plus spacedock-dev/spacedock#677 and #630"
kind: feature
risk: high
milestone: v1-maintenance
proof: Detached, attached, wrong-branch, and missing split-root fixtures plus parser, app, Ratatui, watcher, and git-sync guardrails
started: 2026-08-21T06:46:19Z
completed:
verdict:
score: 0.88
worktree:
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
