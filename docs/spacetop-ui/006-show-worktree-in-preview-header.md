---
id: "006"
title: Show worktree info in Preview header
status: implement
source: captain
started: 2026-05-12T01:23:24Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-006-show-worktree-in-preview-header
issue:
pr:
---

Surface the entity's `worktree` frontmatter field in the Preview pane header area so the captain can see at a glance whether a task currently has a live worktree (and where it is on disk) without opening the file.

Today the Preview pane renders `Preview · #id Title` on the first line and then a metadata block of `status:`, `score:`, `source:`, and `path:` (see `render_preview_header_lines` in `src/ui/mod.rs`, around lines 692–784). The `worktree` value is parsed by the entity loader but never shown. When the field is non-empty it is the strongest signal that an agent is (or recently was) dispatched on this entity — much more useful for the operator than `path:`, which is static.

The change should:

- Add a `worktree:` metadata line/segment alongside the existing `status`/`score`/`source` block, rendered for both `PreviewPlacement::Bottom` (single combined line, joined by `  ·  `) and `PreviewPlacement::Left` (its own line, matching the existing per-line style).
- When the field is empty, render it dimmed as `worktree: —` (em dash) or `worktree: none` so the absence is explicit rather than collapsing the row. Choose whichever reads cleanest with the rest of the header during design.
- When the field is non-empty, render the path value in the default (non-dim) style, matching how `source` is rendered today.
- Apply to both the active view and the archived view (archived entities also carry historical `worktree` values worth seeing).

## Acceptance criteria

**AC-1 — Worktree appears in Preview header for entities with a non-empty `worktree` field.**
Verified by: a `cargo test` rendering assertion that loads a fixture entity with `worktree: .worktrees/ensign-foo` set and asserts that the rendered preview buffer contains the substring `worktree: .worktrees/ensign-foo`.

**AC-2 — Worktree row renders an explicit empty marker when the field is unset.**
Verified by: a `cargo test` rendering assertion against a fixture entity with `worktree:` empty that asserts the rendered preview buffer contains `worktree: ` followed by the chosen empty marker (e.g. `—` or `none`) on a non-DIM-only line.

**AC-3 — Both Bottom and Left placements include the worktree segment.**
Verified by: `cargo test` cases that render the preview in each `PreviewPlacement` and assert the worktree text is present in the resulting buffer.

**AC-4 — No regression in existing header tests.**
Verified by: `make lint` passes and `cargo test` passes with the existing `src/ui/mod.rs` and `tests/` assertions unchanged or updated in lockstep with the new field.

## Design Notes

### Code locations to modify

All changes land in `src/ui/mod.rs`. No other modules need touching: the `worktree: Option<String>` field already exists on `WorkItem` (`src/domain/mod.rs:133`) and the parser already populates it.

1. **`build_preview_header_lines`** — `src/ui/mod.rs:678–795` (currently ends at L795 with the `lines` return; the divider lives at L789–L792, and the `path:` row at L784). This is the only render function that needs a behavioral change.
2. **New helper** — add a small `worktree_segment(item: &WorkItem, dim: Style)` (or inline closure) near the top of `build_preview_header_lines` to produce a reusable `Vec<Span<'_>>` for the `worktree:` cell. Keeps the Bottom/Left branches symmetric and matches how `status_spans` is already built at L707–L717.
3. **Tests** — add new `#[test] fn`s inside the existing `mod tests` block (anchor: after `bottom_preview_compacts_metadata_into_one_line` at L1262–L1276, which is the canonical pattern). Reuse the existing `item(...)` builder at L1309 and the `app_with_items(...)` harness at L1278; both already initialize `worktree: None`, so the new tests just mutate that field on the returned `WorkItem` before passing it in. No new fixture files required.

Approximate net diff: ~25 lines in `build_preview_header_lines`, ~80 lines of new tests.

### Empty-marker convention

Use the em dash `—` (U+2014), rendered with the existing `dim` style for both the label and the value, i.e. `worktree: —` (entire segment dimmed).

Why em dash over the literal `none`:
- The Preview header already uses em-dash-style visual separators (`·` U+00B7 between segments), so `—` reads as the same family of typographic punctuation rather than a magic English word.
- `none` collides with how `verdict: n/a`, `score: n/a`, and `source: n/a` are spelled in the rest of the header. Using a third spelling (`none`) for the same "absent" concept is inconsistent; using `—` deliberately signals "this field has no per-item value at all" vs. "the value is the literal n/a placeholder for a numeric/string slot".
- Dimming the whole `worktree: —` segment (label + value) matches the operator intent: an unset worktree is the common case and should fade into the background, while a set worktree should pop (label dimmed, value default — same treatment as `source:`).

