---
title: Show tasks that need attention
status: plan
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-22T13:28:21Z
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
id: 086
---

Help a person unfamiliar with the project find work that needs attention and understand why it appears.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: domain / query / app-state / UI / docs
- Include review requests, recorded blockers, and missing information supported by existing workflow facts.
- Reuse the plain-language explanations from explain-status-in-plain-language.
- Non-goals: Automatic prioritization, delay or productivity guesses, agent dispatch, task transitions, or approvals.

## Acceptance criteria

**AC-1 -- A focused Needs attention view explains every inclusion.**
Each listed task shows the recorded reason it needs attention. Missing or invalid information is distinguished from a recorded blocker. Tasks without a qualifying fact do not appear.
Verified by: Query and app scenarios containing each supported reason and a task with no qualifying reason.

**AC-2 -- Users can reach the relevant evidence from a selected task.**
A person unfamiliar with the project can find a task awaiting review, explain why it appears, and open its report or supporting evidence without help.
Verified by: A terminal walkthrough using a representative workflow, including a missing-evidence case.

**AC-3 -- Runtime silence never means blocked work.**
No detected activity, old timestamps, and process absence alone cannot place a task in the view. Recorded workflow facts remain separate from detected AI activity.
Verified by: Negative query cases with idle or unavailable activity and no recorded attention reason.

**AC-4 -- Inspection remains read-only.**
The view does not approve requests, change stages, run agents, or modify workflow files.
Verified by: Before-and-after workflow file comparison during the walkthrough and existing write-safety guardrails.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.
