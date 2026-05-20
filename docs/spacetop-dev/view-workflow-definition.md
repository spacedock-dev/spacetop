---
id: 041
title: View the active workflow's definition from the overview page
status: implement
source: captain request — inspect workflow structure (stages, gates, feedback edges) without leaving the TUI
started: 2026-05-20T08:13:05Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-view-workflow-definition
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

## Implementation plan

This plan is for a separate implement-stage worker running in a worktree. It is broken into three work units (parser/domain, app-state, rendering) that can be implemented top-to-bottom or in parallel against module boundaries. Each step names the file it touches and the unit of code it adds. All design decisions (full-pane `AppMode::Definition`, `D` opens, `Esc` returns, scroll keys, content surfaced) are fixed by the `## Chosen UX` and `## Content surfaced` sections above — the implementer does not redecide them.

### Work unit A — Parser / domain (prose extractor)

Goal: surface the per-stage README prose (`### {stage}` Inputs/Outputs/Good/Bad bullets) on `WorkflowDefinition` without adding any write paths.

1. **`src/parser/readme.rs` — add `parse_stage_prose`.**
   New pure function:
   ```rust
   pub(crate) fn parse_stage_prose(readme_contents: &str) -> HashMap<String, String>
   ```
   Behavior:
   - Strip frontmatter using `extract_frontmatter` (already in `super::frontmatter`) and operate on the markdown body only.
   - Walk lines; whenever a line matches `^###\s+(.+?)\s*$`, capture the trimmed stage name and collect every subsequent line into the body until the next line matches `^(#|##|###)\s` (heading of equal-or-higher level). The body is stored as the raw substring (preserving newlines) with no normalization, no rewriting.
   - Stage names found in prose but not declared in frontmatter are silently retained in the map (the renderer ignores them; storing them costs nothing).
   - Stage names declared in frontmatter but absent from prose simply have no entry — the renderer must tolerate a missing key.
   - No `fs::*`, no `&Path`. Signature is `(&str) -> HashMap<String, String>`.
2. **`src/domain/mod.rs` — extend `WorkflowDefinition`.**
   Add a new field: `pub stage_prose: HashMap<String, String>` next to `stage_colors`. Default to empty `HashMap::new()`. Update **every** existing constructor / fixture in tests that builds `WorkflowDefinition` literally (search via `rg "WorkflowDefinition\s*\{" src/` — at least `app/keys.rs::tests`, `ui/mod.rs::tests`, `app/tests.rs`, possibly `parser/tests.rs`) to add `stage_prose: HashMap::new()`. No accessor method required; renderer reads the field directly.
3. **`src/parser/readme.rs::parse_workflow_readme` — wire the extractor.**
   After computing `stage_colors`, call `parse_stage_prose(&contents)` and populate the new field on the returned `WorkflowDefinition`. The function is already reading `contents` from disk; the extension is one extra line plus the field on the struct literal.
4. **`src/parser/readme.rs` unit tests (`#[cfg(test)] mod tests`).** Add a tests module (or extend the existing tests via `parser/tests.rs` if shared) with fixtures covering:
   - extractor returns one entry per `### {stage}` block with byte-equal body (`prose_extracts_stage_body_verbatim`);
   - frontmatter-only stage (no prose) yields no entry, no panic (`prose_missing_block_is_silent`);
   - prose-only stage (no frontmatter declaration) is non-fatal (`prose_unknown_stage_is_retained_or_ignored` — assertion is "no panic, output map ok");
   - integration test against `docs/spacetop-dev/README.md` confirms the `plan` stage body contains the substring "Approved design notes" (`prose_extracts_real_readme_plan_stage`).

### Work unit B — App-state (`AppMode::Definition`)

Goal: capture the user intent to open / close the Definition view, with full preservation of the underlying `OverviewSession` (including the active tab in multi mode).

1. **`src/app.rs` — extend `AppMode`.**
   Add a new variant:
   ```rust
   Definition {
       underlying: OverviewSession,
       scroll: usize,
   }
   ```
   Place it alongside `PickerOverlay` (which already follows the `underlying: OverviewSession` shape).
