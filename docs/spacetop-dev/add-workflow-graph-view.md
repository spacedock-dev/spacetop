---
id: 007
title: Render the workflow stage graph on the main TUI page
status: design
source: captain feedback after build-initial-tui-overview
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

The main TUI page should visually present the workflow's stage graph — nodes for each stage with its defaults/properties (initial, terminal, gate, worktree, feedback-to), edges for forward transitions, and a distinct edge style for feedback loops (e.g., `review --feedback-to--> implement`). The graph should be derived from the parsed `WorkflowSnapshot`, not hard-coded, so it reflects whatever workflow is loaded.

Open design questions to resolve in the `design` stage:

- Rendering approach in ratatui: ASCII/Unicode box-and-arrow layout, a Sugiyama-style layered layout, or a simpler stage ribbon? (Terminal width is the dominant constraint.)
- Layout algorithm: which crate or hand-rolled topological pass?
- How are gate / worktree / feedback markers encoded in the node glyph?
- Where does the graph sit on the main page — replaces the summary pane, sits above the list, toggles with a key?
- How are per-stage counts overlaid on the graph (badge inside the node, separate line)?

## Acceptance criteria

_To be firmed up during design. Expected shape:_

**AC-1 -- The main page renders stages as graph nodes derived from the loaded workflow, including feedback edges.**
Verified by: render test against `docs/spacetop-dev` fixture asserts each stage name, terminal/initial/gate markers, and at least one feedback edge.

**AC-2 -- The graph view updates automatically when a different workflow is loaded.**
Verified by: render test across two fixture workflows with different stage topologies.

**AC-3 -- Graph rendering degrades gracefully when the terminal is too narrow.**
Verified by: narrow-width render test.
