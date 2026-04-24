---
commissioned-by: spacedock@0.10.1
entity-type: development_task
entity-label: task
entity-label-plural: tasks
id-style: sequential
stages:
  defaults:
    worktree: false
    concurrency: 2
  states:
    - name: design
      initial: true
    - name: plan
    - name: implement
      worktree: true
    - name: review
      fresh: true
      feedback-to: implement
      gate: true
    - name: done
      terminal: true
---

# SpaceTop Dev

This workflow manages development of SpaceTop, a Rust-based terminal UI for inspecting Spacedock workflow state and helping users understand workflow structure. Each task moves from product and technical design through implementation and review, with the workflow state stored as markdown so progress remains auditable in git.

## File Naming

Each task lives as either:

- a flat markdown file `{slug}.md` (default -- use this unless the entity produces many artifacts), or
- a folder `{slug}/` containing `index.md` as the canonical entity file, when the task produces per-stage artifacts (draft versions, transcripts, outputs) that belong alongside the tracker.

Slugs are lowercase, hyphens, no spaces. Example: `my-feature-idea.md` or `my-feature-idea/index.md`. The status scanner recognizes both forms; `--set` and `--archive` resolve the slug either way, and folder entities archive as a whole folder into `_archive/{slug}/`.

## Schema

Every task file has YAML frontmatter. Fields are documented below; see **Task Template** for a copy-paste starter.

### Field Reference

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier, format determined by id-style in README frontmatter |
| `title` | string | Human-readable task name |
| `status` | enum | One of: design, plan, implement, review, done |
| `source` | string | Where this task came from |
| `started` | ISO 8601 | When active work began |
| `completed` | ISO 8601 | When the task reached terminal status |
| `verdict` | enum | PASSED or REJECTED -- set at final stage |
| `score` | number | Priority score, 0.0-1.0 (optional). Workflows can upgrade to a multi-dimension rubric in their README. |
| `worktree` | string | Worktree path while a dispatched agent is active, empty otherwise |
| `issue` | string | GitHub issue reference (e.g., `#42` or `owner/repo#42`). Optional cross-reference, set manually. |
| `pr` | string | GitHub PR reference (e.g., `#57` or `owner/repo#57`). Set when a PR is created for this entity's worktree branch. |

## Stages

### `design`

The first officer or assigned worker sets this status while clarifying what the task should accomplish for SpaceTop users and maintainers.

- **Inputs:** The task description, repository docs, current SpaceTop code, relevant Spacedock workflow examples, and user-facing requirements.
- **Outputs:** Clear problem statement, target user flow, acceptance criteria, and parser/TUI constraints that will guide implementation.
- **Good:** Explains how the task improves inspection of Spacedock workflows and names the expected terminal behavior or data model outcome.
- **Bad:** Starts coding decisions before confirming what workflow state or user interaction the feature must support.

### `plan`

The first officer or assigned worker sets this status after design is accepted and the task needs an implementation path.

- **Inputs:** Approved design notes, existing Rust module boundaries, current crate dependencies, and expected verification commands.
- **Outputs:** Step-by-step implementation plan, focused test strategy, and any module or file ownership notes needed for worktree execution.
- **Good:** Separates parser/state work from terminal rendering work so logic remains testable without a TUI session.
- **Bad:** Lists vague tasks without commands, expected files, or evidence needed to prove the work is complete.

### `implement`

A worker sets this status while making code or documentation changes for the task, usually in an isolated worktree.

- **Inputs:** Approved plan, SpaceTop source tree, Rust tooling, representative Spacedock workflow fixtures, and any stage reports already recorded in the task body.
- **Outputs:** Working implementation, focused tests or fixtures, updated docs where behavior changes, and a concise stage report with commands run.
- **Good:** Uses established Rust crates and keeps parsing, app state, and TUI rendering behind understandable boundaries.
- **Bad:** Mutates Spacedock workflow state by default, hides untested parsing assumptions in UI code, or changes unrelated project files.

### `review`

The first officer sets this status when implementation is ready for independent review and captain approval.

- **Inputs:** Implementation diff, test output, task acceptance criteria, and any screenshots or terminal output relevant to TUI behavior.
- **Outputs:** Review verdict, defects or missing evidence if rejected, and approval notes if the task can move to done.
- **Good:** Challenges whether the feature actually helps users inspect Spacedock state and whether parser failures are handled clearly.
- **Bad:** Rubber-stamps code without checking workflow examples, terminal edge cases, or test evidence.

### `done`

The first officer sets this terminal status when the captain accepts the reviewed task.

- **Inputs:** Approved review result, final implementation evidence, and any linked issue or PR reference.
- **Outputs:** Completed task record with final verdict, completion timestamp, and relevant artifact links.
- **Good:** Leaves enough context for future sessions to understand what shipped and how it was verified.
- **Bad:** Marks work done while review feedback, missing tests, or unresolved workflow-state questions remain open.

## Workflow State

View the workflow overview:

```bash
/Users/kent/Dev/InfuseAI/GitHub/spacedock/skills/commission/bin/status --workflow-dir docs/spacetop-dev
```

Output columns: ID, SLUG, STATUS, TITLE, SCORE, SOURCE.

Include archived tasks with `--archived`:

```bash
/Users/kent/Dev/InfuseAI/GitHub/spacedock/skills/commission/bin/status --workflow-dir docs/spacetop-dev --archived
```

Find dispatchable tasks ready for their next stage:

```bash
/Users/kent/Dev/InfuseAI/GitHub/spacedock/skills/commission/bin/status --workflow-dir docs/spacetop-dev --next
```

Find tasks in a specific stage:

```bash
grep -l "status: design" docs/spacetop-dev/*.md
```

## Task Template

```yaml
---
id:
title: Task name here
status: design
source:
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

Brief description of this task and what it aims to achieve.

## Acceptance criteria

Each AC names a property of the finished entity (not a stage action) and how it is verified.

**AC-1 -- End-state property.**
Verified by: grep / test name / file path / command a future reader can reproduce.
```

## Commit Discipline

- Commit status changes at dispatch and merge boundaries
- Commit task body updates when substantive
