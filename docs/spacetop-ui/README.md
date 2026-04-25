---
commissioned-by: spacedock@0.10.1
entity-type: ui_task
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
    - name: implement
      worktree: true
    - name: review
      fresh: true
      feedback-to: implement
      gate: true
    - name: done
      terminal: true
---

# SpaceTop Classic Refined Dashboard Layout

This workflow tracks the implementation of the "classic refined" dashboard layout for SpaceTop — a Rust TUI for inspecting Spacedock workflow state. Each task refines a specific UI component: the stage graph ribbon, task list panel, or preview pane. Work moves from clarifying the design intent, through Rust implementation in an isolated worktree, through human review, to completion.

## File Naming

Each task lives as either:

- a flat markdown file `{slug}.md` (default — use this unless the task produces many artifacts), or
- a folder `{slug}/` containing `index.md` as the canonical entity file, when the task produces per-stage artifacts (draft versions, output screenshots) that belong alongside the tracker.

Slugs are lowercase, hyphens, no spaces. Example: `refine-graph-ribbon.md` or `refine-graph-ribbon/index.md`. The status scanner recognizes both forms; `--set` and `--archive` resolve the slug either way, and folder entities archive as a whole folder into `_archive/{slug}/`.

## Schema

Every task file has YAML frontmatter. Fields are documented below; see **Task Template** for a copy-paste starter.

### Field Reference

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier, format determined by id-style in README frontmatter |
| `title` | string | Human-readable task name |
| `status` | enum | One of: design, implement, review, done |
| `source` | string | Where this task came from |
| `started` | ISO 8601 | When active work began |
| `completed` | ISO 8601 | When the task reached terminal status |
| `verdict` | enum | PASSED or REJECTED — set at final stage |
| `score` | number | Priority score, 0.0–1.0 (optional). |
| `worktree` | string | Worktree path while a dispatched agent is active, empty otherwise |
| `issue` | string | GitHub issue reference (e.g., `#42` or `owner/repo#42`). Optional cross-reference, set manually. |
| `pr` | string | GitHub PR reference (e.g., `#57` or `owner/repo#57`). Set when a PR is created for this entity's worktree branch. |

## Stages

### `design`

The worker clarifies exactly what visual and behavioral change this task must achieve in the SpaceTop TUI, referencing the current `src/ui/mod.rs` and `src/ui/graph.rs` layout, the ratatui API, and the "classic refined" aesthetic goal.

- **Inputs:** Current `src/ui/mod.rs` implementation, existing test assertions in the same file, current screenshot or mental model of the terminal layout, and the "classic refined" design intent (clean borders, labeled sections, tight visual hierarchy).
- **Outputs:** Precise description of what changes to make (which functions, which layout constraints, which colors/styles), acceptance criteria that can be verified by running `cargo test`, and a note on any ratatui widgets or layout primitives to add.
- **Good:** Names the exact Rust functions to modify, describes the visual delta concisely (e.g., "add a title bar above the graph ribbon showing the workflow name"), and proposes a test assertion that proves the change.
- **Bad:** Describes the aesthetic without naming any code locations, or leaves the acceptance criteria too vague to verify with a test.

### `implement`

The worker makes the Rust code changes to `src/ui/mod.rs`, `src/ui/graph.rs`, or related files to achieve the design. Tests are updated or added to cover the new behavior. Changes are committed atomically.

- **Inputs:** Approved design notes from the design stage, current Rust source files, and `cargo test` output as a continuous feedback loop.
- **Outputs:** Modified source files with passing `cargo test`, a brief note on what changed and why any existing tests were adjusted.
- **Good:** All existing tests still pass; new visual behavior is covered by at least one test assertion; no unused imports or dead code introduced.
- **Bad:** Breaks existing tests without explaining why, adds code that's unreachable, or leaves layout logic hardcoded in a way that breaks at different terminal widths.

### `review`

A fresh worker (or the captain) reviews the implementation for correctness, code quality, and fidelity to the classic refined aesthetic. If the implementation misses the design intent or introduces regressions, it is rejected back to implement.

- **Inputs:** Git diff of the implementation, `cargo test` output, and the acceptance criteria from design.
- **Outputs:** Verdict PASSED or REJECTED with a short rationale. If REJECTED, a note on what specifically needs to change.
- **Good:** Checks that the visual change actually looks clean in a standard 80×24 terminal and wider; reads the diff for logic errors; ensures layout constraints are readable and maintainable.
- **Bad:** Rubber-stamps without checking the diff, or rejects without explaining what needs to change.

### `done`

The task has passed review and the code is on the main branch.

- **Inputs:** Merged branch, passing CI.
- **Outputs:** Archived task file with `verdict: PASSED` and `completed` timestamp.
- **Good:** The implemented change is visible in the running TUI and consistent with the broader classic refined layout.
- **Bad:** Archived before the branch is actually merged.

## Workflow State

View the workflow overview:

```bash
~/.claude/plugins/cache/spacedock/spacedock/0.10.1/skills/commission/bin/status --workflow-dir docs/spacetop-ui
```

Output columns: ID, SLUG, STATUS, TITLE, SCORE, SOURCE.

Include archived tasks with `--archived`:

```bash
~/.claude/plugins/cache/spacedock/spacedock/0.10.1/skills/commission/bin/status --workflow-dir docs/spacetop-ui --archived
```

Find dispatchable tasks ready for their next stage:

```bash
~/.claude/plugins/cache/spacedock/spacedock/0.10.1/skills/commission/bin/status --workflow-dir docs/spacetop-ui --next
```

Find tasks in a specific stage:

```bash
grep -l "status: design" docs/spacetop-ui/*.md
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

Brief description of this task and what UI change it achieves.

## Acceptance criteria

Each AC names a property of the finished UI (not a stage action) and how it is verified.

**AC-1 — {End-state visual property.}**
Verified by: {cargo test assertion / grep on rendered buffer / command a future reader can reproduce.}
```

## Commit Discipline

- Commit status changes at dispatch and merge boundaries
- Commit task body updates when substantive
