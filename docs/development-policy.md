# Spacetop Development Policy

**Owner:** CTO policy for Spacetop maintainers and agents  
**Applies to:** all code, documentation, workflow, and architecture changes  
**Last reviewed:** 2026-06-11

## Current Status Snapshot

Spacetop is an active Rust TUI, not a scaffold. The current product can discover
Spacedock workflows, parse README metadata and work item frontmatter, browse
active and archived items, render workflow graphs, preview markdown, merge
selected worktree copies into the visible snapshot, auto-refresh filesystem
changes, and explicitly sync with `git pull --ff-only`.

The repository is still early enough that architecture decisions matter. The
v2 design under `docs/superpowers/specs/2026-06-11-spacetop-v2-design.md`
sets the current strategic direction: preserve the read-only product contract
while moving toward a git-aware, indexed core that can support TUI and headless
surfaces.

The main governance gaps this policy addresses are:

- Some docs lag behind the implementation.
- The product has gained enough behavior that ad hoc changes can blur module
  boundaries.
- Clean Code expectations need to be enforceable, not implied.
- Agents need a consistent way to ask for decisions without open-ended prompts.

## Non-Negotiable Product Contract

Spacetop is a read-first inspection tool for Spacedock markdown workflows.

- Workflow markdown is the source of truth.
- Spacetop must not rewrite workflow state by default.
- The current `Y` sync action is the only approved write path. It may run
  `git pull --ff-only` and nothing broader.
- Any future write feature requires an explicit design, a narrow command path,
  git-auditable output, and tests that prove no other write path exists.
- Parser failures should be visible and understandable; they should not be
  hidden by UI rendering fallback.

## Clean Code Rules For Rust

Clean Code in this repo means code that makes workflow state easier to inspect
and future changes harder to get wrong.

- Keep functions focused on one job. Split when a reader has to track parsing,
  state mutation, and presentation at the same time.
- Use names that describe workflow concepts, not implementation accidents.
- Model workflow facts with typed structs/enums before rendering them.
- Keep side effects at the boundary: filesystem, git, terminal, and environment
  access should be easy to identify and test.
- Prefer explicit error types (`thiserror`) inside domain modules and `anyhow`
  only at top-level orchestration boundaries.
- Avoid `unwrap`, `expect`, and `panic!` in production paths unless they protect
  a documented impossible invariant. They are acceptable in tests.
- Do not add abstraction just because two call sites look similar. Add it when
  it removes real duplication, names a stable concept, or enforces a boundary.
- Prefer boring Rust over clever Rust. Lifetimes, traits, and generics should
  earn their complexity.

## Architecture Boundaries

Current two-crate workspace boundaries:

- `spacetop-core` contains pure workflow logic and must not depend on terminal
  crates.
- Workflow data structures, including the `Entity` model, belong in
  `crates/spacetop-core/src/domain/mod.rs`.
- Frontmatter, README, entity, archive, and worktree parsing belong in
  `crates/spacetop-core/src/parser.rs` and
  `crates/spacetop-core/src/parser/*`.
- Discovery and git-root resolution belong in
  `crates/spacetop-core/src/discovery.rs`.
- Filesystem watching belongs in `crates/spacetop-core/src/watcher.rs`.
- Explicit git sync belongs in `crates/spacetop-core/src/git_sync.rs`.
- External file opening belongs in `crates/spacetop-core/src/editor.rs`.
- `spacetop` contains the CLI, TUI app state, terminal event loop, and Ratatui
  views.
- CLI parsing belongs in `crates/spacetop/src/cli.rs`.
- Launch decisions, terminal setup, event loop wiring, and watcher lifecycle
  belong in `crates/spacetop/src/lib.rs`.
- App state and input semantics belong in `crates/spacetop/src/app.rs` and
  `crates/spacetop/src/app/*`.
- Ratatui rendering belongs in `crates/spacetop/src/ui/*`.
- Integration checks belong in each owning crate's `tests/` directory, while
  shared fixtures remain in workspace-root `tests/fixtures/`.

Strategic v2 boundary:

- The core/index layer must not depend on terminal crates.
- TUI views should become thin consumers of queryable domain state.
- Git history and metrics must prefer fewer trustworthy numbers over richer but
  unreliable numbers.
