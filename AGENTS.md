# Spacetop Development — Agent Guidelines

## Project Context

Spacetop is a Rust-based TUI for browsing Spacedock workflow state files.

Spacedock workflows are plain-text state directories. A workflow normally contains:

- `README.md` defining workflow stages, schema, gates, and work item template
- `*.md` work item files with YAML frontmatter such as `id`, `title`, and `status`
- optional `_mods/*.md` workflow modification files

Spacedock records workflow state in git as markdown. Spacetop should treat those files as the source of truth and avoid changing workflow state unless a future feature explicitly adds write support.

## Product Direction

The initial product shape should be a read-first terminal UI that helps users inspect workflow health quickly:

- discover or accept a workflow directory
- parse workflow README/stage metadata and work item frontmatter
- list work items by status/stage
- show the selected item's markdown body and stage reports
- surface gates, blocked items, stale items, and active work at a glance

## Rust/TUI Expectations

Prefer established Rust crates and clear module boundaries:

- `ratatui` for terminal UI rendering
- `crossterm` for terminal events/backends
- `serde`/`serde_yaml` for frontmatter parsing
- a markdown/frontmatter parser instead of ad hoc string slicing when practical
- focused modules for domain parsing, app state, UI views, and input handling

Keep parser and state logic testable without a terminal backend.

## Safety

- Do not mutate Spacedock workflow markdown by default.
- Preserve user changes and workflow state files.
- When adding write features later, make writes explicit and easy to audit in git.
