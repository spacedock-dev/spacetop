---
id: "028"
title: "Refine TUI list view — Tokyo Night palette, DAG header, task row layout"
status: plan
source: feature request
started: 2026-04-26T05:37:33Z
completed:
verdict:
score: 0.85
worktree:
issue:
pr:
---

Overhaul the spacetop list view to a strict terminal-safe aesthetic (text + box-drawing only) with a Tokyo Night-ish muted palette, a centered workflow DAG header, and a redesigned task row layout. All output must survive a plain terminal font with no emoji or decorative icons outside the defined glyph vocabulary.

## Spec

### Header strip (single line)

```
Workflow [active]  archived: (press a)  /path/to/workflow
```

- `[active]` — yellow filled badge, bg-colored bold text
- `archived: (press a)` — muted; `(press a)` is a key hint
- Path — dimmed; truncate from the **left** with `…` on overflow

---

### Workflow DAG (centered, monospace grid)

**Main row:**

```
▶ pending  ──►  ⚑ ⎇ hypothesis  ──►  ⎇ smoke  ──►  ⎇ run  ──►  ⚑ analyze  ──►  ⎇ n-runs  ──►  ⎇ holdout  ──►  done ■
```

**Glyph vocabulary** (fixed, not derived from phase names):

| Glyph | Meaning |
|-------|---------|
| `▶` | Start of pipeline (initial stage) |
| `⎇` | Branchable / regular stage |
| `⚑` | Gate / checkpoint stage |
| `■` | Terminal stage |

**Per-phase color:** oklch-derived, shared lightness 0.78 and chroma 0.12, hue varies per stage index. Active phase: `bg = phase color`, `fg = bg color`, bold.

**Counts row:** integer count centered under each phase label, dimmed.

**Rollback arc** (drawn below the main row in red, box-drawing chars):

```
          ↑   reject     │
          ╰──────────────╯
```

Spans from the anchor under the first char of the `feedback-to` target stage to the anchor under the first char of the stage being rejected (i.e. the `feedback-to` arrow in reverse). Uses box-drawing characters only.

---

### Task list rows

```
  run         17  README ablation arm 1 — direct single agent with minimal README
▸ smoke       18  README ablation arm 2 — direct single agent with methodology …
  hypothesis  11  Query batching — all queries visible, shared exploration
```

- **Gutter:** 2-char wide; `▸` for selected row, space otherwise
- **Phase column:** phase name only — no glyph, user's original casing preserved, fixed 12-char width, ellipsized with `…` if longer. Color = phase color. No auto-uppercasing or title-casing.
- **ID column:** 4-char right-aligned
- **Title column:** fills remaining width, ellipsized with `…`
- **Selected row:** `bg-2` background fill + 2-cell yellow left border
- **No glyphs in the task list phase column** — color alone carries the signal there

---

### Section headers

```
Tasks · 12    Preview · #17
```

Muted, normal (sentence) case, no uppercasing. Slight letter-spacing where the terminal supports it.

---

### Footer

Muted hint bar. Key hints (`?` `/` `⏎` `→` `a` `q`) shown in subtle `bg-2` pill style.

---

### Half-width tmux variant (~80–90 columns)

- Header condenses to one tight line
- DAG wraps to 2 rows: row 1 ends at the last stage before the midpoint (carry the reject arc on row 1), row 2 continues from the next stage
- Tasks and preview stack vertically (tasks on top, preview below)
- Footer key hints split across 2 short lines

---

### Anti-patterns to avoid

- **No 3-letter abbreviations** — do not auto-derive `RUN`, `HYP`, `DON` from phase names
- **No force-casing** — preserve user-defined phase name casing exactly
- **No glyphs in the task-list phase column** — glyphs appear only in the DAG
- **No decorative emoji or icons** outside the glyph vocabulary (`▶ ⎇ ⚑ ■ ──► ↑ ╰ ╯ ▸ …`)
- **No Unicode that won't render** in a typical terminal font

## Acceptance criteria

**AC-1 -- Header strip renders correctly.**
Single-line header shows: yellow badge for active count, muted archived hint with key callout, left-truncated dimmed path. Verified by: snapshot test asserting badge color, muted style, and left-truncation on a short terminal width.

**AC-2 -- DAG glyphs and colors are correct.**
Each stage is prefixed with the correct glyph (`▶`, `⎇`, `⚑`, `■`) based on its `initial`/`gate`/`terminal` properties. Active stage renders with inverted bg/fg. Non-active stages use per-stage hue with shared oklch lightness/chroma. Verified by: snapshot test on a known workflow fixture.

**AC-3 -- Rollback arc renders with box-drawing chars only.**
When a stage has `feedback-to`, the rollback arc is drawn below the DAG row using `↑ ╰ ─ ╯` characters in red. Verified by: snapshot test asserting arc characters and color.

**AC-4 -- Task list rows match spec.**
Gutter shows `▸` on selected row. Phase column is 12 chars fixed, user casing preserved, no glyphs, colored. ID is 4-char right-aligned. Title ellipsized. Selected row has yellow left border and bg-2 fill. Verified by: snapshot tests for selected/unselected rows and long phase name ellipsis.

**AC-5 -- Half-width variant wraps correctly.**
At ~80 columns the DAG wraps to 2 rows and tasks/preview stack vertically. Verified by: snapshot test at width=80.

**AC-6 -- Anti-patterns are absent.**
No uppercase phase names, no glyphs in task-list phase column, no emoji outside the vocabulary. Verified by: snapshot test assertions on rendered buffer.
