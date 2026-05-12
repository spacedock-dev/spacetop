---
id: "006"
title: Show worktree info in Preview header
status: review
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
