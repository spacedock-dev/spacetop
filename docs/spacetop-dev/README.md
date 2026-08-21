---
commissioned-by: spacedock@0.26.0
id: spacetop-dev
state: .spacedock-state
entity-type: development_task
entity-label: task
entity-label-plural: tasks
id-style: sequential
github-project:
  owner: spacedock-dev
  number: 2
stages:
  defaults:
    worktree: false
    concurrency: 2
  states:
    - name: shape
      initial: true
    - name: plan
    - name: implement
      worktree: true
    - name: verify
      fresh: true
      feedback-to: implement
      gate: true
    - name: done
      terminal: true
---

# Spacetop Dev

This workflow manages development of Spacetop, a Rust terminal UI for inspecting Spacedock workflow state. Each task moves from outcome shaping through implementation and independent verification, with workflow state stored as markdown so progress remains auditable in git.

The workflow is optimized for a read-first developer tool: preserve the read-only product contract, keep parser and app facts typed before rendering, and prove behavior at the lowest practical test layer.

## File Naming

Each task lives as either:

- a flat markdown file `{slug}.md` (default -- use this unless the task produces many artifacts), or
- a folder `{slug}/` containing `index.md` as the canonical entity file, when the task produces per-stage artifacts (draft versions, transcripts, outputs) that belong alongside the tracker.

Slugs are lowercase, hyphens, no spaces. Example: `my-feature-idea.md` or `my-feature-idea/index.md`. The status scanner recognizes both forms; `--set` and `--archive` resolve the slug either way, and folder entities archive as a whole folder into `_archive/{slug}/`.

## Schema

Every task file has YAML frontmatter. Fields are documented below; see **Task Template** for a copy-paste starter.

### Field Reference

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier, format determined by id-style in README frontmatter |
| `title` | string | Human-readable task name |
| `status` | enum | One of: shape, plan, implement, verify, done |
| `source` | string | Where this task came from |
| `kind` | enum | feature, bugfix, refactor, docs, workflow, or policy |
| `risk` | enum | low, medium, or high |
| `milestone` | string | Planning bucket such as `v1-maintenance`, `v2-p0`, `v2-p1`, `v2-p2`, or `v2-later` |
| `proof` | string | Short pointer to the expected verification path |
| `started` | ISO 8601 | When active work began |
| `completed` | ISO 8601 | When the task reached terminal status |
| `verdict` | enum | PASSED or REJECTED -- set at final stage |
| `score` | number | Priority score, 0.0-1.0 (optional). Workflows can upgrade to a multi-dimension rubric in their README. |
| `worktree` | string | Worktree path while a dispatched agent is active, empty otherwise. Once set on first dispatch into a `worktree: true` stage, it stays set across all non-terminal advancements and clears at terminal merge. |
| `issue` | string | GitHub issue reference (e.g., `#42` or `owner/repo#42`). Optional cross-reference, set manually. |
| `pr` | string | GitHub PR reference (e.g., `#57` or `owner/repo#57`). Set when a PR is created for this task's worktree branch. |
| `mod-block` | string | Pending mod-declared blocking action, format `{lifecycle_point}:{mod_name}`. |

## Stages

### `shape`

The first officer or assigned worker sets this status while deciding what the task should accomplish and whether the requested change is worth doing.

- **Inputs:** The task description, repository docs, current Spacetop code, relevant Spacedock workflow examples, user-facing requirements, and the development policy.
- **Outputs:** Problem statement, target user or maintainer outcome, scope boundaries, acceptance criteria, risk level, milestone, and the product contract touched by the task.
- **Good:** Names how the task improves inspection of Spacedock workflows and explicitly calls out parser, TUI, git, watcher, docs, workflow, or policy impact.
- **Bad:** Starts implementation planning before the user value, non-goals, and safety boundary are clear.

### `plan`

The first officer or assigned worker sets this status after shaping is accepted and the task needs an implementation path.

- **Inputs:** Approved shape notes, existing Rust module boundaries, current crate dependencies, expected verification commands, and any v2 migration constraints.
- **Outputs:** Step-by-step implementation plan, owned files/modules, lowest practical test layer, proof strategy, and any spike or fixture needed before risky work.
- **Good:** Separates parser/state work from terminal rendering work, names exact verification commands, and identifies docs or policy updates needed in the same change.
- **Bad:** Lists vague tasks without commands, expected files, evidence, or a plan to prove read-only and Clean Code boundaries.

### `implement`

A worker sets this status while making code or documentation changes for the task, usually in an isolated worktree.

- **Inputs:** Approved plan, Spacetop source tree, Rust tooling, representative Spacedock workflow fixtures, and any stage reports already recorded in the task body.
- **Outputs:** Working implementation, focused tests or fixtures, updated docs where behavior changes, and a concise stage report with commands run.
- **Good:** Uses established Rust crates, keeps parsing/app facts typed before UI rendering, commits the worktree branch, and records reproducible evidence.
- **Bad:** Mutates Spacedock workflow markdown by default, hides untested parsing assumptions in UI code, broadens git writes, or changes unrelated project files.

- When consuming a review round's findings, triage before fixing:
  - **Material** — breaks a value AC or a declared safety, security, data-integrity, or compatibility boundary reachable through the supported workflow. Fix it.
  - **Correct-but-disproportionate** — substantively right, but no value AC breaks and its trigger is outside the supported workflow. Record a decline and the condition that would make it material.
  - **Needs decision** — a genuine product or compatibility fork. Escalate to the first officer.

  Record every disposition in `### Feedback Cycles`. Narrowing an acceptance criterion to make a rejection pass requires captain approval; it is not a licensed implementation decision.

### `verify`

The first officer sets this status when implementation is ready for independent verification and captain approval.

