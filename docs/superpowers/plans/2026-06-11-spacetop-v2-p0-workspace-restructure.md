# SpaceTop v2 — Phase P0: Workspace Restructure + Model Rename — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the single `spacetop` binary crate into a two-crate Cargo workspace — `spacetop-core` (pure logic, zero terminal dependencies) and `spacetop` (bin: TUI + CLI) — and rename the `WorkItem` model to `Entity`, with **behavior identical** and every existing test still passing.

**Architecture:** Strangler step zero from `docs/superpowers/specs/2026-06-11-spacetop-v2-design.md`. Two semantic refactors land **in place** first (sever the lone `domain → ratatui` dependency via a core-owned `Rgb` type; rename `WorkItem → Entity`), each verified against the full existing suite. Then a purely mechanical physical move relocates modules into the two crates and rewrites import paths. Finally a guard test enforces that `spacetop-core` never links a terminal crate.

**Tech Stack:** Rust 2021, Cargo workspaces, ratatui/crossterm/termimad/ratskin (bin only), serde_yaml/notify/walkdir/sha1/thiserror/anyhow (core), clap (bin).

---

## Hard constraints (apply to every task)

- **Behavior-identical.** No feature changes, no logic changes beyond the Color type swap and the name change. No async. Read-only contract untouched (`Y` sync stays `git pull --ff-only`).
- **Green at every commit.** `cargo test` (or the relocated equivalent) and `make lint` pass before each commit. After the workspace exists, run `cargo test --workspace`.
- **Policy.** Follows `AGENTS.md` + `docs/development-policy.md`: Clean Code, lowest-test-layer, conservative deps (no new runtime crate is added), docs updated in the same change.
- **Do not revert unrelated user changes.** Check `git status --short` before starting; only touch files this plan names.

## Current → target file map

**Move to `crates/spacetop-core/src/` (pure logic):**
`domain/mod.rs`, `parser.rs` + `parser/*`, `discovery.rs`, `watcher.rs`, `git_sync.rs`, `editor.rs`.

**Move to `crates/spacetop/src/` (TUI + CLI):**
`main.rs`, `lib.rs` (the `run`/`decide_app`/`run_terminal`/OSC-7 orchestration), `cli.rs`, `app.rs` + `app/*`, `ui/*`.

**Crate dependency split (final state):**
- `spacetop-core`: `anyhow`, `serde`, `serde_yaml`, `notify`, `walkdir`, `sha1`, `thiserror`. dev: `tempfile`.
- `spacetop` (bin): `spacetop-core`, `anyhow`, `clap`, `crossterm`, `ratatui`, `ratskin`, `termimad`, `similar`, `sentry`. dev: `tempfile`, `sentry` (test feature).
  (Add any dependency the compiler reports as missing in a crate; remove any it reports unused via clippy.)

**Integration tests (in `tests/`) relocate by target:**
- `git_sync_e2e.rs`, `watcher_fs.rs`, `no_write_git_calls.rs` → `crates/spacetop-core/tests/`
- `discovery_bypass.rs`, `readme_reload.rs` → `crates/spacetop/tests/`
- `tests/fixtures/` → keep at workspace root; tests reference it via a computed workspace-root path (see Task 6).

---

## Task 1: Sever `domain → ratatui` with a core-owned `Rgb` type (in place)

**Why:** `src/domain/mod.rs:4` is `use ratatui::style::Color;` — the only terminal-crate import in any core-destined module. `assign_stage_colors`/`stage_color`/`stage_color_for` emit **only** `Color::Rgb(r,g,b)` (the existing tests at `src/domain/mod.rs:400-401` and `src/ui/tests/colors.rs:14` assert this). Replace the stored type with a core `Rgb { r, g, b }` and convert to `ratatui::style::Color` at the UI boundary. Still a single crate — verified by the full suite.

**Files:**
- Modify: `src/domain/mod.rs`
- Create: `src/ui/color.rs`
- Modify: `src/ui/mod.rs`, `src/ui/list.rs`, `src/ui/graph.rs`, `src/ui/preview.rs`, `src/ui/definition.rs`
- Test: `src/domain/mod.rs` (inline tests), `src/ui/tests/colors.rs`

- [ ] **Step 1: Add the `Rgb` type to domain (write the failing test first)**

