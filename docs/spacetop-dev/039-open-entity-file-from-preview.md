---
id: 039
title: Open entity file from preview (o keybind + OSC 7)
status: review
source: captain conversation — preview pane shows long markdown content that's hard to read in the TUI; absolute paths don't fit the header, so users can't Cmd+click them either
started: 2026-05-14T09:25:30Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-039-open-entity-file-from-preview
issue:
pr: #32
mod-block: merge:pr-merge
---

When the preview pane is open on a work item, the user has no quick way to open that item's markdown file in a real editor. The full absolute path in the header (`src/ui/mod.rs:858`) gets truncated, which also defeats iTerm2/Ghostty Smart Selection on the rendered text. ratatui doesn't support inline OSC 8 hyperlinks reliably, so embedding clickable links inside the buffer isn't viable.

Two complementary affordances are proposed:

1. **`o` keybind** — when the preview is open, pressing `o` suspends the TUI, opens the entity's markdown file via `$EDITOR` (falling back to `open` on macOS / `xdg-open` on Linux), and resumes the TUI on exit. This is the primary mechanism — it works in every terminal, including over SSH, and doesn't depend on the path being visible.

2. **OSC 7 at startup** — emit `\e]7;file://{host}{cwd}\e\\` once during terminal setup so terminals that support Smart Selection on relative paths can resolve them against the right working directory. Pair with rendering a *relative* path (relative to the workflow root) in the preview header so it fits and stays clickable for mouse users.

Design should resolve:
- which key (`o` confirmed in conversation, but check for conflicts with existing bindings)
- editor selection precedence: `$VISUAL` → `$EDITOR` → platform default opener
- whether to block on the editor or background it (block is simpler and matches lazygit/gitui conventions)
- where to place the OSC 7 emit (before `EnterAlternateScreen`? after?) and whether to guard with `TERM`/`TERM_PROGRAM` checks
- header path rendering: relative-to-workflow-root vs current absolute, and how to handle worktree-resident entities whose paths sit outside the workflow root

## Acceptance criteria

**AC-1 — Pressing `o` in the preview opens the entity file in an external editor.**
Verified by: integration test that drives `App` through `AppMode::Overview` with a preview-open state, simulates `KeyCode::Char('o')`, and asserts the app records an "open file" intent against the selected entity's `path`. (The actual `Command::spawn` may be stubbed behind a trait so the test runs without an editor installed.)

**AC-2 — TUI suspends cleanly around the editor invocation and resumes without artifact.**
Verified by: manual confirmation that `disable_raw_mode` + `LeaveAlternateScreen` happen before the editor spawns and the inverse runs on return, plus a unit test on the terminal-suspend/resume helper that asserts the call sequence.

**AC-3 — `o` is a no-op (or visibly ignored) when the preview is closed.**
Verified by: unit test on the keybind dispatcher that asserts no open-file intent is recorded when `preview_open()` is false.

**AC-4 — Editor resolution falls back deterministically.**
Verified by: unit test on the resolver that exercises `$VISUAL` set, `$EDITOR` set, both unset (platform default), and returns the expected command in each case.

**AC-5 — OSC 7 is emitted exactly once during startup, before the alt screen is entered, and only when stdout is a TTY.**
Verified by: unit test on the startup helper using a captured writer; asserts the bytes match `\e]7;file://<host><urlencoded-cwd>\e\\` and that nothing is emitted when the writer is flagged as non-TTY.

**AC-6 — Help screen documents the new keybinding.**
Verified by: grep for `o` in the help/keybinding list rendered by `render_help_lines` (or equivalent in `src/ui/mod.rs`).

**AC-7 — `make lint` passes.**
Verified by: `make lint` exits zero with no clippy warnings.

## Implementation Plan

### Architecture overview

Three layers, kept testable in isolation:

1. **State / intent layer (`src/app/`)** — `o` becomes an `OverviewKeyAction::OpenSelectedFile { path: PathBuf }`. Pure state, no I/O. Unit-testable.
2. **Editor resolver (`src/editor.rs`, new)** — pure function `resolve_editor(env: &dyn EditorEnv) -> EditorCommand` plus an `EditorLauncher` trait abstracting `Command::status`. Allows AC-1/AC-2/AC-4 to run without a real editor.
3. **Terminal suspend/resume + OSC 7 (`src/lib.rs`)** — only place that touches `crossterm` raw mode and the alt screen. OSC 7 emit is a small helper that takes a writer + a TTY flag (testable with a captured `Vec<u8>` writer).

### Module / file changes