- No async runtime is approved by default. Background threads plus channels are
  the current preferred pattern.

## Development Workflow

Every non-trivial change should follow this sequence:

1. Read `AGENTS.md`, this policy, and the files directly owned by the change.
2. Check `git status --short` and avoid touching unrelated user changes.
3. Identify the lowest practical test layer before editing.
4. Make a narrow change that preserves module ownership.
5. Update docs or workflow notes when behavior, commands, or architecture change.
6. Run `cargo fmt` when Rust code changes.
7. Run `cargo test` for code changes.
8. Run `make lint` before marking a code task complete.
9. Run `cargo test -- --ignored` only when real watcher backend behavior changes
   or the task explicitly asks for manual watcher verification.

Documentation-only changes do not require Rust formatting or clippy unless they
change documented commands, code examples, or policy that affects build behavior.

## Testing Policy

Test at the boundary where the behavior actually lives.

- Parser/schema behavior: parser unit tests and fixtures.
- App selection, input, reload, archive scope, and picker behavior: app tests.
- Discovery/root behavior: discovery unit tests or `tests/discovery_bypass.rs`.
- Watcher filtering/debounce behavior: watcher tests, plus ignored real backend
  smoke only when the backend itself matters.
- Git sync: `git_sync` tests and
  `crates/spacetop-core/tests/no_write_git_calls.rs`.
- Terminal-free core boundary:
  `crates/spacetop-core/tests/no_terminal_deps.rs`.
- Rendering behavior: Ratatui `TestBackend` assertions over manual screenshots
  whenever practical.

A test is not required for spelling-only docs changes, but every behavior change
needs reproducible evidence.

## Dependency Policy

Use the existing stack unless a new dependency clearly improves correctness or
removes substantial complexity:

- `ratatui` and `crossterm` for terminal UI.
- `clap` for CLI parsing.
- `serde` and `serde_yaml` for structured metadata.
- `pulldown-cmark` or the existing markdown renderer path for markdown preview.
- `notify` for watching.
- `walkdir` for discovery.
- `thiserror` for domain errors and `anyhow` at top-level boundaries.

Before adding a dependency, document why the standard library and existing crates
are insufficient.

## Documentation Policy

Docs are part of the product contract.

- Keep README status aligned with the current product.
- Keep `AGENTS.md` short enough to be used as an agent entrypoint.
- Put deeper governance in this file instead of scattering policy across task
  notes.
- If a task changes keyboard behavior, update footer/help expectations and tests.
- If a task changes workflow schema assumptions, update parser tests and the
  relevant docs together.
- Do not rewrite Spacedock workflow state files unless the task specifically
  asks for workflow state changes.

## Decision Protocol

When an agent needs user input, it must not ask an open-ended "what should I do?"
question. It should provide two or three concrete options, lead with the
recommendation, and explain tradeoffs.

Use this Decision Tabs format:

### Option A - Recommended: concise title

| Pros | Cons | Choose this when |
|------|------|------------------|
| Specific advantages | Specific costs or risks | The conditions that make this best |

### Option B - Alternative title

| Pros | Cons | Choose this when |
|------|------|------------------|
| Specific advantages | Specific costs or risks | The conditions that make this best |

### Option C - Conservative or ambitious title

| Pros | Cons | Choose this when |
|------|------|------------------|
| Specific advantages | Specific costs or risks | The conditions that make this best |

End with a direct recommendation. Do not make the user design the option set.

## Review Policy

Reviews should protect user-facing behavior and the read-only contract.

- Findings first, ordered by severity.
- Each finding needs a file/line reference and concrete impact.
- "Looks fine" is acceptable only after checking relevant tests and risky paths.
- Missing evidence is a real review result.
- Reject changes that move parsing assumptions into UI code, broaden git writes,
  skip required lint gates, or make terminal behavior untestable.

## Completion Gate

A task can be called complete only when:

- The change matches the requested scope.
- Unrelated user changes were not reverted.
- Tests appropriate to the changed layer were run or explicitly skipped with a
  reason.
- `make lint` passed for code changes.
- Docs and policy remain consistent with implementation.
- Any remaining risk is named plainly in the final response.