### Render specification

For both active and archived views, in **`PreviewPlacement::Bottom`** (single combined line, L729–L741 / L761–L770):

- Append `Span::raw("  ·  ")` then the worktree segment to the same `spans` vec that already carries `status` / `score` / `source` (and, in archived, `verdict`).
- Set ordering: `status  ·  score  ·  source  ·  worktree[  ·  verdict]`. Place `worktree` after `source` because they share a "provenance" feel, and keep `verdict` last in archived to preserve the existing visual end-of-line cue.

For both active and archived views, in **`PreviewPlacement::Left`** (one `Line` per field, L742–L757 / L771–L781):

- Push a new `Line::from(worktree_segment(...))` after the `source` line and before any `verdict` line (so archived order becomes: status, score, source, worktree, verdict).

Segment contents:

- **Non-empty** (`item.worktree = Some(path)` where `path` is non-empty after trim): `Span::styled("worktree: ", dim)` followed by `Span::raw(path.clone())`. Matches the `source:` rendering at L735–L736 / L778–L779.
- **Empty** (`None`, or `Some("")` after trim): `Span::styled("worktree: ", dim)` followed by `Span::styled("—", dim)`. The em-dash span carries `dim` explicitly so it stays muted even though `Span::raw` defaults to no modifier.

Treat `Some("")` and `None` as the same "empty" case — the parser may yield either depending on how the YAML key was written (`worktree:` vs missing key), and the captain cannot distinguish them visually.

The `path:` row at L784 stays as-is; this design does not replace it.

### Test plan

Add the following `#[test]` functions inside the existing `mod tests { ... }` block in `src/ui/mod.rs`. All four reuse the existing `item(...)` / `app_with_items(...)` harness — no new fixtures.

1. **`bottom_preview_shows_worktree_when_set`** (covers AC-1 + AC-3 Bottom)
   - Build `let mut wi = item("001", "WT", "Body"); wi.worktree = Some(".worktrees/ensign-foo".to_string());`
   - Render in an 80×180 terminal (forces `PreviewPlacement::Bottom`, matching the existing `bottom_preview_compacts_metadata_into_one_line` setup).
   - Assert `rendered.contains("worktree: .worktrees/ensign-foo")`.

2. **`left_preview_shows_worktree_when_set`** (covers AC-1 + AC-3 Left)
   - Same item as above.
   - Render in a 180×24 terminal (forces `PreviewPlacement::Left`, matching `preview_opens_on_right_in_wide_terminals_and_bottom_in_taller_ones`).
   - Assert `rendered.contains("worktree: .worktrees/ensign-foo")`.

3. **`preview_renders_em_dash_for_empty_worktree`** (covers AC-2)
   - Build `let wi = item("001", "WT", "Body");` (leaves `worktree: None`).
   - Render in either terminal size (pick 80×180 for parity with test 1).
   - Assert `rendered.contains("worktree: —")` (literal em dash U+2014).
   - Also assert `rendered.contains("status: ● design")` to confirm the surrounding header is otherwise intact — guards against the empty-case branch nuking the rest of the line.

4. **`archived_preview_includes_worktree_segment`** (covers AC-3 across views)
   - Use the existing `archived_view_preview_renders_verdict_and_completed` (L1198) as a template: load the real `docs/spacetop-dev` workflow root via `App::load`, press `a` then `Enter`, render at 180×30.
   - Assert `rendered.contains("worktree:")` — substring only, since archived items may have varied real worktree values and we don't want to pin one. The existing fixtures under `docs/spacetop-dev/` are stable; if none of the currently-archived items has a non-empty `worktree`, the test still passes because the em-dash empty-marker row also contains the substring `"worktree:"`.

For AC-4, no existing assertions need rewriting — `status: ● design`, `score: 0.75`, `source: captain`, `verdict:`, and `completed:` substrings are all preserved by appending the new segment rather than rearranging the row.

## Stage Report: design

- DONE: Name the exact Rust functions and approximate line ranges to modify (primarily render_preview_header_lines in src/ui/mod.rs around L680-L784, plus any helpers/tests touched).
  Identified `build_preview_header_lines` at `src/ui/mod.rs:678–795`; new helper / inline span builder near L684–L690; new tests appended after `bottom_preview_compacts_metadata_into_one_line` at L1262 using the existing `item(...)` / `app_with_items(...)` harnesses at L1278/L1309.
