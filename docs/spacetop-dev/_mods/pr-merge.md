---
name: pr-merge
description: Push branches and create/track GitHub PRs for workflow entities
version: 0.12.1-spacetop.2
---

# PR Merge

Manages the PR lifecycle for workflow entities processed in worktree stages. Pushes branches, creates PRs, detects merged PRs, and advances entities accordingly.

## Hook: startup

Scan all entity files (in the workflow directory only, not `_archive/`) for entities with a non-empty `pr` field and a non-terminal status. For each, extract the PR number (strip any `#`, `owner/repo#` prefix) and check: `gh pr view {number} --json state --jq '.state'`.

If `MERGED`, advance the entity to its terminal stage immediately: set `status` to the terminal stage, `completed` to ISO 8601 now, `verdict: PASSED`, clear `worktree`, archive the file, and clean up the clean worktree and local branch. Keep the remote branch while the merged PR references it. If `issue` is non-empty, resolve its repository and number, check its live state, and close it when still open. Report each auto-advanced entity and issue closure to the captain. The captain's "merge PR" instruction or report of a manual merge is explicit authorization for this finalization; do not ask for separate archive, issue-close, worktree-removal, or local-branch-deletion confirmation.

If `CLOSED` (closed without merge), report to the captain: "{entity title} has PR {pr number} which was closed without merging. How to proceed? Options: reopen the PR, create a new PR from the same branch, or clear `pr` and fall back to local merge." Wait for the captain's direction before taking action.

If `OPEN`, no action needed — the PR is still in review.

If `gh` is not available, warn the captain and skip PR state checks.

## Hook: idle

Check PR-pending entities using the same logic as the startup hook: scan entity files for non-empty `pr` and non-terminal status, run `gh pr view` for each, and advance merged PRs. This provides a periodic re-check in case the event loop's built-in PR scan missed a state change (defense in depth). Report any advanced entities to the captain.

## Hook: merge

**Automatic PR publication after verification approval.** The captain's approval of the `verify` gate is the approval to publish the implementation branch. Do NOT ask for a second push/PR approval after the `verify` gate is approved.

Before pushing, construct the full PR body so the exact prose that will land on GitHub is determined before publication.