In `src/domain/mod.rs`, inside the existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn rgb_is_plain_value_type() {
        let a = Rgb { r: 10, g: 20, b: 30 };
        let b = Rgb { r: 10, g: 20, b: 30 };
        assert_eq!(a, b);
        assert_eq!((a.r, a.g, a.b), (10, 20, 30));
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib domain::tests::rgb_is_plain_value_type`
Expected: FAIL — `cannot find type Rgb in this scope`.

- [ ] **Step 3: Define `Rgb` and remove the ratatui import**

In `src/domain/mod.rs`, delete line `use ratatui::style::Color;` and add near the top of the module:

```rust
/// A plain RGB color owned by the core (no terminal-crate dependency).
/// The UI layer converts this to `ratatui::style::Color` at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
```

Then replace every `Color` in this file with `Rgb`:
- `assign_stage_colors(...) -> HashMap<String, Rgb>`; the constructor `Color::Rgb(r, g, b)` → `Rgb { r, g, b }`.
- `stage_color(...) -> Rgb`; `Color::Rgb(r, g, b)` → `Rgb { r, g, b }`.
- `pub stage_colors: HashMap<String, Rgb>` and `stage_color_for(...) -> Rgb`.
- In the existing tests (`src/domain/mod.rs` ~lines 232-235 and ~400-401): `results.push(Color::Rgb(r, g, b))` → `results.push(Rgb { r, g, b })`; `HashSet<Color>` → `HashSet<Rgb>`. The `matches!(color, Color::Rgb(_, _, _))` assertion at ~line 400 becomes a tautology once the type is `Rgb` (it has no non-Rgb variant), so **delete that single assertion line** and keep the surrounding distinctness assertions (the `HashSet` length check) unchanged — those still carry the real guarantee.

- [ ] **Step 4: Create the UI conversion boundary**

Create `src/ui/color.rs`:

```rust
use crate::domain::Rgb;
use ratatui::style::Color;

/// Convert a core `Rgb` into a ratatui `Color` at the UI boundary.
pub(crate) fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}
```

In `src/ui/mod.rs` add `mod color;` (with the other `mod` lines) and update the two thin re-exports so they convert:

```rust
pub(crate) fn stage_color(stage_name: &str) -> ratatui::style::Color {
    color::to_color(crate::domain::stage_color(stage_name))
}

pub(crate) fn assign_stage_colors(
    stages: &[crate::domain::StageDefinition],
) -> std::collections::HashMap<String, ratatui::style::Color> {
    crate::domain::assign_stage_colors(stages)
        .into_iter()
        .map(|(k, v)| (k, color::to_color(v)))
        .collect()
}
```

- [ ] **Step 5: Convert at the remaining UI call sites**

Each site calling `definition.stage_color_for(name)` now gets an `Rgb`; wrap it. Update:
- `src/ui/list.rs:150` → `let stage_color = crate::ui::color::to_color(state.snapshot().definition.stage_color_for(&item.status));`
- `src/ui/graph.rs:549, 1099, 1413` → wrap each `definition.stage_color_for(...)` in `crate::ui::color::to_color(...)`.
- `src/ui/preview.rs:321` → wrap in `crate::ui::color::to_color(...)`.
- `src/ui/definition.rs:131, 148, 208` → wrap each `definition.stage_color_for(...)` (and `target`) in `crate::ui::color::to_color(...)`.

(`src/ui/definition.rs:358` calls `crate::domain::assign_stage_colors` directly to build a test `stage_colors` map of `Rgb` — that is fine; the field is now `HashMap<String, Rgb>`.)

- [ ] **Step 6: Reconcile the UI color test**

`src/ui/tests/colors.rs` calls `super::stage_color(...)` / `super::assign_stage_colors(...)`, which now return ratatui `Color` (converted). The `matches!(c, Color::Rgb(_,_,_))` assertions remain valid because `to_color` always produces `Color::Rgb`. Leave the assertions; only fix imports if the compiler complains.

- [ ] **Step 7: Run the full suite and lint**

Run: `cargo test`
Expected: PASS (all existing tests, plus the new `rgb_is_plain_value_type`).
Run: `make lint`
Expected: clean (no warnings).

- [ ] **Step 8: Verify the ratatui import is gone from domain**

Run: `grep -rn 'ratatui\|crossterm\|termimad\|ratskin' src/domain/`
Expected: no output.

- [ ] **Step 9: Commit**

```bash
git add src/domain/mod.rs src/ui/
git commit -m "refactor: core-owned Rgb type; convert to ratatui Color at UI boundary

