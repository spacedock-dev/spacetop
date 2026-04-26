# Spacetop Development - Agent Guidelines

## Project Context

Spacetop is a Rust terminal UI for inspecting Spacedock workflow state stored as markdown in git.

Spacedock workflows are plain-text state directories. A workflow normally contains:

- `README.md` with YAML frontmatter defining workflow metadata, stages, gates, defaults, and labels.
- Active work item files as `*.md` with YAML frontmatter such as `id`, `title`, `status`, `source`, `score`, `worktree`, `issue`, and `pr`.
- Optional folder-form work items as `{slug}/index.md`.
- Archived work items under `_archive/*.md` or `_archive/{slug}/index.md`.
- Optional `_mods/*.md` workflow modification files.

Spacetop treats these files as the source of truth. It should remain read-only unless a future feature explicitly adds auditable write support.

## Current Product Shape

The app is no longer just a scaffold. It currently provides a read-first TUI that can:

- Resolve the scan root from the current git repository and discover Spacedock workflows by `commissioned-by: spacedock@...` in README frontmatter.
- Accept an explicit workflow path with `-w` or `--workflow-dir`; direct workflow paths bypass discovery, while repo/root paths can discover workflows below them.
- Open one workflow directly or manage multiple discovered workflows in a tabbed overview.
- Switch workflows with arrow keys and open a picker overlay with `P` when multiple workflows are available.
- Parse workflow stage metadata, active items, archived items, and selected worktree copies.
- Merge `.worktrees/*/<workflow>` items into the active snapshot, preserving main-branch frontmatter for matching slugs while showing changed worktree bodies.
- Render a stage graph ribbon with counts, gates, worktree markers, feedback arcs, and graph-aware stage colors.
- Toggle between active and archived scopes with `a`.
- Preview selected markdown bodies with `Enter`, scroll preview content, and toggle preview wrapping with `w`.
- Auto-refresh a workflow after relevant filesystem changes using `notify`, with a polling fallback.

## Code Map

Keep module boundaries clear and testable:

- `src/cli.rs` owns the `clap` CLI definition.
- `src/lib.rs` owns launch decisions, discovery flow, terminal setup, event loop wiring, watcher lifecycle, and top-level `run`.
- `src/app.rs` owns app state, overview sessions, picker state, selection, key handling, reload semantics, archived scope state, and pending workflow switches.
- `src/domain/mod.rs` owns typed workflow data and stage color helpers.
- `src/parser.rs` owns README/work item parsing, archive loading, frontmatter splitting, status validation, `.worktrees` scanning, and worktree merge behavior.
- `src/discovery.rs` owns workflow discovery and git-root scan-root resolution.
- `src/watcher.rs` owns filesystem watching, event filtering, debounce, fallback backend selection, and refresh signaling.
- `src/ui/mod.rs` owns the main Ratatui layout, task list, preview pane, markdown rendering, help popup, footer, and workflow tabs.
- `src/ui/graph.rs` owns stage graph rendering. It supports `SPACETOP_ASCII=1` for ASCII graph glyphs.
- `src/ui/picker.rs` owns picker dialog rendering.
- `tests/` contains integration tests for launch/discovery behavior and the ignored real-backend watcher smoke test.

Parser, app-state, discovery, watcher, and UI rendering logic all have tests. Keep new behavior covered at the lowest practical layer before relying on terminal behavior.

## Rust/TUI Expectations

Prefer established crates and existing local patterns:

- `ratatui` for terminal rendering.
- `crossterm` for terminal events and backend integration.
- `clap` for CLI parsing.
- `serde` and `serde_yaml` for structured metadata parsing.
- `pulldown-cmark` for markdown preview rendering.
- `notify` for filesystem watching.
- `walkdir` for discovery walks.
- `thiserror` for domain-specific error enums and `anyhow` at top-level boundaries.

Do not hide parsing assumptions in UI code. If new workflow metadata is needed, parse it into domain/app state first and render from typed data.

## Workflow Parsing Rules

Preserve the current parsing contracts unless the task explicitly changes them:

- Workflow directories are identified by README frontmatter with `commissioned-by` starting with `spacedock@`.
- Discovery prunes `.git`, `.worktrees`, `node_modules`, `vendor`, `dist`, `build`, `__pycache__`, and `tests`.
- Active item loading ignores `README.md`, `_mods`, `_archive`, and nested non-item files.
- Status values must match stages from the workflow README.
- Archived parsing skips malformed archived entries but surfaces archive IO errors.
- Missing `_archive/` is not an error.
- Worktree discovery is intentionally separate from workflow discovery: `.worktrees` must not inflate workflow counts, but matching worktree item files can affect the item snapshot.

## UI/Input Expectations

Keep the TUI read-oriented, dense, and predictable:

- Preserve keyboard behavior documented in the help popup and footer.
- Keep preview/list behavior responsive for narrow and wide terminals.
- Avoid terminal-only logic in parser or app-state tests.
- For visual changes, prefer Ratatui `TestBackend` assertions when practical.
- Keep Unicode graph/list glyphs usable, and preserve ASCII fallbacks where `SPACETOP_ASCII=1` applies.

## Safety

- Do not mutate Spacedock workflow markdown by default.
- Preserve user changes and workflow state files.
- Do not rewrite or clean up docs workflow files unless the task specifically asks for workflow state changes.
- When write features are added later, make writes explicit, narrow, and easy to audit in git.
- Be careful with existing dirty worktrees. Ignore unrelated user changes; do not revert them.

## Build, Test, And Lint

Useful commands:

```bash
cargo fmt
cargo test
make lint
cargo run -- --workflow-dir docs/spacetop-dev
cargo test -- --ignored
```

`make lint` is the required completion gate. It runs:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

All warnings are errors. Fix every clippy diagnostic before marking a code task complete. The ignored watcher test exercises the real `notify` backend and is meant for local/manual verification when watcher behavior changes.

## Release/Telemetry Notes

`build.rs` provides `SENTRY_DSN` at compile time. `src/main.rs` initializes Sentry only for release builds when the DSN is non-empty, and captures top-level run errors. Debug and test builds should not send events.
