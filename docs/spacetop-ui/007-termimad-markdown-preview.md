---
id: "007"
title: Render preview-pane markdown with termimad
status: review
source: captain
started: 2026-05-16T08:59:16Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-007-termimad-markdown-preview
issue:
pr: #34
mod-block: merge:pr-merge
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