2. **`src/app.rs` — accessors.**
   - `App::as_session()` already covers `Overview` and `PickerOverlay`; extend it to also expose the underlying session when in `Definition` mode (so the status footer and any external probe still sees the active workflow).
   - Add `App::is_definition(&self) -> bool` for tests and for `render()` to dispatch.
   - The back-compat accessor `App::overview()` currently `panic!`s for non-overview modes; extend the match arms so `Definition { underlying, .. }` returns `underlying.active_state()` — preserves preview header rendering during a transient frame between close and overview re-entry, and lets existing tests that probe `selected_index()` etc. still work.
3. **`src/app/keys.rs` — emit a new intent for `D`.**
   Add `OpenDefinition` to `OverviewKeyAction` and bind:
   ```rust
   KeyCode::Char('D') if !state.preview_open() => OverviewKeyAction::OpenDefinition,
   ```
   (`!state.preview_open()` mirrors the `s` binding's guard so the `D` key cannot fire while the preview pane is consuming keys.)
4. **`src/app.rs::apply_overview_key_action` — handle `OpenDefinition`.**
   Pull the active session out of `self.mode` (mirroring `open_picker_overlay_with`'s `mem::replace` pattern) and install `AppMode::Definition { underlying: session, scroll: 0 }`.
5. **`src/app.rs::handle_key` — service the Definition mode.**
   Add a match arm for `AppMode::Definition { underlying, scroll }` that handles:
   - `KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('D')` → restore the underlying session (re-wrap into `AppMode::Overview`), keeping the active tab index and per-tab state intact.
   - `KeyCode::Char('?')` → set `help_open = true` (mirrors the picker pattern).
   - `KeyCode::Down | KeyCode::Char('j')` → `*scroll = scroll.saturating_add(1)`.
   - `KeyCode::Up | KeyCode::Char('k')` → `*scroll = scroll.saturating_sub(1)`.
   - `KeyCode::PageDown` → `*scroll = scroll.saturating_add(10)`.
   - `KeyCode::PageUp` → `*scroll = scroll.saturating_sub(10)`.
   - `KeyCode::Home` → `*scroll = 0`.
   - `KeyCode::End` → `*scroll = usize::MAX` (the renderer clamps to `content_height - body_height`).
6. **`src/app.rs::tests` — extend with three cases.**
   - `d_from_overview_enters_definition_mode` — calls `App::handle_key(KeyCode::Char('D'))` on an overview, asserts `matches!(app.mode(), AppMode::Definition { .. })`.
   - `esc_from_definition_restores_overview_state` — opens definition, mutates a probe (e.g. moves the underlying selection before opening — actually we capture `selected_index` before opening, press `D`, press `Esc`, assert `selected_index` / `view_scope` / `sort_mode` / `preview_open` are unchanged).
   - `d_ignored_when_preview_open` — preview open + `D` keeps mode as `Overview`.

### Work unit C — Rendering (`src/ui/definition.rs`)

Goal: full-pane render of the Stages table and per-stage prose, driven by `WorkflowDefinition` + `stage_prose` + scroll offset. No I/O on the render path.

1. **`src/ui/definition.rs` — new module.**
   Public entry point:
   ```rust
   pub fn render_in(
       frame: &mut Frame<'_>,
       area: Rect,
       definition: &WorkflowDefinition,
       scroll: usize,
   )
   ```
   Internal layout (vertical):
   - **Row 0 (1 line)** — header: `Workflow Definition  ·  {basename}  ·  {entity_label_plural || "entities"}`; right-aligned dim path. Mirror `render_header_bar`'s casing/dim scheme.
   - **Row 1 (1 line)** — workflow-scope sub-line: `id-style: {id_style || "—"}  ·  entity-type: {entity_type || "—"}  ·  entity-label: {entity_label || "—"}`. Dim styling.
   - **Stages table block** — minimum 2 rows + (stages.len()) rows. Columns: `Stage` (left, stage color), `Flags` (left, list of chips like `[initial] [gate]` separated by spaces; chips rendered with dim brackets and normal text), `Feedback` (left, `→ {target}` colored with `definition.stage_color_for(target)` or em-dash), `Concurrency` (right, number or em-dash). Header row in dim.
   - **Stage detail blocks** — for each stage in `definition.stages.iter()`, render:
     1. one blank line (visual separator);
     2. a heading line: `### {name}` in the stage color, bold;
     3. the prose body — looked up via `definition.stage_prose.get(&stage.name)` — rendered through `crate::ui::markdown::render_markdown_termimad(body, area.width)` to a `Vec<Line>`. When the lookup misses, emit one dim line `"(no description in README)"`.
   The function flattens all of the above into a single `Vec<Line<'_>>`, renders a single `Paragraph::new(lines).scroll((scroll as u16, 0)).wrap(Wrap { trim: false })` into the body area (everything below row 0/1), and renders a vertical `Scrollbar` on the right edge when content overflows. (Reuse the scrollbar wiring pattern from `render_preview`.)
2. **`src/ui/mod.rs::render` — dispatch.**
   Add a `mod definition;` declaration at the top. Extend the top-level `match app.mode()` to:
   ```rust
   AppMode::Definition { underlying, scroll } => {
       definition::render_in(frame, frame.area(), &underlying.active_state().snapshot().definition, *scroll);
   }
   ```
   The `Definition` view owns the entire frame area — no tab strip, no graph ribbon, no status footer. (Tab strip suppression is automatic because we no longer call `render_overview`.)
3. **`src/ui/mod.rs::render_help_popup` — add `D` line.**
   Extend the `lines` vec with a new entry:
   ```rust
   lines.push(Line::from("  D              open workflow definition"));
   ```
   placed near `s` (sort). Bump `popup_h` height bookkeeping (`+1` for both branches).
4. **`src/ui/mod.rs::status_footer_hints` — add `D: definition` pill.**
   Append `"D: definition"` to the hint list when `!preview_open` (matches the design intent that `D` is an Overview-mode entry point).
5. **`src/ui/definition.rs` tests (`#[cfg(test)] mod tests`).** Use `TestBackend` with a synthetic `WorkflowDefinition` plus a populated `stage_prose` map. Tests:
   - `stages_table_renders_every_stage_field` — 5-stage fixture exercising every flag combination; asserts each stage name, each `true` flag chip, each `feedback_to` arrow / em-dash, each `concurrency` value / em-dash, and header-line scope fields appear in the rendered buffer.
   - `stage_prose_block_appears_in_view` — fixture with prose for `plan`; assert the body substring "Approved design notes" is present.
   - `missing_stage_prose_renders_placeholder` — fixture with prose missing for one stage; assert the dim "no description in README" placeholder appears for that stage.
   - `definition_renders_against_real_readme` — load `docs/spacetop-dev/README.md` via `App::load`, transition to `AppMode::Definition`, render, and assert each frontmatter-declared stage name appears in the buffer.

### Work unit D — Multi-workflow / AC-5 coverage

This is small enough to fold into work unit B+C but is called out separately because it carries its own AC.

1. **`src/app.rs::tests` — `definition_scopes_to_active_tab_and_esc_preserves_index`.**
   Build an `OverviewSession::from_discovery` with three discovered workflows; cycle to index 1 via `KeyCode::Right`; press `D`; press `Esc`. Assert `session.active_index() == 1` both before and after the cycle, and the per-tab `selected_index` is unchanged.
2. **`src/ui/definition.rs::tests` — header carries active tab basename.**
   Same 3-workflow fixture; cycle to middle; render with `AppMode::Definition`; assert the header line includes the middle workflow's basename.

## Test strategy

| AC   | Test file                          | Test name                                                | Asserts                                                                                                                       |
|------|------------------------------------|----------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------|
| AC-1 | `src/app/keys.rs::tests`           | `d_from_overview_emits_open_definition_action`           | `KeyCode::Char('D')` on an overview (preview closed) yields `OverviewKeyAction::OpenDefinition`.                              |
| AC-1 | `src/app.rs::tests`                | `d_from_overview_enters_definition_mode`                 | `App::handle_key('D')` transitions `App::mode()` to `AppMode::Definition`.                                                    |
| AC-1 | `src/app.rs::tests`                | `esc_from_definition_restores_overview_state`            | After open + close, `selected_index`, `view_scope`, `sort_mode`, `preview_open` are bit-for-bit unchanged from before open.   |
| AC-1 | `src/ui/mod.rs::tests`             | `help_popup_lists_definition_keybind`                    | Rendered help popup buffer contains `"D"` and `"open workflow definition"` on the same row.                                   |
| AC-2 | `src/ui/definition.rs::tests`      | `stages_table_renders_every_stage_field`                 | 5-stage fixture; every flag chip / feedback target / concurrency value / em-dash visible in `TestBackend` buffer.             |
| AC-2 | `src/ui/definition.rs::tests`      | `header_carries_scope_fields`                            | `id_style`, `entity_label_plural` strings appear in the header / sub-line.                                                    |
| AC-3 | `src/parser/readme.rs::tests`      | `prose_extracts_stage_body_verbatim`                     | Fixture prose under `### {stage}` is byte-equal to the value returned by `parse_stage_prose` for that key.                    |
| AC-3 | `src/parser/readme.rs::tests`      | `prose_missing_block_is_silent`                          | Frontmatter-declared stage with no `### {stage}` block returns no entry, no panic.                                            |
| AC-3 | `src/parser/readme.rs::tests`      | `prose_extracts_real_readme_plan_stage`                  | Loading `docs/spacetop-dev/README.md` and looking up the `plan` stage yields a body containing `"Approved design notes"`.     |
| AC-3 | `src/ui/definition.rs::tests`      | `stage_prose_block_appears_in_view`                      | Rendered view contains `"Approved design notes"` from the `plan` stage's Inputs bullet.                                       |
| AC-4 | `src/parser/readme.rs::tests`      | `parse_stage_prose_signature_is_pure`                    | Compile-time: signature is `fn(&str) -> HashMap<String, String>`; no `Path`, no `fs::*` calls inside the function body.       |
| AC-4 | shell at verification time         | `grep -rn 'fs::write\|OpenOptions' src/parser src/discovery.rs` | Returns the same set of hits as `git show HEAD~1:src/parser` (no new write paths introduced).                            |
| AC-5 | `src/app.rs::tests`                | `definition_scopes_to_active_tab_and_esc_preserves_index`| 3-workflow session; middle tab; `D`; `Esc`; `active_index() == 1` retained, per-tab `selected_index` retained.                |
| AC-5 | `src/ui/definition.rs::tests`      | `definition_header_carries_active_tab_basename`          | Rendered header contains the basename of the active tab's workflow root.                                                      |

Verification commands the implement-stage worker will run before reporting done:

- `make lint` — clears `cargo clippy --all-targets --all-features -- -D warnings` (CLAUDE.md mandatory gate).
- `cargo test` — full suite must pass.
- `cargo test parse_stage_prose` — targeted parser tests.
- `cargo test --test '*' definition` and `cargo test definition` — targeted module tests.
- `cargo test d_from_overview esc_from_definition d_ignored_when_preview_open definition_scopes_to_active_tab` — targeted app-state tests.
- `grep -rn 'fs::write\|OpenOptions::write' src/parser src/discovery.rs` — must return only pre-existing hits.

## Files touched

Files the implement-stage worker is permitted to create or modify (everything else is out of scope):

- `src/parser/readme.rs` — add `parse_stage_prose` + tests + call site in `parse_workflow_readme`.
- `src/parser/tests.rs` — extend if shared fixtures land here.
- `src/domain/mod.rs` — add `stage_prose` field to `WorkflowDefinition`; update fixtures inside its `#[cfg(test)] mod tests`.
- `src/app.rs` — extend `AppMode`, `as_session`, `overview` accessor, `handle_key`, `apply_overview_key_action`; add tests at `src/app/tests.rs`.
- `src/app/tests.rs` — new test cases for definition mode entry / exit / scope.
- `src/app/keys.rs` — add `OpenDefinition` variant + `D` binding + unit test.
- `src/ui/mod.rs` — `mod definition;`, dispatch arm, help popup line, footer hint, height bookkeeping.
- `src/ui/definition.rs` — new module (render + tests).
- Any test fixtures that literally construct `WorkflowDefinition { … }` need `stage_prose: HashMap::new()` added; identify via `rg "WorkflowDefinition\s*\{" src/` (expected: `src/app/keys.rs::tests`, `src/ui/mod.rs::tests`, `src/app/tests.rs`). These are mechanical edits, not behavior changes.

Files explicitly off-limits (read-only invariant):

- `src/discovery.rs` — untouched; the definition view is a presentational read of already-parsed data, not a new discovery surface.
- `src/parser/frontmatter.rs`, `src/parser/item.rs`, `src/parser/snapshot.rs`, `src/parser/archive.rs`, `src/parser/worktree.rs` — no mutation needed; the new prose extractor lives inside `src/parser/readme.rs` as a pure helper and does not require touching the other parser surfaces.
- Any file under `agents/`, `references/`, or workflow `README.md` scaffolding — protected per CLAUDE.md / scaffolding guardrails.
- No new `fs::write` / `OpenOptions::write` paths anywhere in the codebase. The new prose extractor's signature `(&str) -> HashMap<String, String>` is the structural guarantee of read-only behavior.

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

## Stage Report: plan

- DONE: Produce a step-by-step implementation plan that separates parser/domain work, app-state work, and rendering work — each step naming the file(s) it touches and the unit it adds.
  See `## Implementation plan` — three work units (A: Parser/domain in `src/parser/readme.rs` + `src/domain/mod.rs`, B: App-state in `src/app.rs` + `src/app/keys.rs`, C: Rendering in new `src/ui/definition.rs` wired from `src/ui/mod.rs`), plus a small AC-5 multi-workflow follow-up; each step names its file and the symbol it adds.
- DONE: Spell out a test strategy mapping each AC (AC-1..AC-5) to a concrete test and name the verification commands the implement-stage worker will run.
  See `## Test strategy` — a per-AC table mapping AC-1..AC-5 to `#[cfg(test)]` test names with file locations and the exact assertions, plus the verification command list (`make lint`, `cargo test`, targeted `cargo test parse_stage_prose` / `cargo test definition` / `cargo test d_from_overview esc_from_definition`, and the `grep` for new write paths).
- DONE: Confirm the read-only invariant — list every file the implement stage may modify and explicitly state that `src/discovery.rs` and existing write surfaces stay untouched.
  See `## Files touched` — explicit allow-list of permitted files plus an off-limits list that names `src/discovery.rs` and every existing `src/parser/*.rs` surface besides `readme.rs`; the new prose extractor is a pure `(&str) -> HashMap<String, String>` helper, not a mutation of existing parser internals.

AC coverage: AC-1 — work units B (`OpenDefinition` variant, `D` binding, `Esc` exit) and tests `d_from_overview_emits_open_definition_action` / `d_from_overview_enters_definition_mode` / `esc_from_definition_restores_overview_state` / `help_popup_lists_definition_keybind`. AC-2 — work unit C step 1 (Stages table with every `StageDefinition` field) and tests `stages_table_renders_every_stage_field` / `header_carries_scope_fields`. AC-3 — work unit A (`parse_stage_prose` + `stage_prose` field) and work unit C step 1 (per-stage prose blocks); tests `prose_extracts_stage_body_verbatim` / `prose_missing_block_is_silent` / `prose_extracts_real_readme_plan_stage` / `stage_prose_block_appears_in_view`. AC-4 — work unit A pure-function contract, `## Files touched` allow/deny lists; tests `parse_stage_prose_signature_is_pure` and the verification-time `grep`. AC-5 — work unit D and tests `definition_scopes_to_active_tab_and_esc_preserves_index` / `definition_header_carries_active_tab_basename`.

### Summary

Decomposed the approved design into four work units (parser/domain, app-state, rendering, multi-workflow follow-up) totaling 14 named test cases across `src/parser/readme.rs`, `src/domain/mod.rs`, `src/app.rs`, `src/app/keys.rs`, and a new `src/ui/definition.rs`. The new mode is plumbed exactly like `AppMode::PickerOverlay` (capture `OverviewSession` underlying state, restore on `Esc`) so multi-workflow tab preservation comes for free. Read-only invariant locked in by a pure `&str → HashMap<String, String>` extractor signature plus an explicit files-touched allow-list that excludes `src/discovery.rs` and every existing parser file besides `readme.rs`.
