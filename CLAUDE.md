# Spacetop

Read-only Rust TUI (ratatui + crossterm) for browsing [Spacedock](https://github.com/clkao/spacedock) workflow state — markdown files in git with YAML frontmatter (`id`, `title`, `status`).

## Safety: read-only by default

Spacetop must NOT mutate Spacedock workflow files. Treat the markdown tree as the source of truth. If a future feature needs writes, make them explicit and auditable in git.

## Lint Gate

Before marking any task complete, run:

    make lint

All clippy warnings are treated as errors (`-D warnings`). Fix every diagnostic before committing — do not bypass with `#[allow(...)]` unless justified.

## Commands

| Command | Purpose |
|---------|---------|
| `make build` | Release build (runs lint first; forwards `SENTRY_DSN`) |
| `make lint` | `cargo clippy --all-targets --all-features -- -D warnings` |
| `make install` | Build and install to `~/.cargo/bin` (override with `PREFIX=`) |
| `cargo test` | Run unit + integration tests |
| `cargo run -- -w <path>` | Open a specific workflow directory |
| `cargo run` | Discover workflows under the current git root |

## Module Layout

- `src/main.rs` — entry point; initializes Sentry in release builds only.
- `src/lib.rs` — `decide_app` (CLI → launch decision; testable without TUI) and `run_terminal` event loop.
- `src/cli.rs` — clap definition: `-w/--workflow-dir`.
- `src/discovery.rs` — scan a root for workflow directories.
- `src/parser.rs` — markdown + YAML frontmatter parsing for workflow README and work items.
- `src/domain/` — `WorkflowDefinition`, `StageDefinition`, `WorkItem`, oklch-based stage color assignment.
- `src/app.rs` — `App`, `AppMode`, `OverviewState`, `OverviewSession`; all UI-agnostic state transitions live here.
- `src/ui/` — ratatui rendering (`mod.rs` overview, `graph.rs` stage graph, `picker.rs` workflow picker).
- `src/watcher.rs` — `notify`-based filesystem watcher with polling fallback.
- `tests/` — integration tests that drive `decide_app` and the watcher without a terminal.

Keep parser, domain, and app-state logic testable without a terminal backend.

## Conventions

- Prefer established crates (`ratatui`, `crossterm`, `serde_yaml`, `pulldown-cmark`, `notify`, `walkdir`) over ad hoc string slicing.
- New behavior should land with tests in the same module (`#[cfg(test)] mod tests`) or `tests/` for integration.
- Stable user-facing strings (e.g. the zero-workflows stderr message) are pinned by tests — update both together.
- Sentry: only initialized in release builds with a non-empty `SENTRY_DSN` baked in by `build.rs`. Debug builds never send events.

## Environment

Copy `.env.example` to `.env` and source it before `make build` to embed the Sentry DSN. The DSN is a public write-only key.
