# Spacetop in Herdr

Open Spacetop to the right of the invoking agent with a shortcut. The first open
keeps focus on the agent. The next invocation focuses the existing inspector in
that workspace. Closing the inspector lets the next invocation open a new one.

## Support and prerequisites

Initial support: **macOS and Herdr 0.9.3 or later**. Version 0.9.3 (socket protocol
22) is the oldest tested version. Linux, Windows and remote machine sessions are
not verified. Later Herdr API changes can require an integration update.

Install Git and both `spacetop` and `spacetop-herdr`. The invoking local pane must
be in a Git repository or linked worktree. Building from source needs the Rust
toolchain and clippy. The plugin uses Rust binaries and has no scripting runtime.
Plugins execute ordinary user code; link only a checkout you trust.

## Install and choose a shortcut

From the Spacetop checkout:

```sh
make install
herdr plugin link "$PWD/plugins/herdr"
```

`make install` installs both binaries in `~/.cargo/bin` by default. Put that
directory on the PATH used when starting the Herdr server. A custom `BINDIR`
must likewise be on that PATH. This plugin does not register itself or edit your
configuration.

Add the following to your Herdr config (`~/.config/herdr/config.toml`, or the
file selected by `HERDR_CONFIG_PATH`). Choose an unused key if `prefix+v` is
already bound:

```toml
[[keys.command]]
key = "prefix+v"
type = "plugin_action"
command = "spacetop.sidecar.open"
```

```sh
herdr config check
herdr server reload-config
```

Focus the agent pane, then press your prefix (normally Ctrl+B), followed by v.
The action uses `HERDR_PLUGIN_CONTEXT_JSON`, the injected `HERDR_BIN_PATH`, and
the inherited socket/session environment. Do not run the helper directly from a
shell without the action's context.

The helper prefers an absolute executable `SPACETOP_BIN` override, then the
`spacetop` binary beside itself, then absolute directories on PATH. If needed,
set `SPACETOP_BIN` in the environment used to launch your Herdr server. It never
interpolates project paths into shell command text.

## Project and workflow selection

The focused pane's cwd selects its Git repository or linked worktree root,
including when the pane is in a subdirectory or the workspace points elsewhere.
The plugin's install directory is never used as a fallback. Paths with spaces
are passed as individual arguments. There is no explicit workflow hint and
Herdr does not infer which Spacedock workflow your agent currently runs.
Spacetop's normal discovery, workflow tabs and picker remain available.

Reuse is scoped to a workspace, with live pane metadata and a checked plugin
ownership response. If you switch that workspace to a different project, close
the old inspector before opening one for the new project. Sidecars in other
workspaces are not reused. Duplicate metadata matches fail rather than opening
another pane. The first open splits the invoking tab; reuse may focus an
existing inspector in another tab of the same workspace.

## Read-only boundaries

The launcher only reads Git context and requests Herdr pane operations. It does
not edit workflow markdown, alter Git history/index, repair checkouts or send
commands to the agent's terminal. Spacetop still permits its explicit `Y` sync
with the existing `git pull --ff-only` rules. Config/session persistence stays
under absolute user HOME/XDG paths; do not point those paths into a workflow.
The launcher lock stays in Herdr's absolute plugin state directory, outside the
invoking project.

## Failures and recovery

- **Missing helper:** install both binaries and make them available on the Herdr
  server's PATH. Check `herdr plugin log` for startup failures.
- **Missing Spacetop:** install it, or set an absolute executable `SPACETOP_BIN`.
- **No usable focused context:** focus a local agent inside a Git repository.
  Missing/relative/nonexistent cwd and contradictory workspace/tab IDs fail.
- **Herdr API/socket errors:** inspect `herdr plugin log`, session status and your
  installed API version. The helper never restarts or upgrades Herdr to recover.
- **Sidecar startup error:** check plugin logs and pane output. Correct the
  executable/project access and close/reopen the inspector.
- **Tagging failed after opening:** close the new inspector before retrying to
  prevent duplicates. Likewise close duplicate or stale project sidecars.
- **Lock timeout:** another open action may be running. Retry after it finishes.
  A crash can leave `open-*.lock` in `HERDR_PLUGIN_STATE_DIR` (injected into plugin
  actions). Only remove the named lock after confirming no open action runs.
  Locks wait at most three seconds and are not silently broken.

## Remove

Close the plugin's inspector panes, then:

```sh
herdr plugin unlink spacetop.sidecar
```

Remove the `[[keys.command]]` entry and run `herdr server reload-config`. From
this checkout, `make uninstall` removes both binaries from the same `BINDIR`
used to install them. Unlinking does not alter workflow files.

## Maintainer verification

`cargo test -p spacetop --test herdr_sidecar` exercises the real helper against
controlled command boundaries; Python 3 is needed only for those test fixtures.
The isolated live scenario and retained evidence are in
[`evidence/085/`](evidence/085/). Use temporary HOME, XDG config/state and agent
homes plus a named session. A config override alone does not isolate Herdr's
plugin registry. Never use the user's live session for this check.
