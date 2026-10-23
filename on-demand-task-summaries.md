---
title: Generate human-readable task summaries on demand
status: plan
source: "Captain selected Option A and requested an entity file, 2026-10-23"
kind: feature
risk: high
milestone: usability
proof: "Human comprehension walkthrough, snapshot/race scenarios, and adversarial provider-isolation checks; see acceptance criteria."
started: 2026-10-23T09:22:08Z
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
- Select Claude or Codex through user-level configuration, using non-interactive adapters for `claude -p` and `codex exec`. Claude uses a verified available Haiku-family model; Codex uses a verified available Luna-family model. Confirm exact model IDs and syntax from installed CLI metadata rather than guessing aliases or silently selecting another family. OMP is outside this task.
- Use the selected CLI's existing login and configuration; do not add app-managed API keys, separate accounts, or read/copy credential contents. A user needs only one available provider. Missing executable, login or requested model produces an actionable error with Original still usable; no silent provider fallback. Verify actual installed-version isolation rather than assuming flags are sufficient.
- Capture exact task input and source-copy identity. Bind results to workflow/task/source copy/input digest/provider/model/prompt version and request identity. Keep an in-memory cache only. Refresh outdated summaries only on explicit request.
- Run asynchronously with a visible indeterminate spinner and truthful short state such as `Generating summary…`; elapsed time is optional, but no invented percentage or ETA. Use the existing TUI tick/render loop without extra polling or network requests. Bind the indicator to the exact request/task; keep Original readable, navigation responsive and cancellation available. Stop animation on success, error, cancellation or timeout, and reject late results for a different selection. Bound input/output/time and clean up child processes. Explain provider data transmission and possible usage charges before first generation.
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
Verified by: browsing/opening previews causes zero provider requests; the first generation explains the selected provider, model, data transfer and possible usage charges. Missing executable/credentials/model, nonzero exit, malformed or empty output, oversized input/output, timeout, and cancellation leave Original usable. Oversized input is not silently truncated. Actual in-progress and terminal-state frames prove the request-bound spinner runs only while generation is active, with no invented percentage/ETA or extra requests. Navigation/late-result scenarios cannot attach progress to the wrong task. The UI remains responsive and child processes do not survive cancellation or application exit.

**AC-5 -- Both supported adapter paths enforce the summarization-only boundary.**
Verified by: adversarial task text requesting edits, shell execution, additional file reads, or external tool actions cannot trigger those actions. Cover both Claude/Haiku and Codex/Luna adapter paths, including unavailable executable/login/model behavior. Run real synthetic-data end-to-end checks for locally available configured providers and verify no project/workflow files change; record unavailable-provider live-evidence limits explicitly rather than claiming a live pass. A user only needs one available selected provider. Render provider output as untrusted text without terminal control effects. If isolation cannot coexist with existing login/configuration, escalate the concrete tradeoff rather than silently weakening the boundary or changing model family.

## Proof plan

- Lowest test layer: typed request/result and cache state tests; subprocess lifecycle tests for real failure/race boundaries; UI rendering/navigation checks.
- Required command: make lint plus relevant established tests; record commands actually run.
- Manual checks: actual debug-build TUI generation, an in-progress spinner frame and success/failure/cancel frames, navigation during generation, source changes, Original/diff switching, cancel and quit. Real provider smoke uses synthetic data with existing logged-in Claude/Haiku or Codex/Luna where available; both adapter paths require behavior coverage. Navigator retains illustrated acceptance and an immutable debug executable outside the worktree.
- Human check: identify the unfamiliar reader during planning, before declaring a verification-ready task.
- Docs/policy update needed: README/help/configuration guidance, narrow read-only-policy allowance for provider execution, provider prerequisites and isolation limitations. Do not add persistent storage roots in this task.

## Commit Discipline

- Commit status changes at dispatch and merge boundaries.
- Commit task body updates when substantive.
- Commit implementation work on the task worktree branch before signaling completion.

## Captain scope clarifications — 2026-10-23

These explicit decisions amend the original three-adapter scope; they do not waive the safety boundary or AC-1 human evidence. Navigator/FO runtime choices are unchanged.

- Existing-login decision: Kent said 「直接用 claude -p, codex exec , omp -p 就好了 不要管 API key 使用者的 claude, codex, or omp 應該至少有一個可以用的」. Source: parent `71fac6c1-962e-4f9f-abf3-34a04cc7c166`, message `0d79fe0c-98a6-440b-bc4a-040a2d5e6f04`, relayed by Navigator message `b5a93336-737c-41f2-b75a-c5e6ad7b3118`. Preserve existing CLI login/configuration; no new API keys/accounts or credential inspection.
- Superseding adapter/model decision: Kent said 「簡化一點好了 用 claude 或 codex 就好 然後選小 model Haiku 或 Luna」. Source: same parent, message `8e8b06b4-8642-4a13-80da-cb3ae4320bd7`, relayed by Navigator message `ab160757-06de-4878-8191-1b49fe4b1bb8`. Only Claude/Haiku and Codex/Luna remain; prior OMP findings are historical, not an implementation obligation. Do not substitute another model family if the requested one is unavailable.
- Progress decision: Kent said 「對了，在 agent 處理 summary 的期間，可以顯示動畫或進度」. Source: same parent, message `78640aa0-e894-472b-b818-2b6045cb852c`, relayed by Navigator message `91dac744-db43-42c5-a01d-96546f30960a`. Use a standard indeterminate spinner and truthful status within the existing render loop; preserve responsive Original/cancel and exact request binding.
- Authorized live-smoke ceiling: at most **six actual CLI launches total**, normally normal/adversarial/cancel for each of the two adapters, only after adequate local isolation readiness. Each uses at most 8 KiB synthetic input, 45 seconds, 64 KiB stdout and 16 KiB stderr; no automatic retry. Prior launches count toward this ceiling. **Zero actual provider launches used at this amendment.** This is not a dollar hard cap; report observable usage without inventing a cost. Use existing available login/configuration and the verified requested small model; no new model/account/budget questionnaire is required.
- Missing runtime/login/model is an explicit availability/evidence limit, not a reason to silently choose another provider or claim a live pass. Process-group-only cleanup has a demonstrated detached-child gap and cannot be accepted without an effective containment/lifecycle solution. Hooks, plugins, MCP, project discovery, write/exec restrictions, snapshot races, zero navigation requests and unfamiliar-human proof remain required.
