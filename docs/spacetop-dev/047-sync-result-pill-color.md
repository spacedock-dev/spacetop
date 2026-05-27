---
id: 047
title: Color the sync result pill so the Y-sync outcome is readable at a glance
status: review
source: captain
started: 2026-05-27T06:56:37Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-047-sync-result-pill-color
issue:
pr: #46
mod-block: merge:pr-merge
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

## Stage Report: implement

- DONE: src/ui/footer.rs renders the sync pill with an outcome color derived from the SyncStatus variant (Succeeded=green, Unavailable=yellow, Failed=red, InFlight=cyan), prepends the success-marker checkmark to the three Succeeded labels, and removes the starts_with(warn-glyph) string-sniffing branch in render_status_footer.
  Added `fn sync_pill_color(&SyncStatus) -> Color` + `SUCCESS_MARKER` const; `status_footer_hints` now returns `Vec<(String, Color)>` (neutral hints White, broken pill Red, sync pill from `sync_pill_color`); renderer builds each span as `Style::default().fg(color).bg(PILL_BG)` with the per-pill color — the `starts_with('\u{26A0}')` branch is gone.
- DONE: Tests updated/added: the pinned footer_sync_pill_labels_match_pinned_strings expectations reflect the new success prefix, and render assertions prove the success pill foreground is green and the failed pill foreground is red (cyan/yellow covered by a mapping-level unit test).
  Updated the three Succeeded expectations to `✓ …` in footer_sync_pill_labels_match_pinned_strings; tightened footer_renders_sync_pill_when_status_set to require the `✓ ` prefix; added footer_renders_succeeded_sync_pill_green, footer_renders_failed_sync_pill_red, and sync_pill_color_maps_each_variant (cyan/green/red/yellow).
- DONE: `make lint` is clean (-D warnings) and `cargo test` passes in full.
  `make lint` finished with no clippy diagnostics; `cargo test` ran 336 lib tests + all integration suites (decide_app, git_sync_e2e 4/4, no_write_git_calls 2/2, readme_reload, etc.) green, 0 failed; the 3 watcher_fs tests are `#[ignore]` by design.

### Summary

Footer-only change in `src/ui/footer.rs`: the sync pill's foreground color is now derived from the `SyncStatus` variant via a new `sync_pill_color` mapping (InFlight=Cyan, Succeeded=Green, Failed=Red, Unavailable=Yellow), and `status_footer_hints` carries a `Color` per pill (`Vec<(String, Color)>`) so the renderer no longer sniffs the label string — the `starts_with('⚠')` branch is removed and the `⚠ N broken` pill keeps its red explicitly. Success labels gained a leading `✓ ` (`SUCCESS_MARKER`) for parity with the failure `⚠ `, and the pinned label test was updated in lockstep per the stable-strings convention. Only `src/ui/footer.rs` and `src/ui/tests/task_list.rs` changed; `src/git_sync.rs`, the `Y` key handler, `SyncStatus`, and the event-loop flow are untouched (AC-8). Commands run: `make lint` (clean), `cargo test` (all pass), `cargo test --lib` (336 passed) to confirm the new footer tests.

## Stage Report: review

- DONE: Independently re-run `make lint` and `cargo test` in the worktree and report the actual observed results (pass/fail counts), not the implement report's claims.
  Forced a fresh clippy check (`touch` on both changed files, then `cargo clippy --all-targets --all-features -- -D warnings`) — recompiled the crate, exit 0, zero diagnostics. `cargo test` observed: 336 lib passed / 0 failed; integration: decide_app 10/10, git_sync_e2e 4/4, no_write_git_calls 2/2, readme_reload 5/5, discovery_bypass 10/10, watcher_fs 0 passed/3 ignored (real-notify, `#[ignore]` by design), doc-tests 0; total 0 failed. The five footer tests (`footer_renders_succeeded_sync_pill_green`, `footer_renders_failed_sync_pill_red`, `sync_pill_color_maps_each_variant`, `footer_sync_pill_labels_match_pinned_strings`, `footer_renders_sync_pill_when_status_set`) all pass.
- DONE: Verify every acceptance criterion AC-1..AC-8 against the diff and tests with concrete evidence, and confirm scope: footer-only, with src/git_sync.rs, the Y key handler, SyncStatus, and the event-loop flow unchanged.
  AC-1: `sync_pill_label` success arms emit `{SUCCESS_MARKER} Synced …` (`SUCCESS_MARKER='\u{2713}'`=✓); `sync_pill_color(Succeeded)=Green`; render test asserts green fg on "Synced" and `footer_renders_sync_pill_when_status_set` now asserts the `✓ ` prefix renders. AC-2: `Failed` label arm unchanged (`⚠ Sync failed: {message}`), `sync_pill_color(Failed)=Red`, `footer_renders_failed_sync_pill_red` asserts red fg. AC-3: `Unavailable` label unchanged, maps to Yellow, locked by `sync_pill_color_maps_each_variant`. AC-4: `InFlight` label `Syncing…` unchanged, maps to Cyan, locked by same unit test. AC-5: color comes from `sync_pill_color(variant)`; the `starts_with('\u{26A0}')` renderer branch is deleted; all key hints pushed with `Color::White`. AC-6: three `Succeeded` expectations in `footer_sync_pill_labels_match_pinned_strings` updated to `✓ `-prefixed in lockstep; other labels unchanged. AC-7: green-success + red-failed render tests present and passing. AC-8: clippy clean, full suite green, `git diff main..HEAD --name-only` = only `footer.rs`, `tests/task_list.rs`, and the entity doc. Scope confirmed: `git_sync.rs` has no diff; `SyncStatus` (src/app/overview.rs:64) and the `Y` handler (src/app/keys.rs:123) are untouched.
- DONE: Give a clear PASSED/REJECTED verdict; if REJECTED, name specific defects and the exact fix needed.
  Verdict: PASSED. No defects. One non-blocking observation: `status_footer_hints` uses `sync_status.map(sync_pill_color).unwrap_or(Color::White)`, whose `unwrap_or` arm is dead because `sync_pill_label` returning `Some` guarantees `sync_status` is `Some` — it is defensive, comment-documented, clippy-clean, and harmless; not worth a rework.

### Summary

PASSED. The footer-only change satisfies all of AC-1..AC-8 with concrete diff and test evidence. The fragile `starts_with('⚠')` string-sniffing is replaced by a `SyncStatus`-derived `sync_pill_color` (Cyan/Green/Red/Yellow), success labels carry the `✓ ` marker for parity with the failure `⚠ `, and the pinned label test was updated in lockstep. Independently verified: forced-fresh `cargo clippy -D warnings` is clean (exit 0), and `cargo test` is fully green (336 lib + 21 integration passed, 0 failed; 3 watcher tests `#[ignore]` by design). Scope is clean — only `src/ui/footer.rs` and `src/ui/tests/task_list.rs` changed; `src/git_sync.rs`, the `Y` key path, `SyncStatus`, and the in-flight→result event-loop flow are untouched. Recommend advancing to done.
