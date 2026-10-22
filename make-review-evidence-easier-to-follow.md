---
title: Make review evidence easier to follow
status: plan
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-22T14:47:06Z
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
id: 088
---

Help reviewers understand the question, read the main report, and follow the supporting evidence without searching through files.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: app-state / UI / docs
- Put the review question first and make its main report directly accessible in the gate browser.
- Explain missing or invalid evidence in plain language.
- Connect requested corrections to recorded later responses using task 081, inspect-correction-round-history, as the owner of correction-history parsing and typed records.
- Integration dependency: task 081 supplies correction-round facts. Do not duplicate its scanner or invent response links from prose.
- Non-goals: Approval controls, correction recording, worker routing, relaxed evidence checks, network fetching, or authenticated-person claims.

## Acceptance criteria

**AC-1 -- The review question and main report are easy to reach.**
The browser shows the selected review question first and provides a direct route to its main report and supporting items. Missing question or report data is explicit.
Verified by: App navigation scenarios and a terminal walkthrough with complete and incomplete review rooms.

**AC-2 -- Evidence failures explain their effect.**
Missing, invalid, or mismatched evidence has a clear explanation. Recorded decisions remain inspectable without falsely presenting failed evidence as verified.
Verified by: Existing safe evidence fixtures plus navigation scenarios for missing and tampered items.

**AC-3 -- Corrections lead to recorded responses.**
Reviewers can navigate from a requested correction to its recorded response and identify unresolved corrections. Links use the typed relationships owned by task 081; absent relationships remain unknown.
Verified by: Multi-round scenarios with linked responses, an unresolved correction, and an unrelated response that must not be linked.

**AC-4 -- Existing evidence safety stays intact.**
Navigation preserves path, size, digest, and exact Git revision boundaries. It neither writes workflow state nor treats recorded approval as permission to execute an action.
Verified by: Existing gate-room safety checks and a read-only terminal walkthrough.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.
