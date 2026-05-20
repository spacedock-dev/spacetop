---
id: 041
title: View the active workflow's definition from the overview page
status: plan
source: captain request — inspect workflow structure (stages, gates, feedback edges) without leaving the TUI
started: 2026-05-20T08:13:05Z
completed:
verdict:
score:
worktree:
issue:
pr:
mod-block:
---

Captain request, verbatim: "we need a feature to open the definition of a workflow. In the task page, we can select the workflow to show the detail of the workflow itself."

Spacetop already renders the *state* of a workflow (entities, stages, feedback arrow) but the captain has no in-TUI way to see the workflow's *definition* — the README frontmatter that declares stages, their `initial`/`terminal`/`gate`/`fresh`/`worktree`/`feedback-to`/`concurrency` properties, the entity labels, and the prose blocks under each `### {stage}` heading. Today the only path is to quit and `cat docs/{workflow}/README.md`.

This task adds an in-TUI affordance, accessible from the overview (task) page, that opens a detail view of the currently-active workflow's definition. The view must make the structural decisions legible at a glance (which stage is initial, which is terminal, which is gated, where the reject edges go, which stages own a worktree, what each stage's concurrency cap is) and let the captain read the per-stage prose without leaving the TUI.

## Problem statement

The captain inspects workflow *state* in spacetop dozens of times per session but inspects the workflow *definition* far less often — usually when onboarding a new entity-type workflow, debating a stage-property change, or sanity-checking what the FO will do at a particular stage. Today that question costs a `q` (drop out of the TUI), an `$EDITOR docs/{workflow}/README.md`, and a context rebuild on re-launch. The cost is high enough that captains either skip the check or operate from memory of the README, which goes stale as workflows evolve. The structural facts the captain cares about — `initial`, `terminal`, `gate`, `fresh`, `worktree`, `feedback-to`, `concurrency`, plus the prose under each `### {stage}` heading — are already on disk in a known format (`parse_workflow_readme` parses the frontmatter today; the prose is sitting unread under each stage heading in the same file). The gap is purely presentational: there is no surface in the TUI that renders the definition.

The feature must respect the read-only invariant from `CLAUDE.md`: no new write paths, no mutation of the markdown tree. It must also avoid duplicating the stage-graph ribbon — the captain already sees stage order and feedback edges at the top of the overview; the definition view should add the structural *flags* and prose, not re-render the graph.

## Target user flow

1. Captain is in Overview for a workflow (single or one tab of a multi-workflow session).
2. Captain presses `D` (capital D, mnemonic for "Definition"). The overview is replaced by a full-pane Workflow Definition view scoped to the *active* workflow (the one currently focused in the tab strip or the only one in a `-w` session).
3. The Definition view renders, top to bottom:
   - A one-line header: `Workflow Definition  ·  {workflow basename}  ·  {entity-label-plural}` (dim path to the right, same scheme as the existing header bar).
   - A **Stages table** with one row per stage. Columns: stage name (colored using the existing `stage_color_for` palette), flags column (`initial` / `terminal` / `gate` / `fresh` / `worktree` rendered as compact `[…]` chips, only shown when true), `feedback-to` (renders `→ {target}` when set, em-dash otherwise), `concurrency` (the number, or em-dash when unset / unlimited).
   - A **Stage detail** section below the table: one block per stage with a `### {stage}` heading (in the stage color), followed by the README prose verbatim — the `Inputs`/`Outputs`/`Good`/`Bad` bullets are rendered through the same `markdown::render_markdown_termimad` pipeline the preview pane uses, so formatting matches what the captain already sees in entity bodies.
4. The view is scrollable: `PageUp` / `PageDown` and `Up` / `Down` / `j` / `k` scroll the body; `Home` / `End` jump to top / bottom. (No selection within the view — it is a read-only document, not a list.)
5. Captain presses `Esc` (or `D` again, or `q`) to return to the overview. The overview restores verbatim — selected entity, scope, scroll position, sort mode, archive cache.
6. The `?` help popup gains a single line documenting the `D` keybind. The status footer pill row gains a `D: definition` hint when the active mode is Overview (footer hints already adapt to mode, so this is a one-line addition in `status_footer_hints`).

