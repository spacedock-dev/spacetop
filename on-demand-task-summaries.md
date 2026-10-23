---
title: Generate human-readable task summaries on demand
status: shape
source: "Captain selected Option A and requested an entity file, 2026-10-23"
kind: feature
risk: high
milestone: usability
proof: "Human comprehension walkthrough, snapshot/race scenarios, and adversarial provider-isolation checks; see acceptance criteria."
started:
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
id: 090
---

Help a person understand a long agent-authored task file without reading its full history. Add an explicitly requested AI summary in the preview while retaining the original text as the source of truth. The captain selected Option A: on-demand generation, not automatic generation while browsing.

## Scope

- Kind: feature.
- Risk: high, because launching coding-agent CLIs adds a new side-effect boundary.
- Milestone: usability.
- Touches: app-state / UI / configuration / subprocess execution / docs / policy.
- Provide Summary and Original views. Preserve recorded metadata, gate/attention facts, source access, and existing worktree-diff behavior. Choose the shortcut against the existing key map; S is a proposal, not a reserved binding.
- Aim for 150–250 words explaining purpose, recorded progress, remaining work/decisions, and important limitations or missing evidence. Say not recorded when the source does not answer. Distinguish reported verification from independently checked evidence; expose conflicting reports rather than inventing reconciliation.
- Select Claude, Codex, or omp through user-level configuration. Use provider-specific non-interactive adapters for claude -p, codex exec, and omp -p; no silent provider fallback. Verify actual installed-version behavior rather than assuming flags establish isolation.
- Capture exact task input and source-copy identity. Bind results to workflow/task/source copy/input digest/provider/model/prompt version and request identity. Keep an in-memory cache only. Refresh outdated summaries only on explicit request.
- Run asynchronously with visible progress, cancellation, bounded input/output/time, and child-process cleanup. Explain provider data transmission and possible usage charges before first generation.
- Explicitly amend the product read-only policy to permit only constrained summarizer execution. Disable or contain tools, hooks, plugins, MCP, and project-instruction discovery. Empty working directories and instructions alone are not security boundaries. Use executable/argument arrays, never source-derived shell commands.
- Non-goals: automatic generation on navigation, background batches, persistent summary caches, changes to canonical task files, workflow dispatch, autonomous task actions, linked-evidence crawling, or inferred status/approval changes.

## Acceptance criteria

**AC-1 -- A reader unfamiliar with a task can identify its purpose, recorded progress, and outstanding decision or limitation from the summary.**
Verified by: a planned walkthrough with an identified unfamiliar reader and recorded answers against source material. Agent-generated judgments do not substitute for this human evidence; any missing proof remains explicitly unmet unless the captain approves a documented scope decision.

**AC-2 -- Generated prose never substitutes for recorded workflow facts or hides missing proof.**
Verified by: tasks with rejected reviews, contradictory reports, and unperformed human checks retain those distinctions. Typed status/gate facts remain authoritative; original text and worktree-diff views remain reachable. Proposed source links, if included, resolve only to validated sections of the captured input.

**AC-3 -- Results remain bound to their exact task snapshot.**
Verified by: switch tasks/workflows/source copies and change input while generation runs; no result appears on the wrong selection or as current for changed input. Matching requests reuse cached results; stale summaries are visibly marked and require explicit regeneration.

**AC-4 -- Summary generation is explicit and failure does not impair inspection.**
Verified by: browsing/opening previews causes zero provider requests; the first generation explains the selected provider and data transfer. Missing executable/credentials, nonzero exit, malformed or empty output, oversized input/output, timeout, and cancellation leave Original usable. Oversized input is not silently truncated. The UI remains responsive and child processes do not survive cancellation or application exit.

**AC-5 -- Every supported adapter enforces the summarization-only boundary.**
Verified by: adversarial task text requesting edits, shell execution, additional file reads, or external tool actions cannot trigger those actions. Exercise each of the three declared adapters end-to-end with synthetic input and verify no project/workflow files change. Render provider output as untrusted text without terminal control effects. If an adapter cannot meet the boundary, escalate rather than silently weakening it or dropping support.

## Proof plan

- Lowest test layer: typed request/result and cache state tests; subprocess lifecycle tests for real failure/race boundaries; UI rendering/navigation checks.
- Required command: make lint plus relevant established tests; record commands actually run.
- Manual checks: actual debug-build TUI generation, navigation during generation, source changes, Original/diff switching, cancel and quit; real provider smoke for Claude, Codex, and omp using synthetic data and constrained configuration.
- Human check: identify the unfamiliar reader during planning, before declaring a verification-ready task.
- Docs/policy update needed: README/help/configuration guidance, narrow read-only-policy allowance for provider execution, provider prerequisites and isolation limitations. Do not add persistent storage roots in this task.

## Commit Discipline

- Commit status changes at dispatch and merge boundaries.
- Commit task body updates when substantive.
- Commit implementation work on the task worktree branch before signaling completion.
