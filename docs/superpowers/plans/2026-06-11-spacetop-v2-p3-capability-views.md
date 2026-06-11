# SpaceTop v2 - Phase P3: Capability Views Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only TUI capabilities on top of the P1/P2 query API: search, command palette, timeline, metrics, activity, and richer entity detail.

**Architecture:** P3 is a UI and app-state layer over existing core queries. Core may add small serializable view models, but parsing, history, metrics, and relationship facts must remain in `spacetop-core`; TUI modules only render and dispatch commands. Every new view has an unavailable state rather than reaching around the query API.

**Tech Stack:** Rust 2021, `spacetop-core::index`, `spacetop-core::query`, Ratatui `TestBackend`, crossterm key events.

---

## Prerequisites

- P0, P1, and P2 are merged.
- `WorkflowIndex::query`, `timeline`, `metrics`, and `activity(since)` exist.
- P2's TUI history worker folds `WorkflowIndex::with_history_result(...)` into the active index without blocking the render/input loop, so P3 views see populated history after worker completion or a concrete `HistoryUnavailable` reason.
- This phase does not add workflow writes.

## Keymap decisions for P3

- `/` opens search overlay.
- `:` opens command palette overlay.
- `T` opens timeline for the selected entity.
- `M` opens metrics view.
- `A` opens activity feed.
- `R` opens relationships/details view for the selected entity.
- `Esc` returns to the prior overview session from every P3 overlay/view.

These bindings avoid existing lowercase `a` archive, `s` sort, `D` definition, `Y` sync, and preview navigation keys.

## File map

- Modify: `crates/spacetop-core/src/index.rs`
- Create: `crates/spacetop-core/src/relations.rs`
- Modify: `crates/spacetop-core/src/lib.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/app/keys.rs`
- Create: `crates/spacetop/src/app/search.rs`
- Create: `crates/spacetop/src/ui/search.rs`
- Create: `crates/spacetop/src/ui/timeline.rs`
- Create: `crates/spacetop/src/ui/metrics.rs`
- Create: `crates/spacetop/src/ui/activity.rs`
- Create: `crates/spacetop/src/ui/relations.rs`
- Modify: `crates/spacetop/src/ui/mod.rs`
- Modify: `crates/spacetop/src/ui/footer.rs`
- Modify: `crates/spacetop/src/ui/help.rs`

---

## Task 1: Add core entity details and relationship view models

**Files:**
- Create: `crates/spacetop-core/src/relations.rs`
- Modify: `crates/spacetop-core/src/index.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write relation tests**

Create `crates/spacetop-core/src/relations.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_relation_labels_are_stable() {
        assert_eq!(
            RelationView::Issue {
                value: "https://example.test/1".to_string()
            }
            .label(),
            "issue"
        );
        assert_eq!(
            RelationView::PullRequest {
                value: "https://example.test/pr/1".to_string()
            }
            .label(),
            "pr"
        );
    }

    #[test]
    fn entity_details_groups_core_facts_without_ui_inference() {
        let details = EntityDetails {
            id: "050".to_string(),
            title: "Roadmap".to_string(),
            status: "verify".to_string(),
            worktree: Some("p3".to_string()),
            relations: vec![RelationView::FeedbackStage {
                from: "verify".to_string(),
                to: "plan".to_string(),
            }],
        };
        assert_eq!(details.relations[0].label(), "feedback-to");
    }
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core relations::tests`

Expected: FAIL because `EntityDetails` and `RelationView` do not exist.

- [ ] **Step 3: Implement detail and relation types**

Add:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDetails {
    pub id: String,
    pub title: String,
    pub status: String,
    pub worktree: Option<String>,
    pub relations: Vec<RelationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationView {
    Issue { value: String },
    PullRequest { value: String },
    FeedbackStage { from: String, to: String },
}

impl RelationView {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Issue { .. } => "issue",
            Self::PullRequest { .. } => "pr",
            Self::FeedbackStage { .. } => "feedback-to",
        }
    }
}
```

- [ ] **Step 4: Add `WorkflowIndex::related` and `entity_details`**

In `index.rs`, add:

```rust
pub fn related(&self, entity_id: &str) -> Vec<crate::relations::RelationView> {
    let Some(entity) = self.entity_by_id(entity_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(issue) = entity.issue.clone().filter(|s| !s.trim().is_empty()) {
        out.push(crate::relations::RelationView::Issue { value: issue });
    }
    if let Some(pr) = entity.pr.clone().filter(|s| !s.trim().is_empty()) {
        out.push(crate::relations::RelationView::PullRequest { value: pr });
    }
    for stage in &self.definition().stages {
        if let Some(target) = &stage.feedback_to {
            if stage.name == entity.status || target == &entity.status {
                out.push(crate::relations::RelationView::FeedbackStage {
                    from: stage.name.clone(),
                    to: target.clone(),
                });
            }
        }
    }
    out
}

pub fn entity_details(&self, entity_id: &str) -> Option<crate::relations::EntityDetails> {
    let entity = self.entity_by_id(entity_id)?;
    Some(crate::relations::EntityDetails {
        id: entity.id.clone(),
        title: entity.title.clone(),
        status: entity.status.clone(),
        worktree: entity.worktree.clone(),
        relations: self.related(entity_id),
    })
}
```

Add index tests that prove:

- an entity with `issue` and `pr` returns both relations
- a stage with `feedback_to` returns a feedback relation for entities on either side of the feedback arc
- an entity without relations returns an empty `related(entity_id)` list and `Some(EntityDetails { relations: vec![], .. })`
- an unknown entity id returns an empty relation list and `entity_details(entity_id) == None`

- [ ] **Step 5: Export and verify**

In `lib.rs`, add:

```rust
pub mod relations;
```

Run: `cargo test -p spacetop-core relations::tests index::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/lib.rs crates/spacetop-core/src/index.rs crates/spacetop-core/src/relations.rs
git commit -m "feat(core): expose entity relationship facts"
```

---

## Task 2: Add search and command palette app state

**Files:**
- Create: `crates/spacetop/src/app/search.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/app/keys.rs`

- [ ] **Step 1: Write search state tests**

Create `crates/spacetop/src/app/search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_backspace_is_safe_on_empty_query() {
        let mut state = SearchState::new(SearchMode::Search);
        state.backspace();
        assert_eq!(state.query(), "");
    }

    #[test]
    fn command_palette_starts_with_empty_query() {
        let state = SearchState::new(SearchMode::Command);
        assert_eq!(state.mode(), SearchMode::Command);
        assert_eq!(state.query(), "");
    }

    #[test]
    fn typed_input_resets_selection_and_backspace_updates_query() {
        let mut state = SearchState::new(SearchMode::Command);
        state.push('m');
        state.select_next(2);
        state.push('e');
        assert_eq!(state.query(), "me");
        assert_eq!(state.selected_index(), 0);
        state.backspace();
        assert_eq!(state.query(), "m");
    }

    #[test]
    fn selection_moves_within_result_bounds() {
        let mut state = SearchState::new(SearchMode::Command);
        state.select_next(2);
        state.select_next(2);
        assert_eq!(state.selected_index(), 1);
        state.select_previous();
        assert_eq!(state.selected_index(), 0);
    }
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop app::search::tests`

Expected: FAIL because search state types do not exist.

- [ ] **Step 3: Implement search state**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Search,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchState {
    mode: SearchMode,
    query: String,
    selected_index: usize,
}

impl SearchState {
    pub fn new(mode: SearchMode) -> Self {
        Self {
            mode,
            query: String::new(),
            selected_index: 0,
        }
    }