- DONE: Decide and justify the empty-marker convention for an unset worktree field (e.g. em-dash vs the literal 'none'), and specify how the worktree segment is rendered in both PreviewPlacement::Bottom (joined by '  ·  ') and PreviewPlacement::Left (its own line) for active and archived views.
  Chose em dash `—` (U+2014) with the whole `worktree: —` segment dimmed; Bottom appends `  ·  worktree: <value>` after `source` (before `verdict` in archived); Left adds a dedicated `Line` after the `source` line (before `verdict` in archived). `Some("")` is treated the same as `None`.
- DONE: Translate the AC items into concrete cargo test assertions an implementer can write without further design questions (which fixture or test harness to extend, what substring/buffer assertions, which placement matrix to cover).
  Specified four `#[test]` functions in the existing `mod tests` block reusing `item(...)`/`app_with_items(...)`/`App::load`: bottom-set, left-set, empty-em-dash, and archived. All use `buffer_text(...)` + `rendered.contains(...)` substring assertions matching the existing pattern at L1273–L1275.

### Summary

The change is fully contained in `src/ui/mod.rs::build_preview_header_lines` plus four new substring tests. The worktree segment is rendered identically to `source:` in the non-empty case and as a dimmed `worktree: —` in the empty case, slotted after `source` in both placements and both views. No fixtures, no frontmatter changes, no parser changes — `WorkItem::worktree` already exists.

## Design Deviation (implement)

- Bottom-preview test fixture path changed from `.worktrees/ensign-foo` to `wt/foo`. At the design's prescribed 80×180 terminal, the combined metadata line (status · score · source · worktree) exceeds 80 cols with the long path and wraps mid-segment; `buffer_text` joins rows without delimiters but trailing-pads each row, so the substring is broken. Using `wt/foo` keeps the segment on one row at 80 cols; the rendered segment matches the design's spec verbatim. The Left-placement (180×24) and empty-marker tests follow the design unchanged.
- Pre-existing `preview_scrollbar_thumb_starts_at_top_at_zero_scroll` asserted `first_thumb_row < height/2`. The design adds one row to the Left-placement header (new worktree `Line` after `source:`), shifting the thumb from row 14 to row 15 of a 30-row buffer. Bound relaxed to `<= height/2`; still guards "thumb sits in the upper half of the track at scroll=0".

## Stage Report: implement

- DONE: cargo test passes with new assertions covering the worktree row for non-empty value, empty-marker value, and both PreviewPlacement variants (active + archived views per the design notes).
  191/192 lib tests passing; the sole failure (`ui::graph::tests::narrow_tier_renders_compact_textual_summary`) is pre-existing on the dispatch baseline and unrelated to this change.
- DONE: make lint passes — no new clippy warnings, no #[allow(...)] suppressions added without justification in the entity body.
  `make lint` finishes cleanly with `-D warnings`; no new allows introduced.
- DONE: Rendered Preview header matches the design notes' empty-marker convention and placement layout verbatim (no improvisation; if the design is unclear, append a Design Deviation note to the entity body before changing it).
  Em-dash empty marker, `worktree:` slotted after `source:` and before `verdict:` in both PreviewPlacement variants and both views, matching `build_preview_header_lines`. Two minor test-side deviations (fixture path length, scrollbar bound) documented in the Design Deviation (implement) section above.

### Summary

Added a reusable `worktree_segment` span builder in `build_preview_header_lines` and wired it into all four Bottom/Left × active/archived render paths. Added four new tests covering bottom-set, left-set, empty em-dash, and archived placement. Two small test deviations (shorter fixture path to avoid 80-col wrap; loosened scrollbar bound to absorb the design-mandated extra header row) are documented above; no behavioral or visual deviations from the design.

## Stage Report: review

- DONE: Verdict (PASSED or REJECTED) is stated explicitly, with the diff inspected for logic errors, regressions, and dead/unreachable code.
  PASSED. Diff (`bc1b3f1`) is contained to `src/ui/mod.rs`: a single `worktree_segment` builder near L687-L704 plus four wiring sites (Bottom/Left × active/archived) and four new tests. No dead code, no unreachable branches; `Some("")` and `None` are correctly collapsed by `.filter(|s| !s.is_empty())` after `str::trim`.