### Multi-workflow interaction

In a multi-workflow session, `D` opens the definition view for the *active* tab. The tab strip (rendered by `render_workflow_tabs_panel`) is hidden inside the definition view (it scopes to one workflow at a time); pressing `Esc` returns to the tabbed overview with the same active workflow. Cycling tabs (`Left`/`Right`) is not bound inside the definition view — exit first, then cycle. Rationale: the definition view is per-workflow content; mixing tab-cycle navigation inside it would force a re-render path for each cycle and confuse the "Esc returns me where I was" contract.

## Chosen UX: full-pane Definition mode with `D` toggle and `Esc` return

**Decision: dedicated `AppMode::Definition` (or equivalent state) that owns the whole dashboard pane while open. Rejected alternatives:**

- **Modal overlay (sibling of the help popup).** The definition content is too tall for a centered popup — even a minimal workflow has 5 stages × ~6 prose lines = 30+ rows. An overlay would either truncate (defeating AC-3) or force its own scroll affordance overlapping the underlying overview's scrollbar, which is visually confusing. Rejected.
- **Reuse the preview pane (Enter on a synthetic "workflow" row).** Conflates entity-preview semantics with workflow-definition semantics. The preview pane already has worktree-diff, wrap toggle, and `o`-open-in-editor wired against a `WorkItem`; a `WorkflowDefinition` is not a `WorkItem` and shouldn't pretend to be one. Rejected.
- **Re-enter via the picker overlay (`P` with a "show definition" affordance).** Couples a frequently-needed read to the multi-workflow-only picker path; users in `-w` single-workflow sessions wouldn't have access. Rejected.
- **Inline expander above/below the stage ribbon.** Would push the task list down by a variable height every time the captain opens it, breaking the muscle-memory geometry of the overview. Rejected.
- **Open the README in `$EDITOR` (reuse the `o` keybind logic).** Already possible by other means, defeats the in-TUI goal, and shows raw markdown rather than the structured Stages table the captain actually wants. Rejected.

The full-pane choice mirrors the existing convention: `AppMode::Picker` and `AppMode::PickerOverlay` are full-pane swaps; the help popup is the only true overlay and it stays light enough to fit centered. The definition view is closer in weight to a picker than to help, so it sits alongside the picker family in the `AppMode` enum.

Key choice rationale: `D` (capital) is unused in `handle_overview_key`; lowercase `d` is reserved for future "delete" / "diff" semantics. `D` is mnemonic for "Definition," parallel to the existing `P` (picker) convention of using capital letters for top-level mode entry. `?` (help) and `q` (quit) bindings inside Definition mode behave the same as elsewhere; `Esc` exits the mode.

## Content surfaced (concrete contract for AC-2 and AC-3)

Per stage row in the **Stages table**, the view must surface every field on `StageDefinition` (`src/domain/mod.rs`):

| Source field          | Render when                  | Visual                                          |
|-----------------------|------------------------------|-------------------------------------------------|
| `name`                | always                       | colored using `stage_color_for(&name)`          |
| `initial: bool`       | `true`                       | `[initial]` chip                                |
| `terminal: bool`      | `true`                       | `[terminal]` chip                               |
| `gate: bool`          | `true`                       | `[gate]` chip                                   |
| `fresh: bool`         | `true`                       | `[fresh]` chip                                  |
| `worktree: bool`      | `true`                       | `[worktree]` chip                               |
| `feedback_to: Option` | `Some(t)`                    | `→ {t}` rendered in the target stage's color    |
| `feedback_to: Option` | `None`                       | em-dash `—` in dim style                        |
| `concurrency: Option` | `Some(n)`                    | the numeric value                               |
| `concurrency: Option` | `None`                       | em-dash `—` in dim style                        |

Workflow-scope fields rendered in the header line above the table: `entity_type`, `entity_label`, `entity_label_plural`, `id_style`, and the workflow root path. (These are already parsed into `WorkflowDefinition` by `parse_workflow_readme`; the view is purely a read of the existing struct.)

