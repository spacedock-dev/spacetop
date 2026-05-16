---
id: "008"
title: Render markdown inside the diff preview (worktree vs main)
status: review
source: captain
started: 2026-05-16T09:30:15Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-008-termimad-aware-diff-preview
issue:
pr:
mod-block: merge:pr-merge
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