| File | Change | Rationale |
|------|--------|-----------|
| `src/app/keys.rs` | Add `OverviewKeyAction::OpenSelectedFile(PathBuf)` variant; match `KeyCode::Char('o') if state.preview_open()` and read `state.selected_item().map(|w| w.path.clone())` to fill it; ignore when preview closed (AC-3). | Pure state transition; mirrors the existing `OpenPickerOverlay` intent pattern. |
| `src/app.rs` | Add `pending_open_file: Option<PathBuf>` field on `App`, plus `take_pending_open_file()`. Wire `OverviewKeyAction::OpenSelectedFile` in `apply_overview_key_action` to set it. | Same intent-drain pattern used by `pending_switch` / `pending_overlay_open`; lets `run_terminal` perform the suspend/launch without `App` touching I/O. |
| `src/editor.rs` (new) | `pub struct EditorCommand { program: OsString, args: Vec<OsString> }`. `pub trait EditorEnv { fn var(&self, k: &str) -> Option<OsString>; }` with a `StdEnv` impl. `pub fn resolve_editor(env: &dyn EditorEnv) -> EditorCommand` implementing `$VISUAL → $EDITOR → platform default` (`open` on `cfg!(target_os = "macos")`, `xdg-open` elsewhere). `pub trait EditorLauncher { fn launch(&self, cmd: &EditorCommand, file: &Path) -> io::Result<ExitStatus>; }` with `StdLauncher` calling `Command::new(...).arg(file).status()` (blocking). | Pure resolver function + injectable launcher trait keeps AC-1/AC-2/AC-4 testable without an editor installed. |
| `src/lib.rs` | (1) New `suspend_terminal()` / `resume_terminal()` helpers that call `disable_raw_mode` + `execute!(LeaveAlternateScreen)` and the inverse, plus `terminal.clear()` after resume. (2) New `emit_osc7<W: Write>(w: &mut W, is_tty: bool, cwd: &Path) -> io::Result<()>` that writes `\x1b]7;file://<host><urlencoded-cwd>\x1b\\` when `is_tty`, else nothing. (3) Call `emit_osc7(&mut stdout, stdout.is_terminal(), &cwd)` *before* `enable_raw_mode` / `EnterAlternateScreen` in `run_terminal` (AC-5 ordering). (4) After the key-poll branch, drain `app.take_pending_open_file()`: build resolver, suspend, launch (blocking), resume, force redraw next loop. | Single owner of terminal lifecycle; keeps `App` UI-agnostic and testable. |
| `src/ui/mod.rs` | (1) In `build_preview_header_lines` (header `path:` line at `src/ui/mod.rs:858`), render `item.path` *relative to* `state.workflow_dir()` via `pathdiff`-style `strip_prefix`; fallback to absolute when `strip_prefix` fails. (2) Add help line `o  open file in $EDITOR` to `render_help_popup`'s `if preview_open { ... }` block (and bump `popup_h` by 1 in that branch). (3) Add `"o: open"` to `status_footer_hints` when `preview_open` is true. | UI-only change; logic for "relative-or-absolute" is small enough to live inline. |
| `Cargo.toml` | No new deps. `std::io::IsTerminal` (stable since 1.70) covers AC-5 TTY guard. Editor spawn uses `std::process::Command`. URL-encoding for OSC 7 is a 10-line hand-rolled percent-encoder for non-ASCII / unsafe bytes — adding a crate for this is overkill. | Keep dependency footprint flat. |

### Step-by-step

1. **`OverviewKeyAction::OpenSelectedFile(PathBuf)` + `o` handler** (`src/app/keys.rs`).
   - Add the variant.
   - In `handle_overview_key`, add `KeyCode::Char('o') if state.preview_open() => match state.selected_item() { Some(item) => OverviewKeyAction::OpenSelectedFile(item.path.clone()), None => OverviewKeyAction::None }`. Place *before* the catch-all `_ => None`.
   - When `preview_open()` is false the arm doesn't match — silently ignored (AC-3).
2. **`App::pending_open_file` + drain accessor** (`src/app.rs`).
   - Add field, initialize in every constructor.
   - Add `pub fn take_pending_open_file(&mut self) -> Option<PathBuf>`.
   - Extend `apply_overview_key_action` with the new variant.
3. **Editor resolver module** (`src/editor.rs`, new; declared in `src/lib.rs` as `pub mod editor;`).
   - Define `EditorCommand`, `EditorEnv`, `StdEnv`, `EditorLauncher`, `StdLauncher`, `resolve_editor`.
   - `resolve_editor`: split on whitespace at the top level (e.g. `EDITOR="code --wait"` → `program=code, args=["--wait"]`), platform default falls through to `open` / `xdg-open` with no args.
