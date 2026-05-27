---
id: 047
title: Color the sync result pill so the Y-sync outcome is readable at a glance
status: plan
source: captain
started: 2026-05-27T06:56:37Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

When the user presses `Y` to sync (`git pull --ff-only`), the result already
renders as a pill in the status footer (`Synced (already up to date)`,
`Synced (N new commits)`, `⚠ Sync failed: …`, `Sync unavailable: …`). The
captain reports the result is easy to miss: a **successful** sync pill is
styled identically to the neutral key hints beside it (white text on the same
dark pill background), so it reads as just another hint rather than the
outcome of their action. Only the failure case is visually distinct (red).

The captain wants the sync result to be obvious at a glance — at minimum,
whether it succeeded or not.

## Captain-approved direction

Keep the result in the footer, but color the sync pill by outcome and make
success distinguishable without relying on color alone:

- `Succeeded` → green, with a leading `✓ ` glyph (mirrors the existing `⚠ ` on failure)
- `Unavailable` → yellow
- `Failed` → red (unchanged)
- `InFlight` ("Syncing…") → cyan, so it reads as in-progress rather than done

Derive the pill color from the `SyncStatus` value, replacing the current
fragile `starts_with('⚠')` string-sniffing in the footer renderer. The key
hints stay neutral gray. Scope is footer-only: no changes to the sync logic
(`src/git_sync.rs`), the `Y` key path, or the in-flight→result flow.

## Acceptance criteria

- **AC-1** A `Succeeded` sync pill renders green and its label begins with `✓ ` (e.g. `✓ Synced (already up to date)`, `✓ Synced (3 new commits)`).
- **AC-2** A `Failed` sync pill renders red and keeps the `⚠ Sync failed: {message}` label.
- **AC-3** An `Unavailable` sync pill renders yellow with the `Sync unavailable: {hint}` label.
- **AC-4** An `InFlight` sync pill renders cyan with the `Syncing…` label.
- **AC-5** The pill color is derived from the `SyncStatus` variant, not by inspecting the label string; the neutral key hints remain unchanged in style.
- **AC-6** The pinned sync-pill label strings (and their tests) are updated together for the new `✓ ` success prefix; all other labels are unchanged.
- **AC-7** A render test asserts the success pill's foreground is green and the failed pill's foreground is red.
- **AC-8** `make lint` is clean and the full test suite (`cargo test`) passes; `src/git_sync.rs` behavior is untouched.

## Out of scope

`Y` only triggers a sync when the preview pane is closed. Making `Y` work with
the preview open is a separate concern and is not part of this task.

## Implementation plan

Footer-only change in `src/ui/footer.rs`. No edits to `src/git_sync.rs`, the
`Y` key handler, the `SyncStatus` enum (`src/app/overview.rs`), or the
in-flight→result event-loop flow.

### Problem with the current renderer

`render_status_footer` (footer.rs:19) colors pills by *sniffing the label
string*: `if hint.starts_with('\u{26A0}')` → red, else neutral. This is
fragile (couples color to the warn glyph, which is also used by the
`⚠ N broken` parse-error pill) and can only express red vs neutral. We replace
it with a color derived from the `SyncStatus` variant, computed where the pill
label is built.

### Edit 1 — carry per-pill color out of `status_footer_hints`

`status_footer_hints` currently returns `Vec<String>`. Change it to return
`Vec<(String, Color)>` so each pill carries its own foreground color. The
neutral key hints get `Color::White` (the current default); the sync pill and
the `⚠ N broken` pill get explicit colors.

- Add the import: extend the existing `style::Color` use (already imported at
  footer.rs:5) — no new dependency.
- `sync_pill_label` returns the label only; pair it with a color via a new
  helper (Edit 2). Push `(label, sync_color)` for the sync pill.
- The `⚠ N broken` pill keeps red (`Color::Red`) to preserve its current
  styling (today it is red via the `starts_with('⚠')` branch). Push
  `(format!("\u{26A0} {broken_count} broken"), Color::Red)`.
- Every static key-hint `.push(...)` becomes `.push((<label>, Color::White))`.

### Edit 2 — `SyncStatus` → `Color` mapping + leading `✓ ` on success

In `sync_pill_label` (footer.rs:88):

- Prepend the success labels with `✓ ` (`'\u{2713}'` + space). The three
  `Succeeded { .. }` arms become:
  - `new_commits: 0` → `"\u{2713} Synced (already up to date)"`
  - `new_commits: 1` → `"\u{2713} Synced (1 new commit)"`
  - `new_commits` → `format!("\u{2713} Synced ({new_commits} new commits)")`
- `InFlight`, `Failed`, `Unavailable` labels are unchanged (Failed keeps its
  `⚠ ` marker; Unavailable keeps `Sync unavailable: {hint}`; InFlight keeps
  `Syncing…`).
- Define a `SUCCESS_MARKER: char = '\u{2713}'` const beside the existing
  `SYNC_FAIL_MARKER` (footer.rs:14) and use it in the success arms for parity
  with the failure marker.
- Add a sibling `fn sync_pill_color(status: &SyncStatus) -> Color` that maps:
  - `InFlight` → `Color::Cyan`
  - `Succeeded { .. }` → `Color::Green`
  - `Failed { .. }` → `Color::Red`
  - `Unavailable { .. }` → `Color::Yellow`
  Call it from `status_footer_hints` when pushing the sync pill so color is
  derived from the variant, not the label (AC-5).

### Edit 3 — drop string-sniffing in the renderer

In `render_status_footer` (footer.rs:19–39):

- Delete the `broken_pill_style` / `pill_style` `starts_with('\u{26A0}')`
  branch.
- Build each span as `Style::default().fg(color).bg(PILL_BG)` using the color
  paired with each `(label, color)` entry from `status_footer_hints`.
