---
title: Explain status labels in plain language
status: plan
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-22T09:48:59Z
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
id: 087
---

Help new users understand workflow status and where they can act without learning Spacedock terminology first.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: app-state / UI / docs
- Add short explanations for gate readiness, workflow coordinator activity, and storage warnings on their existing surfaces.
- Keep precise technical labels available and derive explanations from typed facts.
- Examples: awaiting captain means waiting for your decision; approved-awaiting-merge means approved but not yet merged; FO means workflow coordinator.
- Non-goals: Renaming canonical states or JSON fields, changing readiness rules, or restoring inferred activity to task rows and preview headers.

## Acceptance criteria

**AC-1 -- Explanations distinguish states without specialist vocabulary.**
Users can distinguish a pending decision, approved work awaiting merge, coordinator activity, and missing progress files. Explanations do not turn unknown information into a healthy or completed state.
Verified by: Representative state scenarios and a terminal walkthrough where a new reader explains each state.

**AC-2 -- Precise facts remain available.**
Users can inspect the original technical status. Existing machine-readable values and readiness semantics remain unchanged.
Verified by: App scenarios and existing headless contract checks for the affected states.

**AC-3 -- Suggested actions respect product limits.**
Explanations identify where an action belongs when that is known. They never imply that Spacetop can approve requests, move tasks, repair checkouts, or authenticate recorded actors.
Verified by: Walkthroughs of approval and storage-warning scenarios, including unknown action destinations.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.
