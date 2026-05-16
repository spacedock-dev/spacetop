---
id: "007"
title: Render preview-pane markdown with termimad
status: implement
source: captain
started: 2026-05-16T08:59:16Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-007-termimad-markdown-preview
issue:
pr:
---

Replace the current hand-rolled markdown rendering used in the preview pane with the [`termimad`](https://crates.io/crates/termimad) Rust crate.

Today the preview body is rendered via `render_markdown_lines` in `src/ui/mod.rs` (and friends in `src/ui/`). That path produces ratatui `Line`s from pulldown-cmark events with a minimal style mapping. `termimad` is a mature crate that already handles headings, lists, inline code, code blocks, emphasis, links, tables, and width-aware wrapping with a coherent visual style.

Scope:

- Use `termimad` only for the **preview-pane body** path — the markdown body of a selected work item. The unified-diff preview (`render_diff_lines` in `src/ui/diff.rs`, used when the worktree body diverges from main) stays as-is.
- Integration must remain renderable inside ratatui without taking over the screen. `termimad` exposes string-based renderers (`MadSkin::term_text`, `text` builders, or `FmtText`) that produce ANSI-styled strings; convert that output into ratatui `Line<'static>`/`Text<'static>` so it composes inside the existing preview `Paragraph` and scrollbar logic.
- Preserve current preview behavior: width-aware wrapping at `body_inner.width`, scrollbar when content overflows, and the existing tests that snapshot preview output for non-diff cases.
- Parsing, app state, and discovery code stays untouched. This is a rendering-only change inside `src/ui/`.

Implementation hints (not binding on design):

- A small adapter module (e.g. `src/ui/markdown.rs`) that takes `(body: &str, width: u16)` and returns `Vec<Line<'static>>` keeps the seam testable without a terminal backend.
- For ANSI-to-ratatui conversion, consider `ansi-to-tui` or a hand-written parser that maps SGR sequences to `Style`. Pick one and justify in design.
- `termimad::MadSkin` is the customization surface; align colors with the existing oklch-based stage palette only if it's natural — otherwise default skin is fine for v1.

## Acceptance criteria

**AC-1 — Preview pane bodies render via `termimad` end-to-end.**
Verified by: a unit test in `src/ui/` that takes a representative markdown body (headings, lists, inline code, code block, emphasis) and asserts the produced `Vec<Line<'static>>` contains styled spans distinctive to `termimad`'s output (e.g. inline code background style, heading color/weight) — not the prior hand-rolled output.

**AC-2 — Width-aware wrapping and scrollbar behavior preserved.**
Verified by: an existing or new test that renders a body wider than the preview area and asserts the line count exceeds `body_inner.height`, triggering the scrollbar path in `src/ui/mod.rs`. Pre-existing preview tests in `src/ui/mod.rs` continue to pass (update fixture strings only when the new renderer's output legitimately differs).

**AC-3 — Diff preview path is unchanged.**
Verified by: `src/ui/diff.rs` tests (including `render_diff_lines_treats_new_as_plus_old_as_minus`) pass without modification, and grep shows `render_diff_lines` is still the call when `main_body` is `Some(_)` in `src/ui/mod.rs`.

**AC-4 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy with `-D warnings`) and `cargo test` from the repo root, both green. Adding `termimad` (and any ANSI adapter) to `Cargo.toml` is in scope.

## Stage Report: implement

- DONE: AC-1 (new test exercises the termimad-backed renderer end-to-end).
  - `src/ui/markdown.rs::tests::renders_termimad_distinctive_styling` asserts heading is bold, inline code carries the `Color::DarkGray` slab background, and code-block spans are `Color::Cyan` on `Color::DarkGray`. Verified by `cargo test --lib ui::markdown::tests::renders_termimad_distinctive_styling` (passes).
  - Replaced the in-module unit test that used to call the deleted helper at `src/ui/mod.rs:3170-3230` with `render_markdown_termimad_multiline_code_block_emits_one_line_per_source_line`, which goes through `markdown::render_markdown_termimad` and asserts each source line keeps its Cyan/DarkGray slab span padded to at least the requested pane width.

- DONE: AC-2 (width-aware wrapping and scrollbar behavior preserved).
  - Added `src/ui/markdown.rs::tests::wide_content_wraps_to_multiple_lines` which renders a 400-char body at width 40 and asserts more than 4 lines are produced — the same precondition the overflow path in `src/ui/mod.rs:679` uses to decide `show_scrollbar`.
  - Pre-existing scroll/wrap tests still pass: `preview_draws_scrollbar_when_content_overflows`, `preview_page_down_scrolls_visible_markdown_content`, `preview_right_key_horizontally_scrolls_long_lines`, `word_wrap_toggle_changes_body_render`, `code_block_long_line_both_wrapped_rows_have_full_background` — all green under `cargo test --lib ui::`.
  - Two width-related adjustments inside `render_preview` (`src/ui/mod.rs:669-715`): when `state.preview_wrap()` is false we render at `MARKDOWN_NO_WRAP_RENDER_WIDTH = 4096` so termimad does not pre-wrap paragraphs (otherwise `max_preview_scroll_x` collapses to 0 and `HORIZONTALSCROLLTARGET` would never become visible); and the scrollbar second-pass re-render is now gated on wrap-on to avoid undoing the wide-render trick.
  - `trim_trailing_padding` in `src/ui/markdown.rs:56-72` drops the unstyled right-margin spaces ratskin appends so ratatui's `Wrap { trim: false }` does not re-wrap them onto blank rows (which broke the long-code-line background test).

- DONE: AC-3 (diff preview path untouched).
  - `grep -n "render_diff_lines" src/ui/diff.rs src/ui/mod.rs` still shows the diff renderer is invoked iff `item.main_body` is `Some(_)` (`src/ui/mod.rs:664-667`). The `main_body.is_some()` branch did not change.
  - `cargo test --lib ui::diff::tests::` passes unchanged. The `preview_renders_diff_when_main_body_present` and `preview_falls_back_to_body_when_main_body_none` regression tests in `src/ui/mod.rs:3686+` also pass.

- DONE: AC-4 (`make lint` and `cargo test` both green).
  - `make lint` from the worktree root: `cargo clippy --all-targets --all-features -- -D warnings` finishes with no warnings (output ends with `Finished \`dev\` profile`).
  - `cargo test` from the worktree root: 231 passed, 1 failed. The single failure is `ui::graph::tests::narrow_tier_renders_compact_textual_summary` (`src/ui/graph.rs:861`, "missing narrow arrow"). Confirmed pre-existing on the dispatch commit `30b7d08` (ran `git stash && cargo test --lib ui::graph::tests::narrow_tier_renders_compact_textual_summary` → same failure with no local changes applied). Unrelated to markdown rendering.

### Notes on implementation choices

- Picked `ratskin = "0.3"` (which re-exports `termimad`) as the ANSI/ratatui adapter rather than `ansi-to-tui`. ratskin reads termimad's `FmtText` directly and emits `ratatui::text::Line<'a>`, so we avoid a string ⇄ ANSI ⇄ Line round-trip and never touch SGR codes. Verified `ratskin 0.3.1` builds cleanly against this project's `ratatui 0.30` (`cargo build` and `cargo test` both green). The single behavior we work around is the right-completion padding ratskin appends — handled in `trim_trailing_padding`.
- The preview skin (`src/ui/markdown.rs::preview_skin`) overrides `code_block` and `inline_code` to use `CtColor::DarkCyan` / `CtColor::DarkGrey`. Termimad uses crossterm colors; the `FromCrossterm` impl in ratatui-crossterm 0.1 maps `CrosstermColor::DarkCyan → ratatui Color::Cyan` and `DarkGrey → DarkGray`, which preserves the historical Cyan-on-DarkGray slab the existing preview tests already pin (`preview_renders_fenced_code_block_without_backtick_fences`, `code_block_background_fills_pane_width_in_wrap_mode`, etc.).
- Deleted `render_markdown_lines`, `TableRender`, `flush_text_block`, `flush_line`, and `add_markdown_block_spacing` from `src/ui/mod.rs` (≈250 lines), plus the `pulldown-cmark` crate from `Cargo.toml` — there are no other consumers (`grep -rn pulldown_cmark src/ tests/` empty after the change).
- Updated `preview_renders_markdown_tables_as_aligned_rows` in `src/ui/mod.rs:1702` to assert the termimad table format (Unicode `│` cell borders, `─` separator row, every cell value present, no raw `| Arm |` or `---` leakage). The previous "no-borders, two-space columns" form was specific to the hand-rolled `TableRender` and would not be a meaningful assertion against the new renderer.

### Commands run

- `cargo build` — succeeded.
- `cargo test --lib ui::` — passed except pre-existing `ui::graph::tests::narrow_tier_renders_compact_textual_summary`.
- `cargo test` (full) — 231 passed, 1 pre-existing failure (as above).
- `make lint` — clean, no clippy warnings.

## Stage Report: review

- DONE: AC-1 — Termimad-distinctive styling test exists and passes.
  `src/ui/markdown.rs:106` `renders_termimad_distinctive_styling` asserts heading BOLD modifier, inline-code `Color::DarkGray` bg, and code-block `Cyan`-on-`DarkGray` slab spans; `cargo test` shows it passing.
- DONE: AC-2 — Wrapping + scrollbar tests pass.
  `src/ui/markdown.rs:162` `wide_content_wraps_to_multiple_lines` plus existing `preview_draws_scrollbar_when_content_overflows`, `preview_right_key_horizontally_scrolls_long_lines`, `word_wrap_toggle_changes_body_render`, `code_block_long_line_both_wrapped_rows_have_full_background`, `code_block_background_fills_pane_width_in_wrap_mode` all green; the wide-render-when-wrap-off seam at `src/ui/mod.rs:682-711` correctly preserves horizontal scroll behavior (no-wrap renders at `MARKDOWN_NO_WRAP_RENDER_WIDTH = 4096` so `max_preview_scroll_x` stays non-zero, and the second-pass re-render is gated on wrap-on so the trick is not undone).
- DONE: AC-3 — Diff preview path unchanged.
  `src/ui/mod.rs:669-672` still wraps `render_diff_lines` behind `item.main_body.as_deref().map(...)`; `preview_renders_diff_when_main_body_present` and `preview_falls_back_to_body_when_main_body_none` pass; `src/ui/diff.rs` tests unchanged and green.
- DONE: AC-4 — `make lint` and `cargo test` green (modulo pre-existing graph test).
  `make lint` → clean (no clippy warnings). `cargo test` → 231 passed, 1 failed = `ui::graph::tests::narrow_tier_renders_compact_textual_summary`. Confirmed pre-existing: the same assertion `"missing narrow arrow"` exists at dispatch commit `30b7d08:src/ui/graph.rs:863`, unrelated to markdown rendering. `pulldown_cmark` is fully gone from `src/`, `tests/`, and `Cargo.toml` (grep empty); `ratskin = "0.3"` and `termimad = "0.34"` added at `Cargo.toml:13-14`. The `preview_renders_markdown_tables_as_aligned_rows` rewrite (`src/ui/mod.rs:1703`) is not a weakening — it still asserts every cell value, no raw markdown leaks (`| Arm |`, `---`), plus presence of Unicode `│` borders and `─` separator rows, which is strictly more structural than the old "two-space columns" check.

### Summary

Implementation cleanly swaps in `ratskin`/`termimad` for the preview body path; the diff path and existing wrap/scroll/code-block contracts are preserved through a thoughtful render-width seam and trailing-padding trim. Lint is clean, test failures are pre-existing, and pulldown-cmark is genuinely dead (noting CLAUDE.md still lists it as preferred — a doc drift the FO can flag).

Recommendation: PASSED — ready for captain approval and merge.
