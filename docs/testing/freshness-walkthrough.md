# Local read and Git sync walkthrough

Use a debug build of the candidate. This walkthrough changes only disposable
local repositories. It needs Git, a POSIX shell, and a terminal. The app's `Y`
action still uses only the existing fast-forward sync path.

## Create the fixture

Run from the candidate worktree. Keep this shell open so the variables remain
available. All remotes are local paths; no network service or credentials are
needed. Setup commits are fixture operations, not Spacetop behavior.

```sh
freshness_candidate="$PWD/target/debug/spacetop"
cargo build -p spacetop
freshness_fixture=$(mktemp -d "${TMPDIR:-/tmp}/spacetop-freshness.XXXXXX")
export freshness_fixture freshness_candidate
mkdir -p "$freshness_fixture/config" "$freshness_fixture/user-state"
git init --bare --initial-branch=main "$freshness_fixture/definition.git"
git clone "$freshness_fixture/definition.git" "$freshness_fixture/definition"
git -C "$freshness_fixture/definition" config user.name 'Freshness Fixture'
git -C "$freshness_fixture/definition" config user.email fixture@example.invalid
mkdir -p "$freshness_fixture/definition/workflow"
cat > "$freshness_fixture/definition/workflow/README.md" <<'README'
---
commissioned-by: spacedock@0.27.3
state: .state
state-branch: main
stages:
  states:
    - name: plan
      initial: true
    - name: done
      terminal: true
---
# Freshness fixture
README
printf 'workflow/.state/\n' > "$freshness_fixture/definition/.gitignore"
git -C "$freshness_fixture/definition" add workflow/README.md .gitignore
git -C "$freshness_fixture/definition" -c commit.gpgsign=false commit -s -m 'Seed definition fixture'
git -C "$freshness_fixture/definition" push origin main
git init --bare --initial-branch=main "$freshness_fixture/state.git"
git clone "$freshness_fixture/state.git" "$freshness_fixture/definition/workflow/.state"
git -C "$freshness_fixture/definition/workflow/.state" config user.name 'Freshness Fixture'
git -C "$freshness_fixture/definition/workflow/.state" config user.email fixture@example.invalid
cat > "$freshness_fixture/definition/workflow/.state/001.md" <<'ENTITY'
---
id: 001
title: Local and shared progress
status: plan
---
This body can change locally without a Git sync.
ENTITY
git -C "$freshness_fixture/definition/workflow/.state" add 001.md
git -C "$freshness_fixture/definition/workflow/.state" -c commit.gpgsign=false commit -s -m 'Seed progress fixture'
git -C "$freshness_fixture/definition/workflow/.state" push origin main
cp "$freshness_fixture/definition/workflow/README.md" "$freshness_fixture/good-readme"
cp "$freshness_fixture/definition/workflow/.state/001.md" "$freshness_fixture/good-entity"
printf '%s\n' "$freshness_fixture"
git rev-parse HEAD
```

## Launch and inspect

Use 120x30, 80x24, and 40x20 terminals. Resize the terminal or use `stty cols 80
rows 24` before launch where supported. Capture the candidate SHA, dimensions,
executable path, actions, observations, and exit status. Freshness has three
reserved rows above the diagnostics. Below ten terminal rows use scrollable
help; the preview keeps its limited space.

```sh
XDG_CONFIG_HOME="$freshness_fixture/config" \
XDG_STATE_HOME="$freshness_fixture/user-state" \
GIT_TRACE="$freshness_fixture/git.trace" \
"$freshness_candidate" --workflow-dir "$freshness_fixture/definition/workflow"
```

1. On startup, `Local read` has an age. Both Git observations say `not observed
   this session`. At narrow widths `unknown` is the same session-only meaning.
2. Browse, open/close preview with Enter, wait, and resize without pressing `Y`.
   In a second shell append a line to `.state/001.md`. Wait for refresh (the
   polling fallback may take a few seconds). Local read advances; Git does not.
3. Restore `001.md` from `good-entity` after checking the local edit (a Git
   rebase preference can reject dirty files). Close preview with Enter, then
   press `Y` from the list. Both Git roots show ages, even if both pulls were already up to
   date. The trace must contain exactly the definition and eligible state pulls.
4. Replace the README with invalid text in the second shell. After refresh,
   `Local read` keeps its previous success age and shows `FAILED (?)`. Press `?`
   then End to read the full reason. Restore the README afterwards.
5. Make only the state remote unavailable using the command below, then press
   `Y`. Definition sync advances; state retains its old success with `FAILED`.
   This must never be presented as both roots succeeding.
6. Restore the state remote, quit (`q`), and restart. Git history is unknown
   again. Times are not restored from commits, mtimes, or saved UI preferences.
7. Quit, move `.state` aside, restart, and press `Y`. The local read can succeed
   with no progress items. State says `skipped (?)`; help explains missing
   state. Restore `.state` when finished. Optional detached/wrong-branch cases
   use fixture-only Git commands outside Spacetop; the app must not repair them.

Second-shell commands (substitute the printed absolute fixture path):

```sh
# Set freshness_fixture to the path printed during setup in this second shell.
printf '\nLocal-only edit\n' >> "$freshness_fixture/definition/workflow/.state/001.md"
cp "$freshness_fixture/good-entity" "$freshness_fixture/definition/workflow/.state/001.md"
printf 'invalid README\n' > "$freshness_fixture/definition/workflow/README.md"
# Inspect failure before restoring:
cp "$freshness_fixture/good-readme" "$freshness_fixture/definition/workflow/README.md"
git -C "$freshness_fixture/definition/workflow/.state" remote set-url origin "$freshness_fixture/unavailable.git"
# Inspect Y failure before restoring:
git -C "$freshness_fixture/definition/workflow/.state" remote set-url origin "$freshness_fixture/state.git"
# Quit before changing checkout topology:
mv "$freshness_fixture/definition/workflow/.state" "$freshness_fixture/held-state"
# Inspect missing state, then quit before restoring:
mv "$freshness_fixture/held-state" "$freshness_fixture/definition/workflow/.state"
```

## Audit and human evidence

Before and after the no-`Y` portion, run `git status --short` in both fixture
checkouts and hash README/001.md with `shasum`. Only the deliberate local edit
should change them. Inspect `git.trace` with `rg 'built-in: git pull'`: zero
pulls before `Y`, two per fully eligible `Y`, one for missing/blocked state.
Keep setup pushes separate from this product trace. The trace includes
read-only Git probes, which are not sync observations.

Automated tests cover successful pulls followed by failed rereads and changed
state target/branch. Real terminal output verifies visibility and interaction.
It does not establish human understanding. Ask the reviewer to explain, in
their own words, what local age and each Git age establish, which checkout a
partial result updated, and why a successful sync does not prove the remote has
no newer work. Keep that proof open if no explanation is recorded.

After exiting Spacetop, remove only this disposable fixture if no evidence is
needed: `rm -rf -- "$freshness_fixture"`. Do not use real workflow repositories
for failure injection or sync testing.
