---
id: "017"
title: Survey termimad as markdown renderer for the preview pane
status: design
source: captain (follow-up from 016)
started: 2026-04-25T15:38:17Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

Task 016 fixed code block rendering by extending the existing `pulldown-cmark` event loop in `src/ui/mod.rs`. During review it was noted that `termimad` — a crate designed specifically for terminal markdown rendering — was not surveyed before committing to that approach. This task surveys `termimad`, decides whether it is a better fit, and either replaces the renderer or documents why `pulldown-cmark` should be kept.

## Acceptance criteria

**AC-1 -- termimad is evaluated against the current renderer.**
A written comparison covers: rendering quality for the content types in the 016 example section (fenced code blocks, inline code, headings, lists, bold/italic), API surface vs. manual event walking, Ratatui integration friction, and crate weight (transitive dependencies).
Verified by: design stage report contains the comparison.

**AC-2 -- A clear keep-or-replace recommendation is made.**
The design stage concludes with an explicit recommendation — replace with `termimad`, keep `pulldown-cmark`, or adopt a hybrid — backed by the AC-1 comparison.
Verified by: design stage report contains a `Recommendation:` line.

**AC-3 -- If replacing: the renderer swap is implemented and all existing preview tests pass.**
`termimad` replaces `pulldown-cmark` in `render_markdown_lines`; the 016 example content renders correctly; `cargo test --lib` exits 0.
Verified by: implement stage report with `cargo test --lib` output.

**AC-4 -- If keeping: the rationale is recorded in the entity body for future reference.**
A `## Decision` section is added to this file explaining why `pulldown-cmark` was retained.
Verified by: presence of `## Decision` section in the final entity file.