For **AC-3 (per-stage prose)** the view must reach the `### {stage}` body sections under each `## Stages` heading in the README. The current parser (`src/parser/readme.rs`) parses only the YAML frontmatter; the prose is unread. The design requires a **read-only extension** to the parser path — a new pure function `parse_stage_prose(readme_contents: &str) -> HashMap<String, String>` (location TBD by the plan stage: most likely a new `src/parser/readme_prose.rs` or an additional function in the existing `readme.rs`) that walks the markdown post-frontmatter and extracts the body under each `### {stage_name}` heading until the next heading of equal-or-higher level. Output: a `HashMap<String, String>` of stage name → raw markdown body (no rewriting, no truncation, no normalization). The Definition view renders each body through `markdown::render_markdown_termimad` (`src/ui/markdown.rs`) at the pane width, so bullets, bold, and code spans render identically to the entity preview.

The parse must be tolerant: a stage declared in frontmatter but missing a prose block renders an empty body section (with a dim "no description" placeholder). A `### {stage}` block whose name does not match any frontmatter stage is ignored (no error, no rendered ghost block). Both behaviors are unit-testable against fixture markdown without a TUI.

## Constraints

- **Read-only invariant (CLAUDE.md).** No new write paths in `src/discovery.rs` or `src/parser/*`. The new prose extractor is a pure `&str → HashMap` function: no `fs::write`, no `OpenOptions::write`, no mutation of `WorkflowDefinition` after parse.
- **Module ownership.** The new mode lives in `src/app.rs` (the `AppMode` enum gains a variant) and `src/app/keys.rs` (the `D` key maps to a new `OverviewKeyAction::OpenDefinition` or equivalent). Rendering lives in a new `src/ui/definition.rs` module sibling to `graph.rs` / `picker.rs` / `diff.rs`, wired in from `src/ui/mod.rs`'s top-level `render` match arm. Parser extension lands in `src/parser/readme.rs` (or a new `src/parser/readme_prose.rs`) and is invoked once at parse time so `WorkflowDefinition` carries the prose map alongside `stages` — this keeps the render path zero-IO.
- **No re-implementation of the stage graph.** The existing graph ribbon (`src/ui/graph.rs`) stays untouched. The Stages table is a different presentation — flags + feedback-target + concurrency — not a redraw of the directed graph.
- **Stage color reuse.** Both the Stages table row labels and the `### {stage}` headings in the prose section call `WorkflowDefinition::stage_color_for` so the colors agree with the graph ribbon and the entity list.
- **Lint gate (`make lint`).** All new code must clear `cargo clippy -- -D warnings` per CLAUDE.md.
- **Test seams.** The prose extractor is unit-testable against string fixtures; the new `AppMode` variant is testable via `handle_overview_key` (existing pattern); the renderer is testable via `TestBackend` with a synthetic `WorkflowDefinition` (existing pattern from `src/ui/mod.rs::tests`).

## Acceptance criteria

**AC-1 — `D` from Overview opens the Workflow Definition view; `Esc` returns to Overview with state intact.**
Verified by: a unit test in `src/app/keys.rs::tests` that constructs an `OverviewSession`, feeds `KeyCode::Char('D')`, and asserts the resulting `OverviewKeyAction` is the new "open definition" intent (or the equivalent mode transition is observable on `App::mode()`). A second test feeds `Esc` from the definition mode and asserts `App::mode()` is back to `AppMode::Overview` with the same `selected_index`, `view_scope`, `sort_mode`, and `preview_open` flags as before. The `?` help popup render test additionally asserts `D` appears in the keymap list.

**AC-2 — The Stages table exposes every `StageDefinition` field declared in the README, with no silent drops.**
Verified by: a unit test that constructs a `WorkflowDefinition` with a 5-stage fixture exercising every flag combination (`initial`, `terminal`, `gate`, `fresh`, `worktree`, `feedback_to`, `concurrency`) and renders the Definition view to a `TestBackend`. Assertions: each stage name appears; each `true`-valued flag's chip text appears on the corresponding row; each `feedback_to: Some(t)` renders `→ {t}` and each `None` renders the em-dash; `concurrency: Some(n)` renders `n` and `None` renders the em-dash. Also assert the workflow-scope fields (`entity_label_plural`, `id_style`) appear in the header line.

