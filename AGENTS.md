# Spacetop Development - Agent Guidelines

## Policy Authority

This file is the mandatory entrypoint for all agents working in this repository.
For non-trivial code, documentation, workflow, or architecture changes, also read
`docs/development-policy.md` before editing. This file is the canonical source of
repo policy; `docs/development-policy.md` is supporting detail and must stay
subordinate to this file. If they conflict, follow `AGENTS.md` and fix the policy
drift before continuing.

For code review tasks, all agents must also read
`docs/code-review-policy.md`. That file is the single maintained review policy
for Codex, Claude Code, and GitHub Copilot. Keep tool-specific instruction files
as loaders only; do not duplicate review rules there.

Authority order:

1. The user's current request.
2. This `AGENTS.md` repo contract.
3. Existing code, tests, and workflow state.

Tool-specific files such as `CLAUDE.md` and
`.github/copilot-instructions.md` may add setup requirements for that tool, but
they must not weaken the read-only, test, lint, or Clean Code rules here, or
the review rules in `docs/code-review-policy.md`.

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
- Read YAML config from `$XDG_CONFIG_HOME/spacetop/config.yaml` or
  `~/.config/spacetop/config.yaml` and persist per-workflow TUI session state
  under `$XDG_STATE_HOME/spacetop/session.yaml` or
  `~/.local/state/spacetop/session.yaml`; relative env-derived roots are ignored.
- Sync from the workflow's git remote with the explicit `Y` action, limited to
  `git pull --ff-only` and guarded by tests.

## CTO Development Policy

Spacetop is a read-first developer tool. Development should protect that product
shape while making the internals easier to reason about and test.

- **Read-only by default:** never mutate Spacedock workflow markdown unless a
  future task explicitly adds audited write support. The existing `Y` sync action
  is the only sanctioned workflow-adjacent write path and must stay
  `git pull --ff-only`. Config/session writes are allowed only under absolute
  user config/state paths, never inside workflow directories.
- **Clean Code is required, not aspirational:** small functions, clear names,
  typed boundaries, limited side effects, no hidden parsing in UI code, and no
  speculative abstractions.
- **Domain before UI:** parse workflow facts into typed domain/app state first;
  render from that state. UI code should not infer schema rules from strings.
- **Lowest practical test layer:** parser behavior belongs in parser tests,
  app/input behavior in app tests, and rendering behavior in Ratatui
  `TestBackend` tests before relying on manual terminal checks.
- **No stale project facts:** when behavior, commands, or architecture changes,
  update the nearby docs in the same change. The README must describe the current
  product, not the initial scaffold.
- **Conservative dependency policy:** prefer existing crates and standard Rust
  APIs. Add a dependency only when it removes real complexity or provides a
  proven domain capability.
- **Decision protocol:** if an agent needs product or architecture input, present
  two or three concrete options using the "Decision Tabs" format in
  `docs/development-policy.md`; lead with a recommendation and list pros/cons.

## Code Map

Keep module boundaries clear and testable:

- `crates/spacetop/src/cli.rs` owns the `clap` CLI definition.
- `crates/spacetop/src/lib.rs` owns launch decisions, discovery flow, terminal
  setup, event loop wiring, watcher lifecycle, and top-level `run`.
- `crates/spacetop/src/app.rs` and `crates/spacetop/src/app/*` own app state,
  overview sessions, picker state,
  selection, key handling, reload semantics, archived scope state, and pending
  workflow switches.
- `crates/spacetop-core/src/domain/mod.rs` owns typed workflow data, including
  the `Entity` model, and core-owned stage color helpers.
- `crates/spacetop-core/src/parser.rs` and `crates/spacetop-core/src/parser/*`
  own README/entity parsing, archive loading, frontmatter splitting, status
  validation, `.worktrees` scanning, and worktree merge behavior.
  `parser/readme.rs` parses the optional `state:` field into
  `WorkflowDefinition.state`; `parser/snapshot.rs::resolve_entity_dir` is the
  single pure helper that turns `(definition_dir, state)` into the entity
  directory, and `load_workflow_dir` / `sources.rs::load_archive` thread that
  resolved dir to the active and archive scans.
- `crates/spacetop-core/src/index.rs`, `query.rs`, and `sources.rs` own the
  v2 index/query spine; TUI code must consume `WorkflowIndex` through query
  methods instead of inferring schema rules from raw vectors.
- `crates/spacetop-core/src/discovery.rs` owns workflow discovery and git-root
  scan-root resolution.
- `crates/spacetop-core/src/watcher.rs` owns filesystem watching, event
  filtering, debounce, fallback backend selection, and refresh signaling.
