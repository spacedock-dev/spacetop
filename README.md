# Spacetop

Spacetop is a Rust terminal UI for browsing [Spacedock](https://github.com/clkao/spacedock) workflow state.

![Spacetop screenshot](assets/images/SpaceTop-screenshot.png)

Spacedock stores workflow progress as markdown files in git. A workflow directory typically contains a `README.md` that defines stages and gates, plus entity files with YAML frontmatter such as `id`, `title`, and `status`. Spacetop is intended to make those state files easier to inspect from the terminal.

## Installation

Install the latest released binary with one command:

```bash
curl -fsSL https://github.com/spacedock-dev/spacetop/releases/latest/download/install.sh | sh
```

The installer script is published with each GitHub Release, so later changes on
`main` cannot change this install path accidentally. It installs the latest
released binary, verifies the selected archive against the release `SHA256SUMS`
file, and installs `spacetop` to `~/.cargo/bin` by default. The supported binary
platforms are macOS Apple Silicon and Linux x64.

Override the install directory with an absolute path:

```bash
curl -fsSL https://github.com/spacedock-dev/spacetop/releases/latest/download/install.sh | SPACETOP_INSTALL_DIR=/usr/local/bin sh
```

## Goals

- Discover or open a Spacedock workflow directory.
- Parse workflow metadata and markdown work item frontmatter.
- Browse work items by status, stage, or file.
- Preview the selected item's markdown body and stage reports.
- Surface useful workflow signals such as pending gates, blocked items, stale items, and active work.

## Status

Spacetop is an active read-first TUI. It can discover workflows, open an explicit
workflow directory, parse active and archived work items, preview markdown,
render workflow graphs, show selected worktree state, open query-backed search,
timeline, metrics, activity, and relation views, auto-refresh filesystem changes,
read YAML user config, restore per-workflow session state, expose headless
query/export commands, and explicitly sync with `git pull --ff-only`.

The product contract remains read-only by default: Spacedock markdown files are
the source of truth, and state-changing features must be explicit and auditable.

## Headless CLI

No-argument and `--workflow-dir` invocations still launch the TUI:

```bash
spacetop
spacetop --workflow-dir docs/spacetop-dev
```

Headless subcommands resolve exactly one workflow. Direct workflow paths,
repository roots, and omitted paths use discovery; zero or multiple workflows
return a stable error and ask for `--workflow-dir`.

```bash
spacetop list --workflow-dir docs/spacetop-dev
spacetop list --workflow-dir docs/spacetop-dev --status verify --text sync --json
spacetop timeline 050 --workflow-dir docs/spacetop-dev --json
spacetop metrics --workflow-dir docs/spacetop-dev --json
spacetop activity --workflow-dir docs/spacetop-dev --json
spacetop export --workflow-dir docs/spacetop-dev --json
```

`list` supports `--scope active`, `--scope archived`, and `--scope all`.
When omitted, scope and sort follow the user config defaults. `export` requires
`--json` and emits the workflow definition, active entities, and archived
entities.

## Mouse

The TUI captures mouse input for its lifetime and releases it on every exit
path (normal quit, panic, and while a file is open in `$EDITOR`), so the
terminal never keeps swallowing clicks.

- **Click** an entity row to select it and open the preview in one action.
  In the workflow picker, clicking a row opens that workflow.
- **Scroll wheel** scrolls the panel under the cursor: the preview body
  when hovering the preview, the list selection when hovering the list.
- **Drag the divider** between the list and the preview to resize the
  split. Each placement (side-by-side or stacked) holds its own ratio for
  the session.
- **Shift+drag** selects text natively. While capture is on, terminals
  follow the standard convention that Shift+left-drag bypasses mouse
  capture (iTerm2, Terminal.app, kitty, WezTerm), so native selection and
  copy keep working.

## Configuration

Spacetop reads YAML config from `$XDG_CONFIG_HOME/spacetop/config.yaml`, falling
back to `~/.config/spacetop/config.yaml` when `XDG_CONFIG_HOME` is unset,
empty, or relative. It stores TUI session state under
`$XDG_STATE_HOME/spacetop/session.yaml`, falling back to
`~/.local/state/spacetop/session.yaml` when `XDG_STATE_HOME` is unset, empty, or
relative. Relative `HOME` values are ignored, so config and session paths are
derived only from absolute user config/state roots.

Config supports theme colors, default scope/sort, and the P3 view keybindings:

```yaml
theme:
  selection_bg: "#283454"
  footer_bg: "#3b4252"
defaults:
  sort: id
  scope: active
keybindings:
  search: "/"
  command: ":"
  timeline: "T"
  metrics: "M"
  activity: "A"
  relations: "R"
```

Malformed config falls back to built-in defaults and is shown as a warning in
the TUI. Invalid, duplicate, or reserved keybindings also fall back to defaults
with warnings. Config and session files are never read from or written into
Spacedock workflow directories.

## Safety

Spacetop should be read-only by default. The only current writes are the explicit
`Y` sync action (`git pull --ff-only`) and session persistence under the user
state path described above. Future workflow-state write features should make
state changes explicit and easy to audit through git.

## Development

### Expected Stack

- Rust
- Cargo workspace with `spacetop-core` for pure workflow logic and `spacetop`
  for the CLI/TUI binary
- `ratatui` for terminal UI rendering
- `crossterm` for terminal backend and input events
- `serde` and `serde_yaml` for structured metadata parsing
- `notify` for filesystem watching
- `walkdir` for workflow discovery
- `thiserror` and `anyhow` for structured errors at the right boundary

### Prerequisites

Install Rust, which includes the Rust toolchain and Cargo, using `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, verify both tools are available:

```bash
rustc --version
cargo --version
```

On a fresh clone, install the required Rust components once:

```bash
make bootstrap
```

This adds the `clippy` component to the active toolchain. `make lint` and
`make build` will refuse to run until clippy is available and will point you
back at this command.

### Common Commands

```bash
cargo fmt
cargo test
make lint
cargo run -p spacetop -- --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- list --workflow-dir docs/spacetop-dev --json
cargo run -p spacetop -- export --workflow-dir docs/spacetop-dev --json
```

### Workspace Layout

- `crates/spacetop-core/` contains domain, parser, discovery, watcher, git sync,
  and editor helpers. It has no terminal UI dependencies.
- `crates/spacetop/` contains the CLI, TUI app state, rendering, terminal event
  loop, and release-only Sentry setup.
- `tests/fixtures/` contains shared integration-test fixtures.

Release and versioning policy lives in `docs/release-policy.md`.

### Install Local Build

Contributors can still build and install from the current checkout:

```bash
make build
make install
```

By default, install places the binary at `~/.cargo/bin/spacetop`.

To install to a different location, override `PREFIX`:

```bash
make install PREFIX=/usr/local/bin
```

To remove the installed binary:

```bash
make uninstall
```