**AC-3 — Per-stage README prose (Inputs/Outputs/Good/Bad bullets) is reachable inside the view, verbatim from the README.**
Verified by: a parser unit test that feeds a fixture README with prose under each `### {stage}` heading into the new `parse_stage_prose` (or equivalent) extractor and asserts the returned `HashMap<String, String>` has one entry per stage with bytes byte-equal to the input (no normalization). A render test then loads the real `docs/spacetop-dev/README.md`, opens the Definition view, and asserts a stable substring from each stage's body (e.g. "Approved design notes" from the `plan` stage's Inputs bullet) appears in the rendered buffer.

**AC-4 — Read-only invariant preserved: no new write paths, no mutation of the markdown tree.**
Verified by: a `grep -r "fs::write\|OpenOptions" src/parser/ src/discovery.rs` in the test plan (or a build-time `#[deny]` if practical) returning no new occurrences. `make lint` clean. The new `parse_stage_prose` function signature `(&str) -> HashMap<String, String>` (pure, no `&Path`, no `fs`) is itself the structural guarantee.

**AC-5 — Single-workflow and multi-workflow sessions both support `D`; in multi mode `D` scopes to the active tab and `Esc` returns to the tabbed overview with the same active tab.**
Verified by: a render test on a 3-workflow fixture cycles to the middle tab, presses `D`, asserts the Definition view's header carries the middle workflow's basename, presses `Esc`, and asserts `OverviewSession::active_index()` is still the middle index with the same per-tab `selected_index` as before.

## Stage Report: design

- DONE: Name the user flow for opening the workflow-definition view from the overview (key/trigger, render surface — full-page vs overlay, exit/return behavior) and justify the pick against at least one rejected alternative.
  Captured in `## Target user flow` and `## Chosen UX: full-pane Definition mode with D toggle and Esc return`; rejected alternatives include modal overlay, preview-pane reuse, picker-coupled access, inline expander, and `$EDITOR` open.
- DONE: Spell out which stage-definition fields (`initial` / `terminal` / `gate` / `fresh` / `worktree` / `feedback-to` / `concurrency`) and which README prose sections the view must surface, so AC-2 and AC-3 become concretely testable.
  Captured in `## Content surfaced (concrete contract for AC-2 and AC-3)` with a per-field rendering table and the `parse_stage_prose` extractor contract for the `### {stage}` Inputs/Outputs/Good/Bad bullets.
- DONE: Confirm the design respects the read-only invariant from CLAUDE.md — no new write paths in discovery.rs / parser.rs — and identify (by name) the rendering and app-state modules the new mode will touch.
  Captured in `## Constraints` — touches `src/app.rs` (new `AppMode` variant), `src/app/keys.rs` (new key intent), new `src/ui/definition.rs` wired from `src/ui/mod.rs`, and a pure `&str → HashMap` extension in `src/parser/readme.rs`; no `fs::write` paths added.

### Summary

Designed a full-pane `AppMode::Definition` reachable via `D` from Overview and dismissed via `Esc`, with a Stages table surfacing every `StageDefinition` field plus per-stage README prose rendered through the existing termimad pipeline. Refined the seeded AC list from 4 to 5 — kept AC-1..AC-4 (tightened verification language and the parser-extension contract) and added AC-5 to pin down multi-workflow tab semantics. Read-only invariant preserved: the only parser addition is a pure `&str → HashMap` prose extractor; no new write paths.

AC coverage: AC-1 covered by `## Target user flow` + key binding choice in `## Chosen UX`; AC-2 covered by the field table in `## Content surfaced`; AC-3 covered by the `parse_stage_prose` contract in the same section; AC-4 covered by `## Constraints` (module ownership + read-only guarantee); AC-5 covered by the `### Multi-workflow interaction` subsection of `## Target user flow`. Commit: `264fdf8`.
