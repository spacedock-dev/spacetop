---
id: "008"
title: Render markdown inside the diff preview (worktree vs main)
status: done
source: captain
started: 2026-05-16T09:30:15Z
completed: 2026-05-16T12:17:39Z
verdict: PASSED
score:
worktree: 
issue:
pr: #35
archived: 2026-05-16T12:17:46Z
---

When an entity's worktree body diverges from the main copy, the preview pane currently renders a plain-text unified diff (gray context, green `+`, red `-`) via `render_diff_lines` in `src/ui/diff.rs`. After task 007, non-diff bodies render through `termimad` with full markdown styling — but the diff path still produces unstyled text. This task closes that gap so divergent bodies remain readable as markdown while still highlighting what changed.

## Background

- `render_diff_lines(old, new) -> Vec<Line<'static>>` in `src/ui/diff.rs` uses `similar::TextDiff::from_lines` and emits one styled line per change with a `+` / `-` / ` ` gutter character.
- The non-diff preview path is now `markdown::render_markdown_termimad(body, width)` (added in task 007).
- The diff path is selected at `src/ui/mod.rs` (look for `item.main_body.as_deref().map(...)` near line 669) when the worktree copy diverges from main.

## Two viable approaches

**(A) Render-then-diff.** Render each side via termimad → diff the produced `Line`s. Simple to implement; preserves markdown styling for every line. Downside: reflowed paragraphs and heading changes register as wholesale add/remove rather than aligned edits, because the diff happens on rendered cells rather than on source structure.

**(B) Diff-then-render.** Diff the markdown source at the block (or hunk) level using `similar`, then render each delta block via termimad and prefix a styled gutter span (`+` green, `-` red, ` ` dim) on every produced line. Slightly more work — needs a small helper that maps "block of source lines" through `render_markdown_termimad` and tags the resulting `Line`s with the gutter — but the result reads like a normal styled preview with green/red gutters.

The design stage should pick one (recommend (B)) with a short justification, name the API of the new helper, and define how context/equal blocks are rendered (probably full termimad styling with a dim ` ` gutter).

## Acceptance criteria

**AC-1 — Diff preview renders markdown styling for every line.**
Verified by: a unit test in `src/ui/diff.rs` (or a new `src/ui/diff_md.rs`) that constructs `old` and `new` markdown bodies containing headings, inline code, and a fenced code block; renders the diff preview; and asserts that styled spans distinctive to termimad's output (heading bold, inline-code background, code-block slab) appear on both context lines and add/remove lines.

**AC-2 — Gutter prefix is preserved and styled.**
Verified by: the same/another unit test that asserts every produced `Line` starts with a single-character gutter span whose content is `+`, `-`, or ` `, and whose style matches the existing diff palette (green for `+`, red for `-`, dim for context). Adjacent removed-then-added blocks still surface as `-` then `+`.

**AC-3 — Existing diff-routing behavior is unchanged.**
Verified by: the entry point in `src/ui/mod.rs` (around `item.main_body.as_deref().map(...)`) still selects the diff renderer iff `main_body` is `Some(_)`. Pre-existing tests `preview_renders_diff_when_main_body_present` and `preview_falls_back_to_body_when_main_body_none` continue to pass. The non-diff path from task 007 is untouched.

**AC-4 — Width-aware wrapping and scrollbar behavior still hold in the diff path.**
Verified by: a unit test that renders a divergent body wider than the preview area and asserts the produced line count exceeds `body_inner.height`, triggering the scrollbar path in `src/ui/mod.rs`.

**AC-5 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green.

## Stage Report: implement

- DONE: AC-1 + AC-2 — markdown styling + styled gutter on every diff line
  New unit test `render_diff_lines_styles_markdown_across_context_add_remove` in `src/ui/diff.rs:233` asserts heading bold on a context line, inline-code DarkGray bg on both `-old_code` and `+new_code` lines, Cyan-on-DarkGray slab on `-let removed = 1;` and `+let added = 2;`, and validates every produced `Line` starts with a `+`/`-`/` ` gutter span carrying Green/Red/Dim styling respectively. Implementation in `src/ui/diff.rs:107` (chunked diff-then-render with fence re-wrapping) passes.