- The `PILL_BG` background and the `"  "` separator styling are unchanged, so
  `footer_hints_have_background` keeps passing.

### Internal-only signature note

`status_footer_hints` is `pub(crate)` and is not asserted directly in tests
(only `sync_pill_label` is pinned). Changing its return type to
`Vec<(String, Color)>` is safe; the only caller is `render_status_footer` in
the same module. If `clippy` flags the tuple as worth a named struct, prefer a
small `struct FooterPill { label: String, color: Color }` — decide at
implement time based on the lint output; either satisfies AC-5.

## Test strategy

All tests live in `src/ui/tests/task_list.rs` (helpers come from
`src/ui/tests.rs` via `use super::*`: `app_with_items`, `find_styled_text`,
`buffer_text`, plus `App::set_sync_status`). Verification commands:
`make lint` (clippy `-D warnings`) and `cargo test`.

1. **Update the pinned label test** (AC-6).
   `footer_sync_pill_labels_match_pinned_strings` (task_list.rs:585) asserts
   `sync_pill_label` output verbatim. Update the three `Succeeded` expectations
   to the new `✓ `-prefixed strings:
   - `Succeeded { new_commits: 0 }` → `"\u{2713} Synced (already up to date)"`
   - `Succeeded { new_commits: 1 }` → `"\u{2713} Synced (1 new commit)"`
   - `Succeeded { new_commits: 3 }` → `"\u{2713} Synced (3 new commits)"`
   `InFlight`, `Failed`, `Unavailable`, and the `None` case stay unchanged.
   Update the string + its test together per project convention.

2. **Update the existing success render test** (AC-1).
   `footer_renders_sync_pill_when_status_set` (task_list.rs:622) asserts the
   buffer contains `"Synced (2 new commits)"` — still true as a substring of
   `"✓ Synced (2 new commits)"`, but tighten it to assert the `✓ ` prefix
   renders so the AC is covered by a render path, not just the unit test.

3. **New render test: success pill is green** (AC-7).
   Mirror `footer_renders_sync_pill_when_status_set`: `app_with_items`, close
   preview (`Enter`), `app.set_sync_status(Succeeded { new_commits: 2 })`,
   render to a `TestBackend`, then
   `assert!(find_styled_text(buffer, "Synced", |s| s.fg == Some(Color::Green)))`.
   `find_styled_text` (tests.rs:118) checks the predicate at every cell of the
   needle, and the renderer styles the whole pill span uniformly, so matching
   on `"Synced"` proves the green foreground.

4. **New render test: failed pill is red** (AC-2).
   Mirror `footer_renders_sync_failed_pill_with_warning_glyph`
   (task_list.rs:646): inject `Failed { message: "boom" }`, render, then
   `assert!(find_styled_text(buffer, "Sync failed", |s| s.fg == Some(Color::Red)))`.
   The label assertion (`⚠ Sync failed: boom`) stays as-is.

5. **Regression coverage already present.**
   `footer_hints_have_background` (task_list.rs:339) keeps the `PILL_BG`
   background guarantee; `footer_shows_broken_count_pill_when_parse_errors_present`
   and `footer_omits_broken_pill_when_no_parse_errors` keep the parse-error
   pill behavior. No `git_sync.rs` test changes (AC-8) — that file is untouched.

Optional (not required by ACs): a unit test asserting
`sync_pill_color(&InFlight) == Color::Cyan` and
`sync_pill_color(&Unavailable { .. }) == Color::Yellow` to lock AC-3/AC-4 at
the mapping level, since the cyan/yellow variants are harder to drive through a
full render in a deterministic footer layout. Decide at implement time.

## Stage Report: plan

- DONE: Implementation plan pins the exact src/ui/footer.rs edits: a SyncStatus->Color mapping (Succeeded=green, Unavailable=yellow, Failed=red, InFlight=cyan) plus the leading checkmark on success labels, replacing the starts_with(warn-glyph) string-sniffing in the renderer.
  See "## Implementation plan" — Edit 1 (carry per-pill Color out of status_footer_hints), Edit 2 (sync_pill_color mapping + `✓ ` SUCCESS_MARKER on the three Succeeded arms), Edit 3 (delete the starts_with('⚠') branch in render_status_footer); all anchored to current line numbers in footer.rs.
- DONE: Test strategy names the pinned label-string test to update (footer_sync_pill_labels_match_pinned_strings) for the new success prefix, and an added render assertion that the success pill foreground is green and the failed pill foreground is red.
  See "## Test strategy" items 1 (update the three Succeeded expectations in footer_sync_pill_labels_match_pinned_strings at task_list.rs:585), 3 (new green-fg render test via find_styled_text on "Synced"), and 4 (new red-fg render test via find_styled_text on "Sync failed").

### Summary

Produced a footer-only implementation plan and test strategy in the entity body; no implementation code was written (planning stage). The plan replaces the renderer's fragile `starts_with('⚠')` string-sniffing with a `SyncStatus`-derived color: it threads a per-pill `Color` out of `status_footer_hints` (changing its return type to `Vec<(String, Color)>`, an internal-only signature with a single in-module caller), adds a `sync_pill_color` mapping (cyan/green/red/yellow), and prepends a `✓ ` `SUCCESS_MARKER` to the three `Succeeded` labels for parity with the existing `⚠ ` failure marker. Test strategy updates the pinned `footer_sync_pill_labels_match_pinned_strings` strings together with the labels (per convention) and adds two render assertions using the existing `find_styled_text` helper (green success fg, red failed fg); cyan/yellow are noted as an optional mapping-level unit test since they are hard to drive deterministically through a full render. `src/git_sync.rs`, the `Y` handler, and the event-loop flow are explicitly out of scope.