    pub fn mode(&self) -> SearchMode {
        self.mode
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn push(&mut self, ch: char) {
        if !ch.is_control() {
            self.query.push(ch);
            self.selected_index = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected_index = 0;
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn select_next(&mut self, len: usize) {
        if len > 0 {
            self.selected_index = (self.selected_index + 1).min(len - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }
}
```

- [ ] **Step 4: Add app modes**

In `app.rs`, add `mod search;` and:

```rust
pub use search::{SearchMode, SearchState};
```

Extend `AppMode`:

```rust
Search {
    underlying: OverviewSession,
    state: SearchState,
},
```

Add `App::open_search(SearchMode)` and `App::close_overlay()` methods using the same underlying-session pattern as picker overlay and definition view.

Also add shared app-mode accessors before adding more P3 views:

```rust
impl AppMode {
    pub(crate) fn as_session(&self) -> &OverviewSession;
    pub(crate) fn as_session_mut(&mut self) -> &mut OverviewSession;
    pub(crate) fn into_session(self) -> OverviewSession;
}
```

Update reload, sync-status rendering, repo-root access, workflow rediscovery, and active-state helpers to use these accessors. Add regression tests for search mode plus the future timeline, metrics, activity, and relations modes as each mode lands. The tests must prove these views still preserve the underlying session state and can return to overview with `Esc`.

- [ ] **Step 5: Add key actions**

In `app/keys.rs`, add actions:

```rust
OpenSearch,
OpenCommandPalette,
OpenTimeline,
OpenMetrics,
OpenActivity,
OpenRelations,
```

Map `/`, `:`, `T`, `M`, `A`, and `R` when preview is closed.

For search and command mode, handle the overlay keys directly:

- printable characters append to the query
- Backspace edits the query
- Up/Down moves the selected result
- Enter activates the selected search row or command row
- Esc returns to the underlying overview session

For command mode, dispatch `"metrics"`, `"activity"`, `"timeline"`, and `"relations"` to the same actions as `M`, `A`, `T`, and `R`. `timeline` and `relations` are no-ops if no entity is selected.

- [ ] **Step 6: Verify and commit**

Run: `cargo test -p spacetop app::search::tests app::keys::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/app.rs crates/spacetop/src/app/keys.rs crates/spacetop/src/app/search.rs
git commit -m "feat(tui): add search and command palette app state"
```

---

## Task 3: Render search overlay from `query()`

**Files:**
- Create: `crates/spacetop/src/ui/search.rs`
- Modify: `crates/spacetop/src/ui/mod.rs`
- Modify: `crates/spacetop/src/ui/tests.rs`

- [ ] **Step 1: Write render test**

Add a `TestBackend` test that opens search with query `roadmap` and asserts the overlay contains:

```text
Search
roadmap
```

and a matching entity title from the fixture.

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop ui::tests::search_overlay`

Expected: FAIL because the renderer does not exist.

- [ ] **Step 3: Implement search renderer**

Create `ui/search.rs` with a modal-style overlay using existing popup patterns. It should:

- Render title `"Search"` for `SearchMode::Search`.
- Render title `"Command"` for `SearchMode::Command`.
- For search mode, call:

```rust
active_state.index().query(EntityQuery {
    scope: active_state.current_query_scope(),
    text: Some(state.query().to_string()),
    ..EntityQuery::default()
})
```

`current_query_scope()` must map the current overview scope to `QueryScope::Active` or `QueryScope::Archived`, so search follows the user's visible scope.
- For command mode, render command rows: `"metrics"`, `"activity"`, `"timeline"`, `"relations"`.
- Highlight `SearchState::selected_index()`.

- [ ] **Step 4: Wire renderer**

In `ui/mod.rs`, add `mod search;` and render the overlay after the main overview when `AppMode::Search` is active.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop ui::tests::search_overlay`

Expected: PASS.

```bash
git add crates/spacetop/src/ui/mod.rs crates/spacetop/src/ui/search.rs crates/spacetop/src/ui/tests.rs
git commit -m "feat(tui): render search and command palette overlay"
```

---

## Task 4: Add timeline view

**Files:**
- Create: `crates/spacetop/src/ui/timeline.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/ui/mod.rs`

- [ ] **Step 1: Write timeline unavailable render tests**

Add `TestBackend` tests for real P2 unavailable states. One test should use an index whose history result is `Err(HistoryUnavailable::ShallowClone)` and assert the view contains:

```text
Timeline
history unavailable: shallow clone
```

Add a pending-worker test with `HistoryUnavailable::Loading` and assert the view contains `"history is loading"`.

Add a separate test for a valid index with no events for the selected entity. It should render `"Timeline"` plus `"No timeline events"` rather than treating an empty entity timeline as a history-system failure.

- [ ] **Step 2: Write timeline event render test**

Add a test with an index containing two events. Assert visible rows include:

```text
plan
verify
```

- [ ] **Step 3: Implement app mode**

Extend `AppMode`:

```rust
Timeline {
    underlying: OverviewSession,
    entity_id: String,
    scroll: usize,
},
```

The `T` key captures the selected entity id. If no valid entity is selected, it is a no-op.

- [ ] **Step 4: Implement renderer**

Create `ui/timeline.rs`. It reads:

```rust
let result = session.active_state().index().timeline(entity_id);
```

Render unavailable via `HistoryUnavailable::user_message()`. Render empty timelines with a neutral `"No timeline events"` state. Render events ordered by time with columns `stage`, `from`, `commit`.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo test -p spacetop ui::tests::timeline_view app::keys::tests
```

Expected: PASS.

```bash
git add crates/spacetop/src/app.rs crates/spacetop/src/ui/mod.rs crates/spacetop/src/ui/timeline.rs crates/spacetop/src/ui/tests.rs
git commit -m "feat(tui): add entity timeline view"
```

---

## Task 5: Add metrics view

**Files:**
- Create: `crates/spacetop/src/ui/metrics.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/ui/mod.rs`

- [ ] **Step 1: Write metrics render tests**

Add tests for:

- unavailable metrics render `"Metrics"` and the unavailable message.
- populated metrics render `"completed"`, `"throughput"`, a stage dwell row, a cycle-time row, and WIP-by-stage rows.

- [ ] **Step 2: Implement app mode**

Extend `AppMode`:

```rust
Metrics {
    underlying: OverviewSession,
    scroll: usize,
},
```

The `M` key opens this mode.

- [ ] **Step 3: Implement renderer**

Create `ui/metrics.rs`. It calls:

```rust
session.active_state().index().metrics()
```

Render a dense table:

```text
Metrics
completed: N
throughput: N
stage dwell
plan    60s
verify  60s
cycle time
050     120s
WIP
verify  2
```

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop ui::tests::metrics_view app::keys::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/app.rs crates/spacetop/src/ui/mod.rs crates/spacetop/src/ui/metrics.rs crates/spacetop/src/ui/tests.rs
git commit -m "feat(tui): add metrics view"
```

---

## Task 6: Add activity feed

**Files:**
- Create: `crates/spacetop/src/ui/activity.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/ui/mod.rs`

- [ ] **Step 1: Write activity render tests**

Add tests for unavailable and populated activity feed. Populated feed should show newest event first.

- [ ] **Step 2: Implement app mode**

Extend `AppMode`:

```rust
Activity {
    underlying: OverviewSession,
    scroll: usize,
},
```

The `A` key opens this mode.

- [ ] **Step 3: Implement renderer**

Create `ui/activity.rs`. It calls:

```rust
session.active_state().index().activity(None)
```

Render rows with entity id, destination stage, and short commit prefix.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop ui::tests::activity_view app::keys::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/app.rs crates/spacetop/src/ui/mod.rs crates/spacetop/src/ui/activity.rs crates/spacetop/src/ui/tests.rs
git commit -m "feat(tui): add activity feed"
```

---

## Task 7: Add relations/details view

**Files:**
- Create: `crates/spacetop/src/ui/relations.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/ui/mod.rs`

- [ ] **Step 1: Write relation render test**

Add a test with an entity containing `issue`, `pr`, and a workflow stage with `feedback_to`. Assert the rendered view includes:

```text
Relations
issue
pr
feedback-to
```

- [ ] **Step 2: Implement app mode**

Extend `AppMode`:

```rust
Relations {
    underlying: OverviewSession,
    entity_id: String,
    scroll: usize,
},
```

The `R` key opens this mode for the selected entity.

- [ ] **Step 3: Implement renderer**

Create `ui/relations.rs`. It calls:

```rust
session.active_state().index().entity_details(entity_id)
```

Render:

- selected entity id/title/status from `EntityDetails`
- issue and PR values from `EntityDetails.relations`
- feedback stage rows from `EntityDetails.relations`
- worktree provenance from `EntityDetails.worktree`

If `entity_details(entity_id)` returns `None`, render `"Entity not found"` and keep the view read-only.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop ui::tests::relations_view app::keys::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/app.rs crates/spacetop/src/ui/mod.rs crates/spacetop/src/ui/relations.rs crates/spacetop/src/ui/tests.rs
git commit -m "feat(tui): add entity relations and details view"
```

---

## Task 8: Update footer/help and full verification

**Files:**
- Modify: `crates/spacetop/src/ui/footer.rs`
- Modify: `crates/spacetop/src/ui/help.rs`
- Modify: `README.md`

- [ ] **Step 1: Update footer hints**

When preview is closed, add compact hints:

```text
/: search
:: command
T/M/A/R: views
```

Keep the footer within existing narrow-width tests. If it overflows, prefer grouping over adding more pills.

- [ ] **Step 2: Update help popup**

Add rows for `/`, `:`, `T`, `M`, `A`, and `R`.

- [ ] **Step 3: Update README current product shape**

Add one short bullet describing the new read-only query-backed views.

- [ ] **Step 4: Run full verification**

Run:

```bash
cargo test --workspace
make lint
cargo test -p spacetop-core --test no_terminal_deps
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/spacetop/src/ui/footer.rs crates/spacetop/src/ui/help.rs README.md
git commit -m "docs: document P3 capability view keybindings"
```

## Definition of done (P3)

- [ ] Search overlay filters entities through `WorkflowIndex::query`.
- [ ] Command palette opens P3 views without workflow writes.
- [ ] Timeline view uses `WorkflowIndex::timeline`.
- [ ] Metrics view uses `WorkflowIndex::metrics`.
- [ ] Activity feed uses `WorkflowIndex::activity`.
- [ ] Relations/details view uses core `WorkflowIndex::entity_details`, which includes `WorkflowIndex::related` relation facts.
- [ ] Help/footer document the new bindings.
- [ ] `cargo test --workspace` passes.
- [ ] `make lint` passes.