Compute the audit-link inputs first: short SHA via `git rev-parse --short HEAD` in the worktree directory (if it exits non-zero — no commits, detached HEAD — substitute the literal string `main` and report the fallback to the captain); owner/repo via `gh repo view --json nameWithOwner --jq '.nameWithOwner'`; short entity-id slot via `spacedock status --short-id {entity ref}` from the workflow directory (shortest-unique-prefix for sd-b32 workflows, literal stored ID for sequential and slug, matching the status table's ID column).

Build the full PR body using the template below — motivation lead, `## What changed`, `## Evidence`, `---` separator, `[{short-id}](...)` audit link, and `Closes {issue}` line if frontmatter `issue` is set. This is the body that will be passed to `gh pr create` verbatim; do not reconstruct it after approval.

Report the publication summary to the captain while proceeding:

- **Title:** {entity title}
- **Branch:** {branch} -> main
- **Changes:** {N} file(s) changed across {N} commit(s)
- **Files:** {list of changed files}
- **Body:**

  ```
  {constructed body}
  ```

Then push main to ensure the remote is up to date with local state commits: `git push origin main`. Rebase the worktree branch onto main: `git rebase main` (from the worktree directory). Push the worktree branch: `git push origin {branch}`.

Then create the PR by running `gh pr create --base main --head {branch} --title "{entity title}" --body "{constructed body}"` against the body already constructed above — do not rebuild it. If `gh` is not available, warn the captain and stop with the merge mod-block still set.

If push, rebase, or PR creation fails because of remote state, authentication, rebase conflicts, or GitHub availability, report the failure to the captain and stop with the merge mod-block still set. Do not silently local-merge, because this workflow expects GitHub Copilot PR review to run after PR creation.

After PR creation, GitHub Copilot PR review is expected to trigger automatically. Start the automatic review-follow-up procedure below without waiting for another captain instruction.

### PR body template

Lead with motivation + end-user value; audit metadata goes at the bottom. The goal is that a reviewer or future debugger sees the "why" first and the audit link last.

**Template structure (top to bottom):**

| Section | Required | Content |
|---|---|---|
| Motivation lead | **yes** | 1 sentence, ≤ 25 words, blending motivation and end-user value. No parentheticals. |
| `## What changed` | **yes** | Action-verb bullets, 3–5 total, each ≤ 15 words. One change per bullet. No rationale inside the bullet — if a change needs justification, it belongs in the task body, not the PR. |
| `## Evidence` | **yes when validation ran** | Test suites with `N/N passed` format, 1–2 bullets. Do not include per-test-class breakdowns or enumerated suite lists — one pass ratio per suite, plus at most one line confirming live-probe verification. |
| `## Review guidance` | optional | 1 line pointing reviewer at the critical file or risky change — include only when a stage report explicitly flagged it |
| `---` separator + `[{entity-id}](/{owner}/{repo}/blob/{short-sha}/{path-to-entity-file})` | **yes** | Audit link, at the bottom |
| `Closes {issue}` | **yes when issue set** | Under the audit link, using the value exactly as it appears in frontmatter, e.g., `#48` or `owner/repo#48` |
| `Related: {siblings}` | optional | Under Closes, only when stage reports flagged follow-ups |

**Extraction rules (apply deterministically from the entity file):**

| PR body section | Source in entity file | Transformation |
|---|---|---|
| Motivation lead | Entity body paragraph(s) between closing `---` and the first `##` heading | Condense first paragraph to 1-2 sentences. Lead with impact or action verb — not "This PR" or "This task". Blend motivation + value. |
| What changed | Implementation stage report's `[x]` DONE items | One action-verb bullet per meaningful unit. Collapse sibling bullets that describe the same thing. Drop `[x]` markers. Do NOT include "what we deliberately did NOT change" bullets — scope boundaries belong in the task body, not the PR, unless a validation stage report flagged them as risk. |
| Evidence | Validation stage report items that assert AC verification (typically rerun-test items) | One bullet per suite with `N/N passed` format. Include any quantitative result the stage report explicitly called out (wallclock delta, size %, perf). Fallback to implementation report's self-test items if no validation stage exists. |
| Review guidance | Explicit "focus on X" / "risk here" notes in either stage report | 1 line. **Omit if no such note exists.** |
| Audit link | Short entity id from `spacedock status --short-id {entity ref}` (shortest-unique-prefix for sd-b32, literal stored ID for sequential and slug), path from the file's repo-relative location, short SHA from `git rev-parse --short HEAD` run in the worktree directory | Format as `[{short-id}](/{owner}/{repo}/blob/{short-sha}/{path})` |
| Closes | Entity frontmatter `issue` field (exactly as written) | Prefix `Closes ` |
| Related | Explicit "related task" / "follow-up" mentions in stage reports | 1 line. **Omit if none.** |

Target total length: **60-120 words**.

**Key design decisions:**

1. **Lead with motivation + end-user value.** First content is a 1-2 sentence user-facing impact statement. The audit link moves to the bottom as audit metadata.
2. **Prescribed sections + extraction rules** — not a strict verbatim template, not free-form. The mod specifies headings and source subsections; the FO paraphrases rather than pasting.
3. **Evidence section is conditional on validation stage.** Non-validated workflows fall back to implementation self-test evidence.
4. **Review guidance and Related are opt-in.** They appear only when stage reports explicitly flagged them, to prevent bloat.

Set the entity's `pr` field to the PR number (e.g., `#57`). Report the PR to the captain.

Do NOT archive yet. The entity stays at its current stage with `pr` set until the PR is merged. The FO handles advancement to the terminal stage and archival when it detects the merge (via the event loop PR check, idle hook, or startup hook).

## Automatic PR Review Comment Follow-Up

Immediately after creating a PR for an entity:

1. Record a review deadline exactly seven minutes after successful PR creation. Wait with the host's interruptible recurring monitoring mechanism in intervals no longer than 60 seconds so captain input remains responsive. An interruption returns control but does not cancel, complete, or duplicate the pending review pass; resume monitoring the same deadline when idle.
2. At or after the deadline, refresh the PR's live head, checks, and every unresolved review thread, including GitHub Copilot comments.
3. Fix every actionable comment on the same worktree branch. Run the relevant focused checks and the workflow's required completion gates, commit only task-related files, and push the updated branch.
4. Reply to every reviewed thread individually with the fix and evidence, or with a concise reason when no change is appropriate. Resolve every addressed thread for which the host has authorization.
5. Refresh the thread list once more and confirm that every thread observed by this review pass is answered and resolved. Do not ask the captain to select comments, and do not merge automatically.

The captain may still say "check PR review comments" to request an immediate additional pass; use the same fix, verify, push, reply, and resolve behavior without waiting seven minutes.

Keep the entity PR-backed and unarchived until the PR is merged.

## Captain Merge And Manual-Merge Follow-Up

When the captain says "merge PR", refresh the live PR head and base, required checks, unresolved review threads, and mergeability. If the PR is ready, merge it without another confirmation. If it is not ready, report the concrete blocker and retain the merge mod-block.

When the captain says the PR was merged manually, verify `gh pr view {number} --json state` reports `MERGED`; never rely on the statement alone for terminal state.

After either path confirms `MERGED`, execute the `MERGED` finalization from the startup hook immediately: archive the entity with `verdict: PASSED`, close a linked issue if it remains open, remove the clean worktree, delete the local feature branch, publish state, and report completion. Do not ask for another confirmation. Keep the remote feature branch while the merged PR references it.