Severs the only terminal-crate dependency in a core-destined module,
prerequisite for the spacetop-core split. Behavior identical.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Rename `WorkItem` → `Entity` (in place, compiler-driven)

**Why:** The v2 model is `Entity`. Doing the rename in the single crate first means the physical move in Task 3 carries no name changes. `WorkItem` appears ~61 times across 14 files (`src/app.rs`, `src/app/overview.rs`, `src/app/keys.rs`, `src/app/tests.rs`, `src/parser/item.rs`, `src/parser/worktree.rs`, `src/parser/archive.rs`, `src/domain/mod.rs`, `src/ui/preview.rs`, `src/ui/tests.rs`, `src/ui/tests/paths.rs`, `src/ui/tests/task_list.rs`, `src/ui/tests/worktree.rs`, `src/ui/graph/tests.rs`).

**Files:** all files containing `WorkItem` (listed above).

- [ ] **Step 1: Rename the struct definition**

In `src/domain/mod.rs`, rename `pub struct WorkItem {` → `pub struct Entity {`. Add a temporary doc line: `/// Renamed from WorkItem in v2 P0.` (optional, remove before commit if noise).

- [ ] **Step 2: Mechanical rename across the tree**

Replace the identifier `WorkItem` with `Entity` everywhere (whole-word). Do NOT rename `WorkflowDefinition`, `WorkflowSnapshot`, or `WorkItem`-adjacent field names — only the exact token `WorkItem`. Suggested command (review the diff afterward):

```bash
grep -rl 'WorkItem' src/ | xargs sed -i '' 's/\bWorkItem\b/Entity/g'
```

(On Linux `sed -i` without the `''` argument.)

- [ ] **Step 3: Check for collisions**