- DONE: Each AC-1..AC-4 in the entity body has a cited evidence line — which test assertion, file:line, or rendered-buffer check demonstrates it — or is explicitly flagged as unverified.
  AC-1: `bottom_preview_shows_worktree_when_set` (src/ui/mod.rs:1301-1314) and `left_preview_shows_worktree_when_set` (src/ui/mod.rs:1316-1329) both assert the literal `worktree: <path>` substring. AC-2: `preview_renders_em_dash_for_empty_worktree` (src/ui/mod.rs:1331-1347) asserts `worktree: \u{2014}` plus the surrounding `status: ● design` row remains intact. AC-3: AC-1's two tests cover both PreviewPlacement variants for active view; `archived_preview_includes_worktree_segment` (src/ui/mod.rs:1349-1369) covers archived. AC-4: `make lint` exits clean under `-D warnings`; 191/192 lib tests pass; sole failure (`ui::graph::tests::narrow_tier_renders_compact_textual_summary`) reproduces on `main` (verified by running it against the main worktree) and is unrelated.
- DONE: Rendered output matches the design notes' empty-marker and placement layout; any deviation is named and either justified or routed back to implement via REJECTED.
  Em-dash `\u{2014}` with the whole `worktree: —` segment dimmed; slotted after `source` and before `verdict` in archived, last in active Bottom, dedicated Line after `source` in Left. Two documented test-side deviations (fixture path `wt/foo` and scrollbar bound `<=`) are pragmatic adjustments to test harnesses, not visual deviations; accepted.

### Summary

PASSED. Implementation matches the design verbatim in render behavior; the worktree segment renders identically to `source` when set and as a dimmed `worktree: —` when unset. Lint clean, all relevant tests green, sole test failure is a pre-existing graph-rendering issue on main untouched by this diff.

### Feedback Cycles

#### Cycle 1 — review → implement (2026-05-12)

Captain rejected the review with one targeted change: the `worktree:` row should display only the basename (final path segment) of the worktree path, not the full relative path. Rationale: in practice every worktree lives under `.worktrees/`, so the prefix adds no information and the long string crowds the Bottom-placement single-line metadata row. The basename alone (e.g. `spacedock-ensign-006-show-worktree-in-preview-header`) is what uniquely identifies a worktree.

Scope of fix:
- Render `worktree: {basename}` when the field is non-empty. Use `std::path::Path::file_name` (fall back to the raw trimmed string if `file_name` returns `None`, which would only happen for trailing-slash edge cases).
- Empty-marker behavior is unchanged: still `worktree: —` (dim).
- Update the existing rendering tests to assert on the basename rather than the full path. AC-1 and AC-3 wording stays the same (the field is "the worktree value" — basename is still that value).
- All other layout decisions (placement, dim style, position between `source` and `verdict`) stay as-is.

Verification: existing four tests in `src/ui/mod.rs` (the ones added by the implement stage — `bottom_preview_shows_worktree_when_set`, `left_preview_shows_worktree_when_set`, `preview_renders_em_dash_for_empty_worktree`, `archived_preview_includes_worktree_segment`) updated to use basename fixtures and assertions. `cargo test` and `make lint` must remain green.

## Stage Report: implement (cycle 1)

- DONE: `worktree:` row renders only the basename (final path segment) of the path when set, falling back to the trimmed raw string only if `Path::file_name` returns None. Empty-marker behavior (`worktree: —` dim) unchanged.
  `build_preview_header_lines` now derives `basename` via `std::path::Path::new(path).file_name().and_then(|s| s.to_str()).map(str::to_string).unwrap_or_else(|| path.to_string())` before rendering. Em-dash branch untouched.
- DONE: The four worktree-related tests in src/ui/mod.rs are updated to assert on basename (not full path) and `cargo test` is green.
  `bottom_preview_shows_worktree_when_set` and `left_preview_shows_worktree_when_set` now use fixture `.worktrees/ensign-foo` and assert `worktree: ensign-foo` is present plus negative-assert the full path absent. Em-dash and archived tests unchanged (path-agnostic). 191/192 lib tests pass; sole failure is the pre-existing `narrow_tier_renders_compact_textual_summary` graph test unrelated to this diff.
- DONE: `make lint` passes — no new clippy warnings.
  `cargo clippy --all-targets --all-features -- -D warnings` finishes clean.

### Summary

Switched the `worktree:` segment to render the basename (`Path::file_name`) with a raw-string fallback for trailing-slash edge cases. Tests now use the realistic `.worktrees/ensign-foo` fixture and assert the basename appears while the full path does not. No layout, placement, or empty-marker changes — only the displayed value is shortened.