- `crates/spacetop-core/src/git_sync.rs` owns the explicit read-refresh sync
  path and must remain limited to audited fast-forward pulls.
- `crates/spacetop-core/src/session_activity.rs` scans local agent session logs
  and reduces exact structured events into `EntityActivity`; `domain/mod.rs`
  owns the three visible states (`Idle`, `Running`, and `HumanGate`) plus the
  typed `Worker`/`FirstOfficer` handler carried only by `Running`. Detection
  requires canonical dispatch/session correlation and structured start, stop,
  FO-action, or approve/reject gate records. Process presence, mtimes, generic
  path mentions, and filesystem writes alone must remain idle.
- `crates/spacetop-core/src/editor.rs` owns opening selected files in an
  external editor/viewer path; it must not become a workflow-state writer
  without explicit policy change.
- `crates/spacetop-core/src/config.rs` and
  `crates/spacetop-core/src/session_state.rs` own user config/session models,
  absolute XDG/HOME path resolution, and YAML load/save behavior.
- `crates/spacetop/src/ui/mod.rs` and `crates/spacetop/src/ui/*` own Ratatui
  rendering, layout, task list, preview pane, markdown rendering, help popup,
  footer, workflow tabs, chrome, and definition/diff views.
- `crates/spacetop/src/ui/graph.rs` owns stage graph rendering. It supports
  `SPACETOP_ASCII=1` for ASCII graph glyphs.
- `crates/spacetop/src/ui/picker.rs` owns picker dialog rendering.
- `crates/spacetop-core/tests/no_terminal_deps.rs` enforces that
  `spacetop-core` does not depend on terminal crates.
- `crates/spacetop/tests/` contains bin-facing integration tests;
  `crates/spacetop-core/tests/` contains core guardrails and watcher smoke
  tests; `tests/fixtures/` remains at the workspace root.

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
- Split-root state: a README may declare `state:` to separate the definition
  directory (where `README.md` lives, what discovery returns) from the entity
  directory (where active `*.md` and `_archive/` are read). A relative `state:`
  resolves the entity directory to `definition_dir.join(state)`; `$inline`,
  empty, or absent keeps the entity directory equal to the definition directory
  (single-root). Resolution is always relative to the definition directory; an
  absolute `state:` or one with a `..` parent-traversal component is unsupported
  and falls back to single-root rather than escaping the definition directory.
  Discovery, the watcher, and `WorkflowDefinition.root` stay on the definition
  directory; only entity/archive scans follow `state:`. A declared-but-absent
  state checkout yields no entities rather than erroring (mirrors missing
  `_archive/`).
- Active item loading ignores `README.md`, `_mods`, `_archive`, and nested non-item files.
- Status values must match stages from the workflow README.
- Archived parsing skips malformed archived entries but surfaces archive IO errors.
- Missing `_archive/` is not an error.
- Worktree discovery is intentionally separate from workflow discovery: `.worktrees` must not inflate workflow counts, but matching worktree item files can affect the item snapshot.

## UI/Input Expectations

Keep the TUI read-oriented, dense, and predictable:

- Preserve keyboard behavior documented in the help popup and footer.
- Keep preview/list behavior responsive for narrow and wide terminals.
- Treat stable user-facing strings as test-pinned behavior. Update the relevant
  tests together with intentional message changes, including zero-workflow
  stderr and documented footer/help text.
- Avoid terminal-only logic in parser or app-state tests.
- For visual changes, prefer Ratatui `TestBackend` assertions when practical.
- Keep Unicode graph/list glyphs usable, and preserve ASCII fallbacks where `SPACETOP_ASCII=1` applies.

## Safety

- Do not mutate Spacedock workflow markdown by default.
- Do not broaden git writes. The static guardrail test
  `crates/spacetop-core/tests/no_write_git_calls.rs` must continue to prove
  workflow-adjacent git writes are limited to the audited `git pull --ff-only`
  sync path.
- Do not read or write config/session files in workflow directories. User config
  and session persistence may use only absolute XDG/HOME-derived paths.
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
make build
make install
cargo run -p spacetop -- --workflow-dir docs/spacetop-dev
cargo test -- --ignored
```

`make lint` is the required completion gate. It runs:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

All warnings are errors. Fix every clippy diagnostic before marking a code task complete. The ignored watcher test exercises the real `notify` backend and is meant for local/manual verification when watcher behavior changes.

## Release/Telemetry Notes

`crates/spacetop/build.rs` provides `SENTRY_DSN` at compile time.
`crates/spacetop/src/main.rs` initializes Sentry only for release builds when
the DSN is non-empty, and captures top-level run errors. Debug and test builds
should not send events.