- **Inputs:** Implementation diff, test output, task acceptance criteria, proof plan, and any screenshots or terminal output relevant to TUI behavior.
- **Outputs:** Verification verdict, defects or missing evidence if rejected, and approval notes if the task can move to done through the PR merge flow.
- **Good:** Challenges whether the change helps users inspect Spacedock state, checks parser failures and terminal edge cases, and confirms every acceptance criterion has evidence.
- **Bad:** Rubber-stamps code, treats prose as proof when a test or guardrail could enforce the behavior, or ignores required `make lint` evidence for code changes.

- **Small-change fast path:** Scale verification to the diff's blast radius. Routine low-risk changes do not require the full checklist or a detached adversarial audit.

### `done`

The first officer sets this terminal status when the captain accepts the verified task and the merge path has completed.

- **Inputs:** Approved verification result, final implementation evidence, and any linked issue or PR reference.
- **Outputs:** Completed task record with final verdict, completion timestamp, and relevant artifact links.
- **Good:** Leaves enough context for future sessions to understand what shipped, how it was verified, and why it matched the read-first product contract.
- **Bad:** Marks work done while review feedback, missing tests, unresolved workflow-state questions, or PR/local merge steps remain open.

## Proof Standard

Every task should prove the most important property at the lowest practical layer:

- Parser/schema behavior belongs in parser tests and fixtures.
- App state and input behavior belongs in app tests.
- Discovery/root behavior belongs in discovery or integration tests.
- Watcher behavior belongs in watcher tests, with the ignored real-backend smoke only when backend behavior changes.
- Git sync and write-safety behavior belongs in git-sync tests and no-write guardrails.
- Rendering behavior should use Ratatui `TestBackend` assertions before relying on manual terminal checks.

For code changes, `make lint` is the completion gate. A task may skip a command only when the stage report explains why it was not applicable or could not run.

## Workflow-specific rules

The first-officer and ensign contracts govern generic stage semantics and proof discipline. The rules below add the development-workflow specifics.

- **Repo-mutation worktree layer.** `implement` runs in a dedicated worktree and `verify` uses a fresh agent. PR state lives on the `pr` field and is managed by the `pr-merge` mod; there is no `pr_open` or `awaiting_merge` stage.
- **No prose-grep over instruction files.** A string, substring, or regex match over an instruction file never proves a behavioral claim. A one-off grep can prove presence or absence when that fact is itself the claim, but committing the same grep as a permanent behavioral test is tautological.
- **Evidence must be able to fail.** Each acceptance criterion's evidence names the concrete change that would make it fail. If the author cannot name that falsifying edit, the criterion is not proven.
- **Opt-in proof disciplines.** Adopt only the disciplines the task's risk requires:
  - **Test-first authoring** — for code or fixture deliverables, write the failing test first, observe the intended failure, then implement the minimum passing change.
  - **External-proof acceptance criteria** — cite a test, command result, produced file, or resulting on-disk state outside the task body.
  - **Detached adversarial audit** — for high-stakes launchers, mutation guards, workflow scaffolding, CI, or release machinery, use a throwaway checkout to try to refute the validation.
  - **Live scenario for runtime claims** — prove agent or model runtime behavior with a scripted before-and-after scenario, a durable result, and a negative case.
- **Declaring a posture is optional.** If useful, state project maturity, default test depth, infrastructure-addition policy, and review-finding priority here. Do not invent a posture solely to satisfy the template.

## PR And Review Comment Flow

Approving the `verify` gate authorizes PR publication. After that approval, the merge hook pushes the implementation branch and creates the GitHub PR directly, without asking for a second push or PR approval. GitHub Copilot PR review is expected to trigger automatically after PR creation.

After PR creation, start an interruptible seven-minute wait. When it completes, fetch all live GitHub review threads, including Copilot, fix every actionable comment on the PR branch, run the relevant checks, push, reply to each thread individually, and resolve every addressed thread. Reply with a concise reason when no code change is appropriate. Do not ask the captain to select comments. An interruption pauses monitoring but does not cancel or duplicate the pending review pass.

When the captain says "merge PR", refresh the live head, checks, unresolved threads, and mergeability, then merge without another confirmation when the PR is ready. When the captain reports a manual merge, verify it on GitHub. After either verified merge, immediately finalize and archive the task, close its linked issue if one is present and still open, and remove the clean worktree and local branch without another confirmation. Keep the remote branch while the merged PR references it.

## Workflow State

View the workflow overview:

```bash
spacedock status --workflow-dir docs/spacetop-dev
```

Output columns include ID, SLUG, STATUS, TITLE, SCORE, and SOURCE.

Include archived tasks with `--archived`:

```bash
spacedock status --workflow-dir docs/spacetop-dev --archived
```

Find dispatchable tasks ready for their next stage:

```bash
spacedock status --workflow-dir docs/spacetop-dev --next
```

Find tasks in a specific stage:

```bash
spacedock status --workflow-dir docs/spacetop-dev --where 'status = shape'
```

## Task Template

```yaml
---
id:
title: Task name here
status: shape
source:
kind:
risk:
milestone:
proof:
started:
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
---
```

Brief description of this task and the outcome it should create.

## Scope

- Kind:
- Risk:
- Milestone:
- Touches: parser / app-state / UI / git / watcher / docs / workflow / policy
- Non-goals:

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- End-state property.**
Verified by:

**AC-2 -- Safety or boundary property.**
Verified by:

## Proof plan

- Lowest test layer:
- Required command:
- Manual check, if any:
- Docs/policy update needed:

## Commit Discipline

- Commit status changes at dispatch and merge boundaries.
- Commit task body updates when substantive.
- Commit implementation work on the task worktree branch before signaling completion.