Run: `grep -rn '\bEntity\b' src/ | grep -i 'entitytype\|entity_type\|EntityParseError'`
Expected: `entity_type` / `EntityParseError` are pre-existing and unrelated — confirm the sed did not mangle them (it won't, they are different tokens). If `EntityParseError` is intact and there is no new `EntityEntity`, proceed.

- [ ] **Step 4: Build, test, lint**

Run: `cargo test`
Expected: PASS.
Run: `make lint`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "refactor: rename WorkItem -> Entity (v2 model name)

Mechanical rename in place; behavior identical.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Stand up the workspace and move modules into the two crates

**Why:** This is the physical restructure. It is atomic — the tree will not compile until both crates are in place and imports are rewritten — so green is verified at the END of this task. No semantic changes happen here (Tasks 1–2 already did them).

**Files:**
- Create: `Cargo.toml` (workspace root, replacing the package manifest), `crates/spacetop-core/Cargo.toml`, `crates/spacetop-core/src/lib.rs`, `crates/spacetop/Cargo.toml`.
- Move: all `src/*` per the file map above.

- [ ] **Step 1: Create crate directories and move core modules**

```bash
mkdir -p crates/spacetop-core/src crates/spacetop/src
git mv src/domain crates/spacetop-core/src/domain
git mv src/parser.rs crates/spacetop-core/src/parser.rs
git mv src/parser crates/spacetop-core/src/parser
git mv src/discovery.rs crates/spacetop-core/src/discovery.rs
git mv src/watcher.rs crates/spacetop-core/src/watcher.rs
git mv src/git_sync.rs crates/spacetop-core/src/git_sync.rs
git mv src/editor.rs crates/spacetop-core/src/editor.rs
```

- [ ] **Step 2: Move bin modules**

```bash
git mv src/main.rs crates/spacetop/src/main.rs
git mv src/lib.rs crates/spacetop/src/lib.rs
git mv src/cli.rs crates/spacetop/src/cli.rs
git mv src/app.rs crates/spacetop/src/app.rs
git mv src/app crates/spacetop/src/app
git mv src/ui crates/spacetop/src/ui
rmdir src 2>/dev/null || true
```

- [ ] **Step 3: Write the workspace root `Cargo.toml`**

Replace the root `Cargo.toml` contents with:

```toml
[workspace]
members = ["crates/spacetop-core", "crates/spacetop"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
```

- [ ] **Step 4: Write `crates/spacetop-core/Cargo.toml`**

```toml
[package]
name = "spacetop-core"
version.workspace = true
edition.workspace = true

[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
notify = "8"
walkdir = "2"
sha1 = "0.10"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 5: Write `crates/spacetop-core/src/lib.rs`**

```rust
pub mod discovery;
pub mod domain;
pub mod editor;
pub mod git_sync;
pub mod parser;
pub mod watcher;
```

- [ ] **Step 6: Write `crates/spacetop/Cargo.toml`**

```toml
[package]
name = "spacetop"
version.workspace = true
edition.workspace = true

[dependencies]
spacetop-core = { path = "../spacetop-core" }
anyhow = "1"
sentry = { version = "0.34", default-features = false, features = ["backtrace", "contexts", "panic", "reqwest", "rustls"] }
clap = { version = "4.5", features = ["derive"] }
crossterm = "0.28"
ratatui = "0.30"
ratskin = "0.3"
termimad = "0.34"
similar = "2"

[dev-dependencies]
sentry = { version = "0.34", default-features = false, features = ["test"] }
tempfile = "3"
```

- [ ] **Step 7: Fix `crates/spacetop/src/lib.rs` module declarations**

Remove the moved-out `pub mod` lines (`domain`, `discovery`, `editor`, `git_sync`, `parser`, `watcher`). Keep only the bin modules:

```rust
pub mod app;
pub mod cli;
pub mod ui;
```

(Leave the `run`/`decide_app`/`run_terminal`/OSC-7 functions and the inline `#[cfg(test)] mod tests` in this file.)

- [ ] **Step 8: Rewrite import paths in the bin crate**

In every file under `crates/spacetop/src/`, rewrite references to moved modules from `crate::` to `spacetop_core::`. The moved modules are: `domain`, `discovery`, `editor`, `git_sync`, `parser`, `watcher`.

```bash
cd crates/spacetop/src
grep -rl 'crate::\(domain\|discovery\|editor\|git_sync\|parser\|watcher\)' . \
  | xargs sed -i '' -E 's/\bcrate::(domain|discovery|editor|git_sync|parser|watcher)\b/spacetop_core::\1/g'
cd /Users/kent/Dev/InfuseAI/GitHub/spacetop
```

Also handle bare `use crate::{...}` groupings that mix bin and core modules — split them by hand if the sed leaves an invalid path (the compiler will point to them).

- [ ] **Step 9: First workspace build — iterate on errors**

Run: `cargo build --workspace 2>&1 | head -40`
Expected initial failures are import-path errors. Resolve each:
- A core module still says `use crate::ui`/`use crate::app` → a layering violation; report it (should not happen given the code map, but if it does, stop and surface it — do not paper over it).
- A bin file references `spacetop::domain::X` (via the crate name) → change to `spacetop_core::domain::X`.
- A missing dependency in a crate → add it to that crate's `Cargo.toml` (only from the allowed stacks above).
Repeat until `cargo build --workspace` succeeds.

- [ ] **Step 10: Run the unit tests (integration tests move in Task 4)**

Run: `cargo test --workspace --lib --bins`
Expected: PASS. (The inline `#[cfg(test)]` module tests moved with their files; core tests now run under `spacetop-core`, bin tests under `spacetop`.)

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "refactor: split into spacetop-core + spacetop workspace crates

Physical module move; import paths rewritten crate:: -> spacetop_core::.
No semantic changes. Unit/bin tests green.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Relocate integration tests to the correct crates

**Why:** Integration tests in `tests/` belong to whichever crate exposes the API they exercise. Cargo only compiles a crate's integration tests from that crate's own `tests/` dir.

**Files:**
- Move: `tests/git_sync_e2e.rs`, `tests/watcher_fs.rs` → `crates/spacetop-core/tests/`
- Move: `tests/discovery_bypass.rs`, `tests/readme_reload.rs` → `crates/spacetop/tests/`
- (`tests/no_write_git_calls.rs` is handled in Task 6.)
- Keep: `tests/fixtures/` at workspace root.

- [ ] **Step 1: Move the core-targeting tests**

```bash
mkdir -p crates/spacetop-core/tests crates/spacetop/tests
git mv tests/git_sync_e2e.rs crates/spacetop-core/tests/git_sync_e2e.rs
git mv tests/watcher_fs.rs   crates/spacetop-core/tests/watcher_fs.rs
git mv tests/discovery_bypass.rs crates/spacetop/tests/discovery_bypass.rs
git mv tests/readme_reload.rs    crates/spacetop/tests/readme_reload.rs
```

- [ ] **Step 2: Fix crate references in the moved tests**

Core tests (`git_sync_e2e.rs`, `watcher_fs.rs`) that referenced `spacetop::git_sync` / `spacetop::watcher` must now use `spacetop_core::git_sync` / `spacetop_core::watcher`:

```bash
cd crates/spacetop-core/tests
sed -i '' -E 's/\bspacetop::(git_sync|watcher|domain|parser|discovery|editor)\b/spacetop_core::\1/g' *.rs
cd /Users/kent/Dev/InfuseAI/GitHub/spacetop
```

Bin tests (`discovery_bypass.rs`, `readme_reload.rs`) keep `spacetop::` for bin API (`decide_app`, etc.) but switch any moved-module reference (`spacetop::domain`, `spacetop::parser`, …) to `spacetop_core::`:

```bash
cd crates/spacetop/tests
sed -i '' -E 's/\bspacetop::(domain|parser|discovery|editor|git_sync|watcher)\b/spacetop_core::\1/g' *.rs
cd /Users/kent/Dev/InfuseAI/GitHub/spacetop
```

- [ ] **Step 3: Fix the fixtures path in moved tests**

Any test that locates fixtures via `CARGO_MANIFEST_DIR/tests/fixtures` now sits one level deeper (`crates/<crate>/`). Update those lookups to resolve the workspace root. Replace the manifest-relative fixtures path with:

```rust
// Workspace root is two levels up from a crate's CARGO_MANIFEST_DIR.
let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../tests/fixtures");
```

Apply to each moved test that references `tests/fixtures` (search: `grep -rn 'fixtures' crates/*/tests`). If a test already used a relative `tests/fixtures` literal, change it to the computed path above.

- [ ] **Step 4: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS. (Pre-existing known-failing fixture tests, if any, must fail no differently than before P0 — confirm parity, do not "fix" unrelated failures in this phase.)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test: relocate integration tests to owning crates; fix fixtures path

git_sync_e2e/watcher_fs -> spacetop-core; discovery_bypass/readme_reload ->
spacetop bin. Fixtures resolved from workspace root.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Verify the build/lint/run toolchain works against the workspace

**Why:** `make lint`, `make build`, and `make install` must still work. From a virtual-manifest root, `cargo clippy --all-targets --all-features` and `cargo build --release` already span all members, and the bin artifact stays at `target/release/spacetop`. Confirm and adjust only if needed.

**Files:** `Makefile` (only if a command proves insufficient).

- [ ] **Step 1: Lint the whole workspace**

Run: `make lint`
Expected: clean across both crates. If clippy reports an unused dependency in a crate, remove it from that crate's `Cargo.toml`; if it reports a missing one, the build would already have failed in Task 3.

- [ ] **Step 2: Release build**

Run: `make build`
Expected: builds; `target/release/spacetop` exists.
Run: `test -x target/release/spacetop && echo OK`
Expected: `OK`.

- [ ] **Step 3: Smoke-run against the real workflow**

Run: `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev` (quit immediately with `q`).
Expected: the TUI launches and renders the workflow exactly as before P0.

- [ ] **Step 4: Adjust the Makefile only if a step failed**

If `make build`/`make lint` needed a `--workspace` flag or an explicit `-p spacetop` for the release build, add it. Otherwise leave the Makefile unchanged. If changed, re-run Steps 1–2.

- [ ] **Step 5: Commit (only if the Makefile changed)**

```bash
git add Makefile
git commit -m "build: adjust Makefile for workspace layout"
```

(Skip this commit if no file changed.)

---

## Task 6: Guard test — `spacetop-core` links no terminal crate

**Why:** The load-bearing rule of the split. A test asserts that `spacetop-core`'s resolved dependency tree contains none of `ratatui`, `crossterm`, `termimad`, `ratskin` — catching even transitive regressions. Also relocate and generalize the read-only `no_write_git_calls` guardrail so it scans **both** crate src trees.

**Files:**
- Create: `crates/spacetop-core/tests/no_terminal_deps.rs`
- Move + modify: `tests/no_write_git_calls.rs` → `crates/spacetop-core/tests/no_write_git_calls.rs`

- [ ] **Step 1: Write the no-terminal-deps guard test**

Create `crates/spacetop-core/tests/no_terminal_deps.rs`:

```rust
//! Guard: spacetop-core must never link a terminal-UI crate. The headless/export
//! surface depends on this boundary. Uses `cargo tree` so transitive deps count.

use std::process::Command;

const FORBIDDEN: [&str; 4] = ["ratatui", "crossterm", "termimad", "ratskin"];

#[test]
fn core_dependency_tree_has_no_terminal_crates() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "spacetop-core",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run `cargo tree`");

    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tree = String::from_utf8_lossy(&output.stdout);
    let offenders: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|crate_name| {
            tree.lines().any(|line| {
                // `--prefix none` lines look like "ratatui v0.30.0".
                line.split_whitespace().next() == Some(crate_name)
            })
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "spacetop-core must not depend on terminal crates, found: {offenders:?}"
    );
}
```

- [ ] **Step 2: Run it — verify it passes for the clean core**

Run: `cargo test -p spacetop-core --test no_terminal_deps`
Expected: PASS.

- [ ] **Step 3: Sanity-check the guard actually catches violations**

Temporarily add `ratatui = "0.30"` to `crates/spacetop-core/Cargo.toml` `[dependencies]`, then run the test again.
Run: `cargo test -p spacetop-core --test no_terminal_deps`
Expected: FAIL with `found: ["ratatui"]`. Then **remove** the temporary line and re-run — expect PASS. (Do not commit the temporary line.)

- [ ] **Step 4: Relocate and generalize the read-only guardrail**

```bash
git mv tests/no_write_git_calls.rs crates/spacetop-core/tests/no_write_git_calls.rs
rmdir tests 2>/dev/null || true   # only if tests/ is now empty except fixtures
```

(`tests/fixtures/` stays at the workspace root, so `tests/` will not be empty — that is fine; do not delete it.)

Edit `crates/spacetop-core/tests/no_write_git_calls.rs` so it scans both crates. Replace the `src_root()` helper with a function returning both src roots resolved from the workspace root, and update the two test bodies to iterate over both:

```rust
fn src_roots() -> Vec<PathBuf> {
    // CARGO_MANIFEST_DIR is crates/spacetop-core; workspace root is two up.
    let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    vec![
        ws.join("crates/spacetop-core/src"),
        ws.join("crates/spacetop/src"),
    ]
}

fn all_rust_files() -> Vec<PathBuf> {
    src_roots().iter().flat_map(|r| rust_files(r)).collect()
}
```

In `src_tree_does_not_reference_disallowed_git_write_subcommands`, replace `rust_files(&src_root())` with `all_rust_files()`. In `src_tree_references_ff_only_exactly_once`, replace `rust_files(&src_root())` with `all_rust_files()` — `--ff-only` still appears exactly once total (it lives only in `spacetop-core/src/git_sync.rs`).

- [ ] **Step 5: Run both guardrails**

Run: `cargo test -p spacetop-core --test no_write_git_calls --test no_terminal_deps`
Expected: PASS — including `--ff-only` found exactly once across both crate srcs.

- [ ] **Step 6: Full workspace test + lint**

Run: `cargo test --workspace`
Expected: PASS.
Run: `make lint`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "test: guard core has no terminal deps; scan both crates for git writes

cargo-tree guard test enforces the spacetop-core terminal-free boundary;
no_write_git_calls relocated to core and now scans both crate src trees.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Update docs to match the new structure

**Why:** Policy requires docs to track architecture changes in the same effort (`AGENTS.md` "No stale project facts"; `development-policy.md` Documentation Policy). The Code Map and architecture-boundary sections name single-crate paths that are now two-crate.

**Files:** `AGENTS.md`, `docs/development-policy.md`, `CLAUDE.md`, `README.md`.

- [ ] **Step 1: Update `AGENTS.md` Code Map**

In the "Code Map" section, reflect the workspace: note that pure-logic modules (`domain`, `parser`, `discovery`, `watcher`, `git_sync`, `editor`) live in `crates/spacetop-core/src/` and TUI/CLI modules (`cli`, `lib`, `app`, `ui`) live in `crates/spacetop/src/`. Update each bullet's path prefix accordingly. Add a line: "`spacetop-core` must not depend on terminal crates; enforced by `crates/spacetop-core/tests/no_terminal_deps.rs`." Note the `WorkItem`→`Entity` rename in the domain bullet.

- [ ] **Step 2: Update `docs/development-policy.md` Architecture Boundaries**

Under "Current single-crate boundaries", retitle to the two-crate layout and update the module paths to their `crates/<crate>/src/...` locations. Keep the "Strategic v2 boundary" subsection (it already describes the core/no-terminal rule).

- [ ] **Step 3: Update `CLAUDE.md` Module Layout + commands**

In the "Module Layout" section, update paths to the workspace layout. In "Commands", note that `cargo run` becomes `cargo run -p spacetop` (or keep `cargo run` — confirm it still resolves the default bin; if the workspace has a single bin, `cargo run` works, otherwise use `-p spacetop`). Update the `no_write_git_calls.rs` path reference in the Safety section to `crates/spacetop-core/tests/no_write_git_calls.rs`.

- [ ] **Step 4: Update `README.md` if it names module paths or `WorkItem`**

Run: `grep -n 'src/\|WorkItem\|single.*crate' README.md`
Update any stale path or the old model name. If README has no such references, skip.

- [ ] **Step 5: Verify docs are consistent and commit**

Run: `grep -rn 'src/domain\|src/parser\|src/git_sync\|WorkItem' AGENTS.md docs/development-policy.md CLAUDE.md README.md`
Expected: no stale single-crate `src/...` module paths or `WorkItem` references remain (matches are acceptable only where they intentionally describe history).

```bash
git add AGENTS.md docs/development-policy.md CLAUDE.md README.md
git commit -m "docs: update code map and boundaries for the two-crate workspace

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Definition of done (P0)

- [ ] `cargo build --workspace` and `cargo build --release` succeed; bin at `target/release/spacetop`.
- [ ] `cargo test --workspace` passes (pre-existing known fixture failures, if any, are unchanged — no new failures).
- [ ] `make lint` is clean across both crates.
- [ ] `crates/spacetop-core/tests/no_terminal_deps.rs` passes and was demonstrated to fail when a terminal crate is added.
- [ ] `crates/spacetop-core/tests/no_write_git_calls.rs` passes scanning both crate src trees; `--ff-only` found exactly once.
- [ ] `spacetop-core` contains only `domain`, `parser`, `discovery`, `watcher`, `git_sync`, `editor`; `spacetop` contains `cli`, `lib`, `app`, `ui`, `main`.
- [ ] The model type is `Entity` everywhere; no `WorkItem` token remains in code.
- [ ] TUI behavior is identical to pre-P0 (verified by the smoke run + the unchanged render/test suite).
- [ ] `AGENTS.md`, `docs/development-policy.md`, `CLAUDE.md`, `README.md` describe the two-crate layout.
- [ ] No async runtime added; read-only contract intact.

## Notes for the executor

- **Atomicity of Task 3.** Tasks 1–2 are independently green. Task 3 is the only step where the tree is temporarily broken mid-task; do not commit until `cargo build --workspace` + unit tests are green.
- **macOS `sed`.** Commands use `sed -i ''` (BSD). On Linux use `sed -i` (no `''`). Review every `sed` diff before committing — a mechanical replace can over-match.
- **Layering violations.** If a core module turns out to reference `crate::app`/`crate::ui`, that is a real coupling the code map did not predict. Stop and surface it with the specific file/line rather than working around it — it may need a tiny interface extraction, which is a scoped follow-up, not a silent hack.
- **Do not fix pre-existing failures.** Two fixture tests were known-failing before P0 (archive fixture drift). Preserve parity; fixing them is out of scope for a behavior-identical restructure.