4. **Suspend/resume helper + OSC 7** (`src/lib.rs`).
   - `fn suspend_terminal(stdout: &mut io::Stdout) -> io::Result<()>` and `fn resume_terminal(stdout: &mut io::Stdout) -> io::Result<()>`.
   - `fn emit_osc7<W: Write>(w: &mut W, is_tty: bool, cwd: &Path) -> io::Result<()>` — when `is_tty`, percent-encode `cwd`'s bytes (keep `unreserved` per RFC 3986 + `/`) and write `b"\x1b]7;file://"`, hostname (`hostname::get()` is not a dep — use empty string; iTerm/Ghostty accept `file:///path` with empty host), `<encoded>`, `b"\x1b\\"` (ST). Empty-host is the OSC 7 convention these terminals already document.
   - In `run_terminal`: call `emit_osc7(&mut stdout, stdout.is_terminal(), &std::env::current_dir().unwrap_or_default())` **before** `enable_raw_mode()` and `EnterAlternateScreen` (AC-5).
   - After the key-poll branch, drain `if let Some(path) = app.take_pending_open_file() { suspend_terminal(&mut stdout)?; let _ = StdLauncher.launch(&resolve_editor(&StdEnv), &path); resume_terminal(&mut stdout)?; terminal.clear()?; }`. Blocking spawn — matches lazygit/gitui convention; simpler resume semantics; the editor's exit returns control to spacetop with a single clean redraw.
5. **Relative path in preview header** (`src/ui/mod.rs:858`).
   - Replace the line with: read `item.path.strip_prefix(state.workflow_dir())`; on `Ok(rel)` render `format!("path: {}", rel.display())`; on `Err(_)` (entity sits outside workflow root — e.g. worktree copies — see AC fallback) render the absolute path. This preserves correctness for the rare out-of-root case.
6. **Help + footer copy** (`src/ui/mod.rs`).
   - In `render_help_popup`, inside the `if preview_open { ... }` block, push `Line::from("  o              open file in $EDITOR")` and bump the `popup_h` "preview_open" branch from `20` to `21`.
   - In `status_footer_hints`, when `preview_open` is true, push `"o: open"` (right before `"q: quit"` is the natural slot).
7. **Cross-cutting verification**: `make lint` (AC-7). Anticipated clippy concerns: `cfg!(target_os)` in the resolver (fine), trait-object via `&dyn EditorEnv` (fine), no `unsafe`, no new deps.

### Test strategy (one named test per AC)