- DONE: AC-3 + AC-4 — diff routing unchanged, non-diff path untouched, width-aware wrapping intact
  `src/ui/mod.rs:670` still selects the diff renderer iff `item.main_body.is_some()`; only the function name changed (`render_diff_lines` → `render_diff_lines_with_width(_, _, body_inner.width)`). Pre-existing regression tests `preview_renders_diff_when_main_body_present` and `preview_falls_back_to_body_when_main_body_none` pass unmodified (full `cargo test` output: `233 passed; 1 failed` where the one failure is the pre-existing `ui::graph::tests::narrow_tier_renders_compact_textual_summary`). AC-4 covered by `render_diff_lines_wraps_wide_content_for_scrollbar` in `src/ui/diff.rs:336`.
- DONE: AC-5 — `make lint` and `cargo test` green
  `make lint` clean (clippy `-D warnings`). `cargo test` shows the only failure is `ui::graph::tests::narrow_tier_renders_compact_textual_summary`, which reproduces on the dispatch commit `203ec10` (stash + checkout confirmed) and is the known pre-existing failure flagged in the assignment.

### Summary

Chose approach (B) diff-then-render: line-diff the markdown sources with `similar`, group adjacent same-tag changes into chunks, and feed each chunk through `render_markdown_termimad` before prefixing a styled gutter span. A small `fence_context` pre-pass tracks per-line code-fence state on each side so that chunks falling inside a fenced code block are re-wrapped with `\`\`\`lang … \`\`\`` before rendering — without this, an insert/delete chunk that lives inside a code fence loses its enclosing delimiters and termimad falls back to plain text. The diff selection at the call site is identical (`main_body.is_some()`); only the renderer behind it changed, so the 007 non-diff path is untouched.

## Stage Report: review

- DONE: AC-1 + AC-2 — Markdown styling + styled gutter test exists, exercises headings + inline code + fenced code block across context/add/remove, and passes
  `render_diff_lines_styles_markdown_across_context_add_remove` at `src/ui/diff.rs:275` asserts bold heading (context), DarkGray-bg inline code on both `-old_code` and `+new_code` lines, Cyan-on-DarkGray slab on `-let removed = 1;` / `+let added = 2;`, AND per-line gutter span (`+`/`-`/` `) with Green/Red/Dim styling; runs green (`cargo test --lib ui::diff::tests`: 5 passed).
- DONE: AC-3 + AC-4 — Diff routing unchanged at `src/ui/mod.rs`; non-diff path from 007 untouched; width-aware wrapping test passes
  `src/ui/mod.rs:669-672` still routes via `item.main_body.as_deref().map(...)`; only the call changed to `diff::render_diff_lines_with_width(main, &item.body, body_inner.width)`. Regression tests `preview_renders_diff_when_main_body_present` and `preview_falls_back_to_body_when_main_body_none` pass unmodified. AC-4 covered by `render_diff_lines_wraps_wide_content_for_scrollbar` at `src/ui/diff.rs:434` (800-char body wrapped to width 40 produces >10 lines).
- DONE: AC-5 — `make lint` clean and `cargo test` green modulo documented pre-existing failure
  `make lint` clean (clippy `-D warnings`, no warnings). `cargo test` final tally: `233 passed; 1 failed`, the single failure being `ui::graph::tests::narrow_tier_renders_compact_textual_summary` flagged in the assignment as pre-existing.

### Risk probe findings

- Fence-context toggling: `fence_context` (src/ui/diff.rs:33) correctly toggles `in_fence` on each ` ``` ` line; the fence delimiter line itself is marked `in_fence: false`, and the language token from the opener is captured and carried until the closer. Unclosed fences keep `in_fence: true` to EOF, which is a reasonable fallback (chunks still get re-wrapped with the captured language).
- Width-wrapping gutter survival: each `Line` produced by termimad is iterated independently and prefixed with the gutter span (src/ui/diff.rs:171-180). The AC-2 test asserts EVERY line in the rendered diff carries a gutter span, which implicitly proves wrapped visual lines each receive their own gutter. The 800-char wrap test confirms multi-line outputs from a single source line.
- 007 width seam: the diff path at `mod.rs:672` does NOT consult `preview_wrap()` / `MARKDOWN_NO_WRAP_RENDER_WIDTH` — diffs always wrap to `body_inner.width`. The non-diff markdown path retains the wrap-off horizontal-scroll behavior. This is a deliberate divergence (gutter alignment makes wrapping the natural choice for diffs) and AC-4 does not require horizontal scroll in the diff path; flagged as a minor follow-up if users later want no-wrap diff scrolling.
- Regression tests `preview_renders_diff_when_main_body_present` / `preview_falls_back_to_body_when_main_body_none` are intact and not weakened — assertions on `+NEW LINE` / `-OLD LINE` text still hold.

Recommendation: PASSED — ready for captain approval and merge.