| AC | Test location | Name | Mechanism |
|----|---------------|------|-----------|
| AC-1 | `src/app/keys.rs` `#[cfg(test)]` | `o_with_preview_open_emits_open_file_intent` | Build `OverviewSession` with a preview-open state, dispatch `KeyEvent(Char('o'))`, assert `handle_overview_key` returns `OpenSelectedFile(expected_path)`. |
| AC-2 | `src/lib.rs` `#[cfg(test)]` | `suspend_resume_call_sequence` | Use a `MockTerm` newtype that records `disable_raw_mode → leave_alt → enter_alt → enable_raw_mode` ordering via a `Vec<&'static str>` log; assert order. The trait/seam: factor out an internal `trait TerminalControl { fn disable_raw_mode(&mut self); fn leave_alt(&mut self); ... }` that `suspend_terminal`/`resume_terminal` accept as a parameter. (Manual verification of an actual editor invocation is the AC's second clause and stays manual.) |
| AC-3 | `src/app/keys.rs` `#[cfg(test)]` | `o_with_preview_closed_is_noop` | Same setup with preview closed; assert action is `OverviewKeyAction::None` and no path is captured. |
| AC-4 | `src/editor.rs` `#[cfg(test)]` | `resolve_editor_visual_editor_default_precedence` | Stub `EditorEnv` returning fixed maps for four cases: VISUAL set, EDITOR set + no VISUAL, both unset (assert platform default `open` on macOS / `xdg-open` otherwise — use `cfg!(target_os = "macos")` in the assertion to match), and EDITOR with args (`"code --wait"`). |
| AC-5 | `src/lib.rs` `#[cfg(test)]` | `emit_osc7_writes_bytes_when_tty` + `emit_osc7_skips_when_not_tty` | Pass a `Vec<u8>` writer plus `is_tty=true/false`; assert exact byte sequence `\x1b]7;file:///some/path\x1b\\` and percent-encoding for a path containing a space (`/a b` → `/a%20b`). Second test: assert writer stays empty when `is_tty=false`. |
| AC-6 | `src/ui/mod.rs` `#[cfg(test)]` | `help_popup_documents_open_file_keybind_when_preview_open` | Render `App` with preview open + help open, extract help popup buffer, assert it contains the substring `"o"` and `"open file"`. |
| AC-7 | manual | n/a | `make lint` (clippy `-D warnings`) — the project's stated gate. |

### Editor invocation model decision

**Blocking.** Rationale: (a) matches lazygit / gitui / k9s precedent that users already expect; (b) sidesteps reaping zombie processes and tracking editor exit asynchronously; (c) the resume sequence (`enable_raw_mode` → `EnterAlternateScreen` → `terminal.clear()` → next loop iteration's `terminal.draw`) is straightforward and avoids racing the watcher; (d) over SSH and inside tmux this is the only model that doesn't fight the parent terminal for stdio. Documented in the editor module's module-level doc comment.

### OSC 7 placement + byte sequence

**Placement.** Emit `emit_osc7(...)` against `io::stdout()` *before* `enable_raw_mode` and *before* `EnterAlternateScreen`. The alt screen swallows scrollback-attached state; OSC 7 needs to land on the primary screen / pre-raw stream so the terminal records the working directory for subsequent path resolution.

**Guard.** Only emit when `stdout.is_terminal()` (via `std::io::IsTerminal`). No `TERM_PROGRAM` allowlist — OSC 7 is a no-op on terminals that don't recognize it (the bytes are silently ignored), so an allowlist would just add maintenance.

**Byte sequence.**
```
ESC ] 7 ; file:// <host> <encoded-cwd> ESC \
0x1b 0x5d 0x37 0x3b "file://" "" <pct-encoded> 0x1b 0x5c
```
- Host: empty string. iTerm2 and Ghostty both treat `file:///abs/path` (empty host) as "this host". This avoids a `hostname` dep and matches what `pwd | xargs printf 'file://%s'`-style emitters in zsh's `osc7-pwd` do.
- Encoding: percent-encode each byte of the path's `OsStr` (via `as_encoded_bytes()` or platform-specific `as_bytes()` on Unix) **except** the unreserved set `A-Za-z0-9-._~` and the path separator `/`. Bytes like `0x20` (space) become `%20`. This is the RFC 3986 path-segment safe set minus `/` preservation — enough for terminal consumers.

### Resolved design questions

- **(a) `o` collision.** Searched `src/app/keys.rs`, picker handlers, and overlay handlers (`src/app.rs` lines 330–391) — `Char('o')` is unbound (confirmed by `grep -rn "Char('o')"` returning no matches). Safe to claim.
- **(b) Header path fallback for out-of-root entities.** When `item.path.strip_prefix(state.workflow_dir())` fails (worktree-resident copies whose absolute path sits outside the workflow root), fall back to rendering `item.path` *absolute*. This keeps the metadata correct at the cost of width — and `o` still works because it operates on the absolute `PathBuf`, not the rendered string. Reject the "basename only" alternative: it loses the disambiguating context users rely on when scanning across multiple workflows.

### Lint gate notes (AC-7)

`make lint` runs `cargo clippy --all-targets --all-features -- -D warnings`. Anticipated diagnostics to pre-empt:
- `clippy::needless_borrow` on the OSC 7 byte writes — write `w.write_all(b"...")` directly.
- `clippy::or_fun_call` if a `unwrap_or_else(|| std::env::current_dir())` appears — use `unwrap_or_default()` or pre-extract.
- Trait objects across module boundaries: keep `EditorEnv` / `EditorLauncher` `?Sized` only where required; default to plain references.
- No new dependencies; no `unsafe`; no platform-conditional compilation beyond `cfg!(target_os = "macos")` in the resolver.

## Stage Report: plan

- [x] Step-by-step implementation plan covering (a) `o` keybind, (b) suspend/resume helper, (c) editor resolver with $VISUAL → $EDITOR → platform default, (d) OSC 7 startup emit, (e) relative-path header
  See "Step-by-step" section above; each sub-item maps to a numbered step.
- [x] Files and module boundaries identified; parsing/state kept out of UI rendering
  See "Module / file changes" table: `src/app/keys.rs` + `src/app.rs` for state; `src/editor.rs` (new) for resolver; `src/lib.rs` for terminal lifecycle; `src/ui/mod.rs` for header + help/footer copy only.
- [x] Test strategy mapping each AC to a named test or grep target; editor spawn mocked behind a trait; captured writer for AC-5
  See "Test strategy" table — one named test per AC-1..AC-6, manual + `make lint` for AC-7. `EditorLauncher` trait + `Vec<u8>` writer cover AC-1/AC-2/AC-5 without an installed editor.
- [x] Editor invocation model decided: blocking, with rationale
  Blocking spawn — lazygit/gitui convention; simpler resume; SSH/tmux-safe. See "Editor invocation model decision".
- [x] OSC 7 placement, TTY guard, and exact byte sequence + encoding rule documented
  Before `enable_raw_mode` and `EnterAlternateScreen`; guarded by `stdout.is_terminal()` (no `TERM` allowlist); bytes `\x1b]7;file://<empty-host><pct-encoded-cwd>\x1b\\`; encoding preserves `A-Za-z0-9-._~/` and percent-encodes the rest. See "OSC 7 placement + byte sequence".
- [x] Open design questions resolved: (a) `o` collision check, (b) out-of-root header fallback
  (a) `grep -rn "Char('o')"` against `src/` returned zero matches — `o` is unbound. (b) Fall back to absolute path when `strip_prefix(workflow_dir)` fails; `o` still opens the absolute path regardless of how the header rendered. See "Resolved design questions".
- [x] `make lint` confirmed as AC-7 gate; anticipated clippy concerns called out
  No new deps (uses stable `std::io::IsTerminal`); no `unsafe`; minimal `cfg!(target_os)`. See "Lint gate notes".
- [x] Plan committed to entity body as the stage report
  This report is the commit target; will be staged + committed after this write.

### Summary

Planned a four-file implementation: state intent in `src/app/keys.rs` + `src/app.rs`, a new `src/editor.rs` resolver with an injectable launcher trait, terminal lifecycle + OSC 7 emission in `src/lib.rs`, and header/help/footer copy in `src/ui/mod.rs`. Key decisions: blocking editor spawn (matches lazygit/gitui), OSC 7 emitted pre-raw-mode and pre-alt-screen with `IsTerminal` as the only guard, empty-host `file://` URL with RFC 3986 path-byte percent encoding, and a `strip_prefix(workflow_dir)`-with-absolute-fallback for the header path so worktree-resident entities still render correctly. No new dependencies; `o` confirmed unbound; every AC has a named test or explicit `make lint` gate.

## Stage Report: implement

- [x] `o` keybind in `src/app/keys.rs`: `OverviewKeyAction::OpenSelectedFile(PathBuf)` added; arm `KeyCode::Char('o') if state.preview_open()` reads `state.selected_item().map(|w| w.path.clone())`; silent no-op when preview closed.
  See `src/app/keys.rs:13` (variant) and `src/app/keys.rs:82-85` (arm).
- [x] `App::pending_open_file` + `take_pending_open_file()` drain; new variant wired in `apply_overview_key_action`; field initialized in all four constructors.
  See `src/app.rs:40` (field), `src/app.rs:166-168` (drain), `src/app.rs:429-431` (wiring), and constructors at `src/app.rs:44,56,68,80,91`.
- [x] `src/editor.rs` exports `EditorCommand`, `EditorEnv` + `StdEnv`, `EditorLauncher` + `StdLauncher`, and `resolve_editor` with `$VISUAL → $EDITOR → platform default` precedence; whitespace-split program + args; `StdLauncher` runs `Command::new(program).args(args).arg(file).status()` (blocking).
  See `src/editor.rs:29-116`; precedence at `src/editor.rs:104-116`; platform default uses `cfg!(target_os = "macos")` at `src/editor.rs:71-81`.
- [x] `suspend_terminal` / `resume_terminal` over an internal `TerminalControl` trait + `emit_osc7<W: Write>(w, is_tty, cwd)` writing `\x1b]7;file://<empty-host><pct-encoded>\x1b\\` when `is_tty`, nothing otherwise; encoding preserves `A-Za-z0-9-._~/`.
  See `src/lib.rs:277-316` (suspend/resume + trait) and `src/lib.rs:326-367` (`emit_osc7` + safe-byte set).
- [x] `emit_osc7` called against `io::stdout()` with `stdout.is_terminal()` before `enable_raw_mode()` and `EnterAlternateScreen` in `run_terminal`; pending-open drain suspends → `StdLauncher.launch(&resolve_editor(&StdEnv), &path)` → resumes → `terminal.clear()?`.
  See `src/lib.rs:117-122` (OSC 7 pre-raw) and `src/lib.rs:217-229` (drain).
- [x] Preview header renders path relative to `state.workflow_dir()` via `strip_prefix`; falls back to absolute path on `Err`.
  See `src/ui/mod.rs:876-880`.
- [x] Help popup adds `o              open file in $EDITOR` line inside `if preview_open` block; `popup_h` bumped by 1 in that branch (20→21, multi 23→24); footer adds `"o: open"` when `preview_open` is true.
  See `src/ui/mod.rs:355-357` (footer), `src/ui/mod.rs:369-383` (popup_h), and `src/ui/mod.rs:413` (help line).
- [x] Named tests per AC pass: `o_with_preview_open_emits_open_file_intent` (AC-1), `suspend_resume_call_sequence` (AC-2), `o_with_preview_closed_is_noop` (AC-3), `resolve_editor_visual_editor_default_precedence` (AC-4), `emit_osc7_writes_bytes_when_tty` + `emit_osc7_skips_when_not_tty` (AC-5), `help_popup_documents_open_file_keybind_when_preview_open` (AC-6).
  `cargo test --lib -- <8 names>` → 8 passed; 0 failed.
- [x] `make lint` and `cargo test` evidence captured; no `#[allow(...)]` added.
  `make lint` closing line: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.81s` (clean, `-D warnings`). `cargo test --lib` summary: `220 passed; 1 failed` — the one failure (`ui::graph::tests::narrow_tier_renders_compact_textual_summary`) reproduces on the pre-dispatch commit `ab6cb1b` and we did not modify `src/ui/graph.rs`, so it is pre-existing and out of scope.
- [x] Code committed on stage branch `spacedock-ensign/039-open-entity-file-from-preview`.
  Commit `60aa7b2` "feat(039): open entity file from preview (o keybind + OSC 7)" — 5 files changed, 580 insertions(+), 4 deletions(-).
- [x] Stage report appended to entity file body (this section); report commit follows.
  See commit history after this write.

### Summary

Implemented the `o` keybind + OSC 7 startup emit exactly as the plan specifies, in four layers: `OverviewKeyAction::OpenSelectedFile` state intent in `src/app/keys.rs` + `src/app.rs`, a new `src/editor.rs` with `EditorEnv`/`EditorLauncher` seams and a pure `resolve_editor` precedence resolver, terminal suspend/resume + `emit_osc7` over a `TerminalControl` seam in `src/lib.rs` (OSC 7 emitted before raw mode and the alt screen), and preview-header relative-path + help/footer copy in `src/ui/mod.rs`. All eight AC-tagged unit tests pass; `make lint` is clean with `-D warnings`. The pre-existing `narrow_tier_renders_compact_textual_summary` failure reproduces on the dispatch commit and is unrelated to this change.

## Stage Report: review

- [x] Read `## Implementation Plan` and `## Stage Report: implement`; understand what was claimed built.
  Plan/implement reports describe 4-layer change: state intent, resolver, terminal lifecycle + OSC 7, header/help/footer copy.
- [x] Compare diff `ab6cb1b..HEAD` against the plan across `src/app.rs`, `src/app/keys.rs`, `src/editor.rs`, `src/lib.rs`, `src/ui/mod.rs`; flag deviations.
  Diff matches plan exactly. Two minor, justified extras: (i) extra test `empty_visual_falls_through_to_editor` in `src/editor.rs` covering whitespace-only `$VISUAL` — defensive and welcome; (ii) `popup_h` bumped to 24 in the `is_multi && preview_open` branch (plan only specified the single-mode bump to 21) — necessary to keep the new help line visible in multi mode; commented inline. No material deviation.
- [x] Each AC has a named passing test or explicit evidence (AC-1..AC-7).
  AC-1 `o_with_preview_open_emits_open_file_intent` ok; AC-2 `suspend_resume_call_sequence` ok; AC-3 `o_with_preview_closed_is_noop` ok; AC-4 `resolve_editor_visual_editor_default_precedence` ok; AC-5 `emit_osc7_writes_bytes_when_tty` + `emit_osc7_skips_when_not_tty` ok; AC-6 `help_popup_documents_open_file_keybind_when_preview_open` ok; AC-7 `make lint` exits 0.
- [x] `make lint` clean.
  `cargo clippy --all-targets --all-features -- -D warnings` → `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.11s`; exit 0.
- [x] `cargo test --lib`; out-of-scope pre-existing failure noted and excluded.
  `220 passed; 1 failed` — the only failure `ui::graph::tests::narrow_tier_renders_compact_textual_summary` is pre-existing on `ab6cb1b`; `git diff --stat ab6cb1b HEAD -- src/ui/graph.rs` returns empty (file untouched by 039). Excluded from verdict per assignment.
- [x] Spot-check key claims: (a) OSC 7 pre-raw, (b) blocking spawn, (c) header relative+absolute fallback, (d) `o` no-op when preview closed.
  (a) `src/lib.rs:117-122` emits OSC 7; `enable_raw_mode()` follows at line 124, `EnterAlternateScreen` at line 128 — correct ordering. (b) `src/editor.rs:55-58` calls `.status()` (blocking), not `.spawn()`. (c) `src/ui/mod.rs:876-880` uses `strip_prefix(state.workflow_dir())` with `Err` → absolute fallback. (d) `src/app/keys.rs:82` arm is guarded by `if state.preview_open()`; when closed, control falls through to `_ => OverviewKeyAction::None`.
- [x] Hygiene: no new `[dependencies]`, no `unsafe`, no new `#[allow(clippy::...)]`.
  `git diff ab6cb1b HEAD -- Cargo.toml Cargo.lock` empty. Grep across the five touched files shows zero `unsafe` blocks. The only `#[allow(clippy::large_enum_variant)]` at `src/app.rs:21` pre-existed on `ab6cb1b` (verified via `git show ab6cb1b:src/app.rs`).
- [x] Write this `## Stage Report: review` section and commit on worktree branch.
  This section is the artifact; commit follows.

### Summary

Recommend approve. The implementation matches the plan one-for-one across all five touched files, all seven AC-tagged tests pass, `make lint` is clean with `-D warnings`, and the four spot-checked invariants (OSC 7 ordering, blocking spawn, header strip_prefix-with-fallback, `o` no-op when preview closed) all hold in the code. No new dependencies, no `unsafe`, no new `#[allow(...)]` introduced. The pre-existing `narrow_tier_renders_compact_textual_summary` failure reproduces on dispatch commit `ab6cb1b` and `src/ui/graph.rs` was not modified by this work, so it is out of scope per the assignment and does not affect the verdict.

### Feedback Cycles

**Cycle 1 — 2026-05-14, review → implement.**
Captain rejected at the review gate. Observed defect (screenshot from a live spacetop session on entity 039 at status=review): the **preview header's `path:` line renders as empty** — the label "path:" is shown with no value after it. The implement worker introduced the bug at `src/ui/mod.rs:876-880` when switching from the previous always-absolute render to `strip_prefix(state.workflow_dir())` with absolute fallback. Likely root cause to investigate: `Path::strip_prefix` returning `Ok` with an empty relative result (e.g. when `item.path == state.workflow_dir()` or some path-normalization edge case), or `item.path` and `workflow_dir` having mismatched normalization (trailing slash, symlinks, canonicalization). Fix and add a regression test that constructs an `OverviewState` + `WorkItem` whose rendered preview header contains a non-empty path string in both the in-workflow-root case and the out-of-root (worktree-resident) case.

## Stage Report: implement (cycle 1 fix)

- [x] Read the `### Feedback Cycles` section (cycle 1) added to the entity file on main.
  Captain rejected at review: preview header `path:` line renders visually EMPTY for entity 039 at status=review.
- [x] Reproduce the bug.
  Built spacetop from the worktree and rendered the live preview via a transient example binary (deleted before commit) that calls `decide_app` + `render` against a `TestBackend(width=200, height=50)` buffer. Output at preview-pane width ~100 (Left placement) showed `path:` alone on one row with the long absolute fallback path wrapping to the next row — exactly the captain's screenshot.
- [x] Diagnose the root cause in `src/ui/mod.rs:876-880`.
  Hypothesis (a) — `Path::strip_prefix` returning `Ok("")` — is a real defensive concern (verified that `Path::strip_prefix` returns `Ok("")` when the two paths are equal), but is NOT what produces the live bug. The actual root cause is the rendering path: `Paragraph::new(header_lines).wrap(Wrap { trim: true })` performs *word* wrapping on whitespace; a long absolute path has no internal whitespace, so ratatui breaks at the single space between the label `path:` and the value, putting the label alone on one row and the value on the next. To the user the label appears EMPTY. Confirmed by inspecting the rendered cell grid row-by-row before the fix.
- [x] Add a regression test in `src/ui/mod.rs` `#[cfg(test)]` covering BOTH the in-root case (relative path expected) and the out-of-root case (absolute fallback with truncation).
  New test `path_line_stays_visible_for_long_paths` builds an `App::from_snapshot` with a workflow_dir of `/repo/docs/wf` and two items — one in-root (`/repo/docs/wf/039-foo.md`) and one worktree-resident (`/repo/.worktrees/SLUG/docs/wf/040-bar.md`) — opens the preview on each, and uses a `find_text`-based row scan helper (`assert_path_row_non_empty`) to assert that the rendered row beginning with "path:" carries a non-empty visible value on the SAME row. Also asserts that the truncated form retains the basename and uses a leading ellipsis. Plus three unit tests for the new `fit_path_to_width` helper covering pass-through, truncation with ample width (full basename retained), and the tiny-pane collapse-to-ellipsis edge case.
- [x] Fix the bug. Preserve the design intent.
  Two-part fix: (i) defensively treat `Ok(rel)` from `strip_prefix` as a failure when `rel.as_os_str().is_empty()`, falling through to the absolute path (so the equal-paths edge case can never render an empty value); (ii) introduce `fit_path_to_width(value, pane_width)` next to `line_width` that truncates the rendered value from the LEFT with a leading `…` so the basename stays visible and the line fits on one row. Both the relative and absolute branches go through the same fit. Design intent preserved: relative-when-in-root, absolute fallback otherwise.
- [x] Run `make lint` and `cargo test --lib`.
  `make lint`: clean (`-D warnings`). `cargo test --lib`: `225 passed; 1 failed` — the one failure is the pre-existing `ui::graph::tests::narrow_tier_renders_compact_textual_summary` (graph.rs untouched by 039, confirmed via `git diff ab6cb1b HEAD -- src/ui/graph.rs` returning empty).
- [x] Append this report and commit; commit the fix in a separate commit.
  Fix commit `ab3bd75` "fix(039): keep preview header path on one line via width-fit truncation" — 1 file changed, 232 insertions(+), 5 deletions(-). Report commit follows.

### Summary

Diagnosed the cycle-1 review defect: the empty `path:` line was a rendering interaction with ratatui's word-wrap, not a `strip_prefix` normalization bug. Long absolute fallback paths (worktree-resident entities) wrapped at the single space after the label, hiding the value on the next row. Fixed by introducing `fit_path_to_width` to truncate the rendered value with a leading ellipsis so the basename stays visible on the same row as the label, and by treating an empty `strip_prefix` result as a fallback to the absolute path as defense-in-depth. New regression test `path_line_stays_visible_for_long_paths` (plus three `fit_path_to_width` unit tests) gates both the in-root and out-of-root cases. `make lint` clean; lib tests `225 passed; 1 failed` (pre-existing, out of scope).

## Stage Report: review (cycle 1)

- [x] Read the implement cycle-1 fix section and understood the claimed diagnosis (ratatui `Wrap { trim: true }` word-wrap at the single space after `path:` label) and fix shape (`fit_path_to_width` with leading-`…` truncation + defensive empty-`strip_prefix` fallback).
  See `## Stage Report: implement (cycle 1 fix)` above.
- [x] Inspected diff `04f209c..HEAD`: only `src/ui/mod.rs` and the entity file changed (`git diff --stat 04f209c..HEAD` → 2 files; 253 insertions, 5 deletions). Two new commits on the worktree branch: `ab3bd75` (fix) and `1db1bda` (report). Diff matches described diagnosis: introduces `fit_path_to_width` helper, calls it after `strip_prefix`, adds an `if !rel.as_os_str().is_empty()` guard on the `Ok(rel)` arm so equal paths fall through to the absolute branch.
  Diff confirms no production code touched outside `src/ui/mod.rs`.
- [x] Read `fit_path_to_width` (`src/ui/mod.rs:1161-1175`) and its call site (`src/ui/mod.rs:878-889`). Verified the four properties by reading the code and hand-tracing an 80-char path in a 40-wide pane: `label_chars=6`, `available=34`, `value_chars=80>34`, `available>1`, `skip=80-33=47`, result is `…` + last 33 chars (34 total), basename preserved at the tail. (a) Short paths pass through unchanged via `value_chars <= available` early return. (b) Long paths get a single leading `…` via `format!("\u{2026}{tail}")`. (c) Truncation keeps the trailing `available-1` chars so the basename stays at the end. (d) Tiny pane (`available <= 1`) collapses gracefully to just `…`.
  Helper code matches all four claims.
- [x] Verified each new regression test asserts what it claims: `path_line_stays_visible_for_long_paths` drives `App::from_snapshot` with both an in-root item (`/repo/docs/wf/039-foo.md`) and a worktree-resident item (long absolute path), opens the preview, and scans the rendered buffer row containing `path:` to assert a non-empty value follows the label on the SAME row — exactly the live bug the captain saw. `fit_path_to_width_keeps_short_path_unchanged` asserts identity for short input. `fit_path_to_width_truncates_long_path_with_leading_ellipsis` asserts `chars().count() == 34` for a 75-char path in width 40 and that the result starts with `…` and ends with `from-preview.md`. `fit_path_to_width_keeps_basename_when_room_is_ample` asserts the full basename remains visible at width 80. `fit_path_to_width_collapses_to_ellipsis_when_pane_is_tiny` asserts width 6 collapses to just `…`. All four would have caught the original bug if present pre-fix.
  Test bodies reviewed in `src/ui/mod.rs:1469-1659`.
- [x] All six original AC tests still pass: `cargo test --lib 2>&1 | grep` showed `o_with_preview_open_emits_open_file_intent ... ok`, `o_with_preview_closed_is_noop ... ok`, `resolve_editor_visual_editor_default_precedence ... ok`, `emit_osc7_skips_when_not_tty ... ok`, `suspend_resume_call_sequence ... ok`, `emit_osc7_writes_bytes_when_tty ... ok`, `help_popup_documents_open_file_keybind_when_preview_open ... ok`. None regressed.
  `cargo test --lib` summary: `225 passed; 1 failed` — the one failure `ui::graph::tests::narrow_tier_renders_compact_textual_summary` is out of scope per assignment.
- [x] Ran `make lint` from the worktree. Closing line: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.26s`. Exit 0.
  Clean with `-D warnings`.
- [x] Hygiene re-check: `git diff 04f209c..HEAD -- Cargo.toml Cargo.lock` is empty (no new dependencies). Grep for `unsafe` and `#[allow(clippy` in the diff returned no matches — no new `unsafe`, no new clippy allows.
  Code stays inside project guardrails.
- [x] Wrote this `## Stage Report: review (cycle 1)` section; commit follows.
  This section is the artifact.

### Summary

Recommend approve, with the caveat that this is a code-level verification only — the captain should re-run spacetop against entity 039 (or any worktree-resident entity with a long absolute path) to visually confirm the `path:` line now renders with the basename visible on the same row as the label. The code-level fix is sound: `fit_path_to_width` correctly truncates from the left with a leading `…` so the basename remains visible, the call site passes the actual inner pane width, and the defensive `is_empty()` guard on `strip_prefix` closes the equal-paths edge case. The new regression test `path_line_stays_visible_for_long_paths` exercises the live rendering path via `TestBackend` for both in-root and out-of-root items and would have failed against the pre-fix code. `make lint` is clean; the six original AC tests plus four new tests all pass; no new dependencies, `unsafe`, or clippy allows.
