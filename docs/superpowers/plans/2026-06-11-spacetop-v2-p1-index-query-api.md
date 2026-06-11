# SpaceTop v2 - Phase P1: Index + Query API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce `WorkflowIndex` and a terminal-free query API over the existing working-tree, archived, and worktree sources, with no new user-facing features.

**Architecture:** This plan assumes P0 has landed and paths use the two-crate workspace (`crates/spacetop-core` and `crates/spacetop`). The core owns source loading, indexing, filtering, sorting, and graceful "history unavailable" API responses; the TUI keeps its existing screens but reads visible entities through the query API instead of iterating raw snapshot vectors directly. History, metrics values, activity, and new views remain unavailable or unchanged until later phases.

**Tech Stack:** Rust 2021, `spacetop-core`, `serde`, `serde_yaml`, existing parser/worktree/archive code, Ratatui `TestBackend` tests in the bin crate.

---

## Prerequisites

- P0 is merged or the executor is working in a branch where the workspace layout exists.
- `WorkItem` has already been renamed to `Entity`.
- `crates/spacetop-core/tests/no_terminal_deps.rs` passes before and after this phase.
- This phase must not add a terminal dependency to `spacetop-core`.

## Hard constraints

- **No new features.** The list, preview, archive toggle, sort, graph, sync, picker, and help behavior stay visibly the same.
- **Owned query results.** Query APIs return owned `Entity` values or stable ids, never `&Entity` or borrowed entity slices.
- **Full rebuild only.** No incremental `apply_change`; every reload builds a fresh `WorkflowIndex`.
- **Archive behavior unchanged.** The TUI still loads archived items lazily when archive scope is first opened, and archived rows keep the parser's completed-descending order. Core may provide archive-index helpers, but `OverviewState::load` must not read `_archive/`.
- **Archive parse errors scoped to archive view.** P1 may collect archived parse errors through the archive loader, but the TUI surfaces them only after archive scope has been opened.
- **Read-only contract unchanged.** Do not add workflow markdown writes. `Y` sync remains the only sanctioned write path.
- **Green checkpoints.** Run `cargo test --workspace` and `make lint` before marking the phase complete.

## File map

- Create: `crates/spacetop-core/src/query.rs`
- Create: `crates/spacetop-core/src/index.rs`
- Create: `crates/spacetop-core/src/sources.rs`
- Modify: `crates/spacetop-core/src/domain/mod.rs`
- Modify: `crates/spacetop-core/src/lib.rs`
- Modify: `crates/spacetop-core/src/parser/archive.rs`
- Modify: `crates/spacetop-core/src/parser.rs` if archive loader exports change
- Modify: `crates/spacetop/src/app/overview.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/ui/list.rs`
- Modify: `crates/spacetop/src/ui/preview.rs`
- Modify tests near the changed app/UI modules

---

## Task 0: Make core query DTOs serializable

**Files:**
- Modify: `crates/spacetop-core/src/domain/mod.rs`
- Test: `crates/spacetop-core/src/domain/mod.rs`

- [ ] **Step 1: Write serialization tests**

In the domain test module, add tests that prove every value returned by the P1 query API can be serialized:

```rust
#[test]
fn entity_serializes_for_headless_export() {
    let entity = Entity {
        path: PathBuf::from("001-test.md"),
        id: "001".to_string(),
        title: "Test".to_string(),
        status: "plan".to_string(),
        source: Some("captain".to_string()),
        started: None,
        completed: None,
        verdict: None,
        score: Some(1.0),
        worktree: None,
        issue: Some("https://example.test/issues/1".to_string()),
        pr: None,
        body: "body".to_string(),
        worktree_source: None,
        main_body: None,
    };
    let yaml = serde_yaml::to_string(&entity).expect("serialize entity");
    assert!(yaml.contains("id: '001'") || yaml.contains("id: 001"));
    assert!(yaml.contains("issue:"));
}

#[test]
fn workflow_definition_serializes_for_headless_export() {
    let definition = mk_definition(vec![mk_stage("plan")], Vec::new());
    let yaml = serde_yaml::to_string(&definition).expect("serialize definition");
    assert!(yaml.contains("stages:"));
    assert!(yaml.contains("plan"));
}
```

- [ ] **Step 2: Run and verify failure**

Run: `cargo test -p spacetop-core domain::tests::entity_serializes_for_headless_export domain::tests::workflow_definition_serializes_for_headless_export`

Expected: FAIL because the domain types do not yet derive `Serialize`.

- [ ] **Step 3: Add serde derives to exported domain types**

Add `serde::{Deserialize, Serialize}` derives to `Entity`, `WorkflowDefinition`, `StageDefinition`, `StageTransition`, `EntityParseError`, and the core-owned `Rgb` type from P0. If a nested field prevents deriving, make that nested value serializable in the same task rather than creating an export-only shadow type.

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo test -p spacetop-core domain::tests::entity_serializes_for_headless_export domain::tests::workflow_definition_serializes_for_headless_export
```

Expected: PASS.

```bash
git add crates/spacetop-core/src/domain/mod.rs
git commit -m "feat(core): make workflow query DTOs serializable"
```

---

## Task 1: Add query/result types in core

**Files:**
- Create: `crates/spacetop-core/src/query.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write query type tests**

Create `crates/spacetop-core/src/query.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_targets_active_entities_sorted_by_id() {
        let query = EntityQuery::default();
        assert_eq!(query.scope, QueryScope::Active);
        assert_eq!(query.sort, EntitySort::Id);
        assert!(query.status.is_none());
        assert!(query.text.is_none());
        assert!(query.field_filters.is_empty());
    }

    #[test]
    fn history_unavailable_has_stable_user_message() {
        assert_eq!(
            HistoryUnavailable::NotImplemented.user_message(),
            "history is not available until v2 P2"
        );
    }
}
```

- [ ] **Step 2: Run the new tests and verify they fail**

Run: `cargo test -p spacetop-core query::tests`

Expected: FAIL because the query types do not exist yet.

- [ ] **Step 3: Add the query API data types**

Add this implementation above the tests:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityQuery {
    pub scope: QueryScope,
    pub status: Option<String>,
    pub text: Option<String>,
    pub field_filters: Vec<FieldFilter>,
    pub sort: EntitySort,
}

impl Default for EntityQuery {
    fn default() -> Self {
        Self {
            scope: QueryScope::Active,
            status: None,
            text: None,
            field_filters: Vec::new(),
            sort: EntitySort::Id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryScope {
    Active,
    Archived,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntitySort {
    Id,
    Status,
    /// Preserve parser-provided archive order: completed timestamp descending,
    /// filename ascending as deterministic tiebreaker.
    ArchiveDefault,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldFilter {
    HasIssue,
    HasPr,
    HasWorktreeSource,
    Verdict(String),
    MinScore(f64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryUnavailable {
    NotImplemented,
    Loading,
    NotGitRepository,
    ShallowClone,
    GitError(String),
}

impl HistoryUnavailable {
    pub fn user_message(&self) -> &str {
        match self {
            Self::NotImplemented => "history is not available until v2 P2",
            Self::Loading => "history is loading",
            Self::NotGitRepository => "history unavailable: not a git repository",
            Self::ShallowClone => "history unavailable: shallow clone",
            Self::GitError(_) => "history unavailable: git log could not be read",
        }
    }
}

pub type HistoryResult<T> = Result<T, HistoryUnavailable>;
```

- [ ] **Step 4: Export the module**

In `crates/spacetop-core/src/lib.rs`, add:

```rust
pub mod query;
```

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop-core query::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/lib.rs crates/spacetop-core/src/query.rs
git commit -m "feat(core): add query API request and unavailable types"
```

---

## Task 2: Add source snapshots around existing parser functions

**Files:**
- Create: `crates/spacetop-core/src/sources.rs`
- Modify: `crates/spacetop-core/src/lib.rs`
- Modify: `crates/spacetop-core/src/parser/archive.rs`

- [ ] **Step 1: Write source wrapper tests**

Create `crates/spacetop-core/src/sources.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntityParseError, WorkflowSnapshot};
    use crate::parser::ParseError;
    use std::path::PathBuf;

    #[test]
    fn archive_snapshot_defaults_to_empty_success() {
        let archive = ArchiveSnapshot::empty();
        assert!(archive.entities.is_empty());
        assert!(archive.parse_errors.is_empty());
        assert!(archive.error.is_none());
    }

    #[test]
    fn workflow_sources_keeps_active_and_archive_separate() {
        let active = WorkflowSnapshot {
            definition: crate::domain::WorkflowDefinition {
                root: PathBuf::from("/tmp/workflow"),
                stages: Vec::new(),
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: Default::default(),
                stage_prose: Default::default(),
                transitions: Vec::new(),
            },
            items: Vec::new(),
            parse_errors: vec![EntityParseError {
                path: PathBuf::from("bad.md"),
                message: "bad frontmatter".to_string(),
                line: None,
                column: None,
            }],
        };
        let sources = WorkflowSources {
            active,
            archive: ArchiveSnapshot::empty(),
        };
        assert_eq!(sources.active.parse_errors.len(), 1);
        assert!(sources.archive.entities.is_empty());
    }

    #[test]
    fn archive_error_is_stored_as_string() {
        let error = ParseError::MissingReadme {
            path: "README.md".to_string(),
        };
        let archive = ArchiveSnapshot::from_error(error);
        assert!(archive.entities.is_empty());
        assert!(archive.error.unwrap().contains("README.md"));
    }
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test -p spacetop-core sources::tests`

Expected: FAIL because `ArchiveSnapshot` and `WorkflowSources` do not exist.

- [ ] **Step 3: Implement source wrapper structs**

Add above the tests:

```rust
use std::path::Path;

use crate::domain::{Entity, EntityParseError, WorkflowDefinition, WorkflowSnapshot};
use crate::parser::{
    load_archived_items, load_archived_items_with_errors, load_workflow_dir, ParseError,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveSnapshot {
    pub entities: Vec<Entity>,
    pub parse_errors: Vec<EntityParseError>,
    pub error: Option<String>,
}

impl ArchiveSnapshot {
    pub fn empty() -> Self {
        Self {
            entities: Vec::new(),
            parse_errors: Vec::new(),
            error: None,
        }
    }

    pub fn from_error(error: ParseError) -> Self {
        Self {
            entities: Vec::new(),
            parse_errors: Vec::new(),
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowSources {
    pub active: WorkflowSnapshot,
    pub archive: ArchiveSnapshot,
}

pub struct WorkingTreeSource;

impl WorkingTreeSource {
    pub fn load(workflow_dir: &Path, repo_root: &Path) -> Result<WorkflowSnapshot, ParseError> {
        load_workflow_dir(workflow_dir, repo_root)
    }
}

pub struct ArchiveSource;

impl ArchiveSource {
    pub fn load(
        workflow_dir: &Path,
        allowed_statuses: &[String],
        id_style: Option<&str>,
    ) -> ArchiveSnapshot {
        match load_archived_items_with_errors(workflow_dir, allowed_statuses, id_style) {
            Ok((entities, parse_errors)) => ArchiveSnapshot {
                entities,
                parse_errors,
                error: None,
            },
            Err(error) => ArchiveSnapshot::from_error(error),
        }
    }
}
```

- [ ] **Step 4: Export the module**

In `crates/spacetop-core/src/lib.rs`, add:

```rust
pub mod sources;
```

- [ ] **Step 5: Collect archived per-entity parse errors**

In `crates/spacetop-core/src/parser/archive.rs`, add `load_archived_items_with_errors(...) -> Result<(Vec<Entity>, Vec<EntityParseError>), ParseError>`. It should use the same item-path collection and sorting as `load_archived_items`, but when `parse_work_item` returns a per-entity parse failure, push `entity_parse_error_from(&item_path, &err)` instead of silently skipping it. Keep `load_archived_items(...)` as a compatibility wrapper returning only the entity vector so older call sites keep compiling during the migration.

Add a parser test with one valid archived entity and one malformed archived entity. Expected: `load_archived_items_with_errors` returns the valid entity plus one parse error; `load_archived_items` still returns the valid entity only.

- [ ] **Step 6: Verify and commit**

Run: `cargo test -p spacetop-core sources::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/lib.rs crates/spacetop-core/src/sources.rs
git commit -m "feat(core): wrap existing workflow sources for indexing"
```

---

## Task 3: Implement `WorkflowIndex`

**Files:**
- Create: `crates/spacetop-core/src/index.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write index query tests**

Create `crates/spacetop-core/src/index.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};
    use crate::query::{EntityQuery, EntitySort, QueryScope};
    use crate::sources::{ArchiveSnapshot, WorkflowSources};
    use std::path::PathBuf;

    fn entity(id: &str, title: &str, status: &str) -> Entity {
        Entity {
            path: PathBuf::from(format!("{id}.md")),
            id: id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            source: None,
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: None,
            issue: None,
            pr: None,
            body: format!("body for {title}"),
            worktree_source: None,
            main_body: None,
        }
    }

    fn definition() -> WorkflowDefinition {
        WorkflowDefinition {
            root: PathBuf::from("/tmp/workflow"),
            stages: vec![
                StageDefinition {
                    name: "plan".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                },
                StageDefinition {
                    name: "verify".to_string(),
                    initial: false,
                    terminal: false,
                    gate: true,
                    fresh: false,
                    feedback_to: Some("plan".to_string()),
                    worktree: false,
                    concurrency: None,
                },
            ],
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: Default::default(),
            stage_prose: Default::default(),
            transitions: Vec::new(),
        }
    }

    fn index() -> WorkflowIndex {
        let active = WorkflowSnapshot {
            definition: definition(),
            items: vec![
                entity("010", "Write query api", "plan"),
                entity("002", "Verify renderer", "verify"),
            ],
            parse_errors: Vec::new(),
        };
        WorkflowIndex::from_sources(WorkflowSources {
            active,
            archive: ArchiveSnapshot {
                entities: vec![entity("001", "Archived work", "verify")],
                parse_errors: Vec::new(),
                error: None,
            },
        })
    }

    #[test]
    fn query_returns_owned_active_entities_sorted_by_numeric_id() {
        let result = index().query(EntityQuery::default());
        let ids: Vec<String> = result.into_iter().map(|entity| entity.id).collect();
        assert_eq!(ids, ["002", "010"]);
    }

    #[test]
    fn query_filters_by_status_and_text() {
        let query = EntityQuery {
            scope: QueryScope::Active,
            status: Some("plan".to_string()),
            text: Some("query".to_string()),
            field_filters: Vec::new(),
            sort: EntitySort::Id,
        };
        let result = index().query(query);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "010");
    }

    #[test]
    fn archived_scope_queries_archive_entities() {
        let query = EntityQuery {
            scope: QueryScope::Archived,
            ..EntityQuery::default()
        };
        let result = index().query(query);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "001");
    }

    #[test]
    fn all_scope_queries_active_and_archived_entities() {
        let query = EntityQuery {
            scope: QueryScope::All,
            ..EntityQuery::default()
        };
        let result = index().query(query);
        let ids: Vec<String> = result.into_iter().map(|entity| entity.id).collect();
        assert_eq!(ids, ["001", "002", "010"]);
    }

    #[test]
    fn history_methods_are_unavailable_in_p1() {
        let index = index();
        assert!(index.timeline("010").is_err());
        assert!(index.metrics().is_err());
        assert!(index.activity(None).is_err());
    }
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test -p spacetop-core index::tests`

Expected: FAIL because `WorkflowIndex` does not exist.

- [ ] **Step 3: Implement index storage and filtering**

Add above the tests:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::{Entity, EntityParseError, WorkflowDefinition};
use crate::query::{
    EntityQuery, EntitySort, FieldFilter, HistoryResult, HistoryUnavailable, QueryScope,
};
use crate::sources::{ArchiveSnapshot, WorkflowSources};

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowIndex {
    definition: WorkflowDefinition,
    active: Vec<Entity>,
    archived: Vec<Entity>,
    active_parse_errors: Vec<EntityParseError>,
    archive_parse_errors: Vec<EntityParseError>,
    archive_error: Option<String>,
    by_id: HashMap<String, Entity>,
    by_slug: HashMap<String, Entity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageEvent {
    pub entity_id: String,
    pub from: Option<String>,
    pub to: String,
    pub at: CommitTime,
    pub commit: CommitId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CommitTime(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageCount {
    pub name: String,
    pub entities: usize,
}

/// P1 placeholder for the stable API surface. P2 replaces this with real
/// dwell/cycle/WIP/throughput fields before `metrics()` can return `Ok`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub entity_id: String,
    pub event: StageEvent,
}

impl WorkflowIndex {
    pub fn from_sources(sources: WorkflowSources) -> Self {
        let definition = sources.active.definition;
        let active = sources.active.items;
        let active_parse_errors = sources.active.parse_errors;
        let ArchiveSnapshot {
            entities: archived,
            parse_errors: archive_parse_errors,
            error: archive_error,
        } = sources.archive;

        let mut by_id = HashMap::new();
        let mut by_slug = HashMap::new();
        for entity in active.iter().chain(archived.iter()) {
            by_id.insert(entity.id.clone(), entity.clone());
            if let Some(slug) = slug_of(&entity.path) {
                by_slug.insert(slug, entity.clone());
            }
        }

        Self {
            definition,
            active,
            archived,
            active_parse_errors,
            archive_parse_errors,
            archive_error,
            by_id,
            by_slug,
        }
    }

    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }

    pub fn active_parse_errors(&self) -> &[EntityParseError] {
        &self.active_parse_errors
    }

    pub fn archive_parse_errors(&self) -> &[EntityParseError] {
        &self.archive_parse_errors
    }

    pub fn archive_error(&self) -> Option<&str> {
        self.archive_error.as_deref()
    }

    pub fn entity_by_id(&self, id: &str) -> Option<Entity> {
        self.by_id.get(id).cloned()
    }

    pub fn entity_by_slug(&self, slug: &str) -> Option<Entity> {
        self.by_slug.get(slug).cloned()
    }

    pub fn stage_counts(&self, archived_done_count: Option<usize>) -> Vec<StageCount> {
        self.definition
            .stages
            .iter()
            .map(|stage| {
                let mut entities = self
                    .active
                    .iter()
                    .filter(|entity| entity.status == stage.name)
                    .count();
                if stage.name == "done" {
                    entities += archived_done_count.unwrap_or(0);
                }
                StageCount {
                    name: stage.name.clone(),
                    entities,
                }
            })
            .collect()
    }

    pub fn query(&self, query: EntityQuery) -> Vec<Entity> {
        let mut source = Vec::new();
        match query.scope {
            QueryScope::Active => source.extend(self.active.iter()),
            QueryScope::Archived => source.extend(self.archived.iter()),
            QueryScope::All => source.extend(self.active.iter().chain(self.archived.iter())),
        }
        let needle = query.text.as_ref().map(|text| text.to_lowercase());
        let mut entities: Vec<Entity> = source
            .into_iter()
            .filter(|entity| {
                query
                    .status
                    .as_ref()
                    .is_none_or(|status| entity.status == *status)
            })
            .filter(|entity| query.field_filters.iter().all(|filter| matches_field(entity, filter)))
            .filter(|entity| {
                needle.as_ref().is_none_or(|needle| {
                    entity.id.to_lowercase().contains(needle)
                        || entity.title.to_lowercase().contains(needle)
                        || entity.body.to_lowercase().contains(needle)
                })
            })
            .cloned()
            .collect();

        match query.sort {
            EntitySort::Id => entities.sort_by(|a, b| compare_ids(&a.id, &b.id)),
            EntitySort::Status => {
                let stage_count = self.definition.stages.len();
                let stage_index = |status: &str| -> usize {
                    self.definition
                        .stages
                        .iter()
                        .position(|stage| stage.name == status)
                        .unwrap_or(stage_count)
                };
                entities.sort_by(|a, b| {
                    stage_index(&a.status)
                        .cmp(&stage_index(&b.status))
                        .then_with(|| compare_ids(&a.id, &b.id))
                });
            }
            EntitySort::ArchiveDefault => {}
        }

        entities
    }

    pub fn timeline(&self, _entity_id: &str) -> HistoryResult<Vec<StageEvent>> {
        Err(HistoryUnavailable::NotImplemented)
    }

    pub fn metrics(&self) -> HistoryResult<Metrics> {
        Err(HistoryUnavailable::NotImplemented)
    }

    pub fn activity(&self, _since: Option<CommitTime>) -> HistoryResult<Vec<ActivityEvent>> {
        Err(HistoryUnavailable::NotImplemented)
    }
}

fn matches_field(entity: &Entity, filter: &FieldFilter) -> bool {
    match filter {
        FieldFilter::HasIssue => entity.issue.as_ref().is_some_and(|value| !value.trim().is_empty()),
        FieldFilter::HasPr => entity.pr.as_ref().is_some_and(|value| !value.trim().is_empty()),
        FieldFilter::HasWorktreeSource => entity.worktree_source.is_some(),
        FieldFilter::Verdict(expected) => entity.verdict.as_ref() == Some(expected),
        FieldFilter::MinScore(minimum) => entity.score.is_some_and(|score| score >= *minimum),
    }
}

fn compare_ids(a: &str, b: &str) -> std::cmp::Ordering {
    let an = a.parse::<u64>().ok();
    let bn = b.parse::<u64>().ok();
    match (an, bn) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.cmp(b)),
        _ => a.cmp(b),
    }
}

fn slug_of(path: &Path) -> Option<String> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    match stem.as_deref() {
        Some("index") => path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned()),
        Some(_) => stem,
        None => None,
    }
}
```

- [ ] **Step 4: Export the module**

In `crates/spacetop-core/src/lib.rs`, add:

```rust
pub mod index;
```

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop-core index::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/lib.rs crates/spacetop-core/src/index.rs
git commit -m "feat(core): add workflow index and query filtering"
```

---

## Task 4: Build indexes from existing workflow sources

**Files:**
- Modify: `crates/spacetop-core/src/index.rs`
- Modify: `crates/spacetop-core/src/sources.rs`

- [ ] **Step 1: Add an index load integration test**

In `crates/spacetop-core/src/index.rs`, add this test in the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn load_from_paths_uses_existing_workflow_parser() {
        let workflow = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/slug-workflow");
        let repo_root = workflow.parent().expect("fixtures parent");
        let index = WorkflowIndex::load(&workflow, repo_root).expect("load index");
        let result = index.query(EntityQuery::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "roadmap-v5");
    }
```

- [ ] **Step 2: Run the test and verify it fails**

Run: `cargo test -p spacetop-core index::tests::load_from_paths_uses_existing_workflow_parser`

Expected: FAIL because `WorkflowIndex::load` does not exist.

- [ ] **Step 3: Add active-only and archive source loading**

In `crates/spacetop-core/src/sources.rs`, add:

```rust
impl WorkflowSources {
    pub fn load_active(workflow_dir: &Path, repo_root: &Path) -> Result<Self, ParseError> {
        let active = WorkingTreeSource::load(workflow_dir, repo_root)?;
        Ok(Self {
            active,
            archive: ArchiveSnapshot::empty(),
        })
    }

    pub fn load_archive(
        workflow_dir: &Path,
        definition: &WorkflowDefinition,
    ) -> ArchiveSnapshot {
        let allowed_statuses = definition
            .stages
            .iter()
            .map(|stage| stage.name.clone())
            .collect::<Vec<_>>();
        ArchiveSource::load(workflow_dir, &allowed_statuses, definition.id_style.as_deref())
    }
}
```

This keeps the startup path active-only while still giving archive-opening and headless export code a typed archive loader.

- [ ] **Step 4: Add `WorkflowIndex::load`**

In `crates/spacetop-core/src/index.rs`, add:

```rust
impl WorkflowIndex {
    pub fn load(workflow_dir: &Path, repo_root: &Path) -> Result<Self, crate::parser::ParseError> {
        WorkflowSources::load_active(workflow_dir, repo_root).map(Self::from_sources)
    }

    pub fn with_archive(mut self, archive: ArchiveSnapshot) -> Self {
        self.archive_parse_errors = archive.parse_errors;
        self.archive_error = archive.error;
        self.archived = archive.entities;
        self.rebuild_lookup_maps();
        self
    }
}
```

Extract the lookup-map population from `from_sources` into `rebuild_lookup_maps()` so `with_archive` updates `by_id` and `by_slug` after the lazy archive load.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop-core index::tests::load_from_paths_uses_existing_workflow_parser`

Expected: PASS.

- [ ] **Step 6: Add fixture-backed query golden tests**

Add tests under `crates/spacetop-core/src/index.rs` or `crates/spacetop-core/tests/query_golden.rs` that load fixture workflows through the parser and assert:

- active query order by id
- archived query order by completed timestamp using `EntitySort::ArchiveDefault`; this test must call `WorkflowSources::load_archive(...)` and `WorkflowIndex::with_archive(...)` explicitly
- text search over id/title/body
- field filters for issue, PR, worktree provenance, verdict, and minimum score
- archived parse errors are surfaced in `ArchiveSnapshot.parse_errors`
- history methods return `HistoryUnavailable::NotImplemented` in P1

Run: `cargo test -p spacetop-core query_golden index::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/index.rs crates/spacetop-core/src/sources.rs
git commit -m "feat(core): load workflow index from parser sources"
```

---

## Task 5: Migrate `OverviewState` to hold `WorkflowIndex`

**Files:**
- Modify: `crates/spacetop/src/app/overview.rs`
- Modify: `crates/spacetop/src/app.rs`

- [ ] **Step 1: Add an app reload test that preserves index-backed selection**

In `crates/spacetop/src/app/tests.rs`, add a test matching the existing reload-selection style:

```rust
#[test]
fn reload_from_index_preserves_selection_by_slug() {
    let root = PathBuf::from("/tmp/spacetop-index-test");
    let first = snapshot_with_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    let mut state = OverviewState::from_snapshot(root.clone(), first);
    state.select_next();
    assert_eq!(state.selected_item().expect("selected").id, "002");

    let second = snapshot_with_items(vec![
        item_at(root.join("002-second.md"), "002", "second changed", "plan"),
        item_at(root.join("003-third.md"), "003", "third", "plan"),
    ]);
    let index = spacetop_core::index::WorkflowIndex::from_sources(
        spacetop_core::sources::WorkflowSources {
            active: second,
            archive: spacetop_core::sources::ArchiveSnapshot::empty(),
        },
    );
    state.reload_from_index(index);

    assert_eq!(state.selected_item().expect("selected").id, "002");
}
```

If the helper names in `app/tests.rs` differ after P0, use the existing helper pattern but keep the test assertion exactly about `reload_from_index` preserving id `002`.

- [ ] **Step 2: Run the app test and verify it fails**

Run: `cargo test -p spacetop app::tests::reload_from_index_preserves_selection_by_slug`

Expected: FAIL because `reload_from_index` does not exist.

- [ ] **Step 3: Replace raw snapshot ownership with index ownership**

In `crates/spacetop/src/app/overview.rs`:

- Add imports:

```rust
use spacetop_core::index::WorkflowIndex;
use spacetop_core::query::{EntityQuery, EntitySort, QueryScope};
```

- Replace only the raw active snapshot and `sorted_active` storage with an index. Preserve path, refresh, preview, layout, archive-loaded, and sync fields:

```rust
pub workflow_dir: PathBuf,
pub repo_root: PathBuf,
pub index: WorkflowIndex,
pub selected_index: usize,
pub view_scope: ViewScope,
pub archive_loaded: bool,
pub archive_error: Option<String>,
pub archived_done_count: Option<usize>,
pub selected_index_archived: usize,
pub last_refresh_error: Option<String>,
pub preview_open: bool,
pub preview_scroll: usize,
pub max_preview_scroll: Cell<usize>,
pub preview_scroll_x: usize,
pub max_preview_scroll_x: Cell<usize>,
pub preview_viewport_height: Cell<usize>,
pub preview_wrap: bool,
pub task_page_size: Cell<usize>,
pub sort_mode: SortMode,
pub sync_status: Option<SyncStatus>,
```

Remove `snapshot`, `sorted_active`, and the raw `archived_items` vector only after the query-backed accessors below compile. Keep `archive_loaded`, `archive_error`, and `archived_done_count` so archive visibility and footer/graph counts stay behavior-identical.

- [ ] **Step 4: Keep compatibility accessors query-backed**

Update `OverviewState` methods to keep the existing public surface:

```rust
pub fn definition(&self) -> &spacetop_core::domain::WorkflowDefinition {
    self.index.definition()
}

pub fn index(&self) -> &WorkflowIndex {
    &self.index
}

pub fn visible_items(&self) -> Vec<spacetop_core::domain::Entity> {
    self.index.query(EntityQuery {
        scope: match self.view_scope {
            ViewScope::Active => QueryScope::Active,
            ViewScope::Archived => QueryScope::Archived,
        },
        status: None,
        text: None,
        field_filters: Vec::new(),
        sort: match self.view_scope {
            ViewScope::Archived => EntitySort::ArchiveDefault,
            ViewScope::Active => match self.sort_mode {
                SortMode::Id => EntitySort::Id,
                SortMode::Status => EntitySort::Status,
            },
        },
    })
}

pub fn archived_items(&self) -> Vec<spacetop_core::domain::Entity> {
    if !self.archive_loaded {
        return Vec::new();
    }
    self.index.query(EntityQuery {
        scope: QueryScope::Archived,
        status: None,
        text: None,
        field_filters: Vec::new(),
        sort: EntitySort::ArchiveDefault,
    })
}

pub fn stage_counts(&self) -> Vec<spacetop_core::index::StageCount> {
    self.index.stage_counts(self.archived_done_count)
}

pub fn parse_errors(&self) -> &[spacetop_core::domain::EntityParseError] {
    match self.view_scope {
        ViewScope::Active => self.index.active_parse_errors(),
        ViewScope::Archived if self.archive_loaded => self.index.archive_parse_errors(),
        ViewScope::Archived => &[],
    }
}
```

Delete the old `snapshot()` accessor after migrating its callers in Task 6. If a narrow test helper still needs a synthetic `WorkflowSnapshot`, build it in the test helper rather than reintroducing raw snapshot ownership on `OverviewState`.

- [ ] **Step 5: Implement reload from index**

Add:

```rust
pub fn reload_from_index(&mut self, index: WorkflowIndex) {
    let prior_slug = self
        .selected_item()
        .and_then(|entity| slug_of(&entity.path));

    self.index = index;

    let len = self.row_count();
    if len == 0 {
        self.set_scope_index(0);
    } else if let Some(slug) = prior_slug {
        let visible = self.visible_items();
        if let Some(pos) = visible
            .iter()
            .position(|entity| slug_of(&entity.path).as_deref() == Some(slug.as_str()))
        {
            self.set_scope_index(pos);
        } else if self.selected_index() >= len {
            self.set_scope_index(len - 1);
        }
    } else if self.selected_index() >= len {
        self.set_scope_index(len - 1);
    }

    self.reset_preview_scroll();
    self.last_refresh_error = None;
}
```

- [ ] **Step 6: Keep load/reload path active-only and add lazy archive load**

Change `OverviewState::load` and `OverviewState::reload` to call:

```rust
let index = WorkflowIndex::load(&workflow_dir, &repo_root)?;
```

Then `reload` delegates to `reload_from_index(index)`. This load path must not read `_archive/`.

Add:

```rust
pub fn ensure_archive_loaded(&mut self) {
    if self.archive_loaded {
        return;
    }
    let archive = spacetop_core::sources::WorkflowSources::load_archive(
        &self.workflow_dir,
        self.index.definition(),
    );
    self.archive_error = archive.error.clone();
    self.archived_done_count = Some(archive.entities.len());
    self.index = self.index.clone().with_archive(archive);
    self.archive_loaded = true;
}
```

Call `ensure_archive_loaded()` only from the existing archive-scope toggle path before switching to or rendering archived rows. Add app tests proving `OverviewState::load` leaves `archive_loaded == false`, and toggling archive scope loads archive parse errors into `state.parse_errors()`.

- [ ] **Step 7: Verify and commit**

Run: `cargo test -p spacetop app::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/app.rs crates/spacetop/src/app/overview.rs crates/spacetop/src/app/tests.rs
git commit -m "refactor(tui): back overview state with workflow index"
```

---

## Task 6: Migrate list and preview rendering to query-backed accessors

**Files:**
- Modify: `crates/spacetop/src/ui/list.rs`
- Modify: `crates/spacetop/src/ui/preview.rs`
- Modify: `crates/spacetop/src/ui/graph.rs` if it still reads `snapshot.items`
- Modify: `crates/spacetop/src/ui/tests/*`

- [ ] **Step 1: Run UI tests before editing**

Run: `cargo test -p spacetop ui::tests`

Expected: PASS before this task starts.

- [ ] **Step 2: Replace direct snapshot item access**

Search:

```bash
rg -n "snapshot\\(\\)\\.items|snapshot\\(\\)\\.definition|visible_items\\(\\)" crates/spacetop/src/ui crates/spacetop/src/app
```

For UI rendering:

- Use `state.visible_items()` for the active row set.
- Use `state.definition()` for stage metadata and colors.
- Use `state.stage_counts()` for graph/header counts. It must call `WorkflowIndex::stage_counts(...)` and preserve the archived-as-done rule.
- Use `state.parse_errors()` for synthetic broken rows.
- Avoid re-sorting in UI code. Sorting belongs in `WorkflowIndex::query`.

- [ ] **Step 3: Pin unchanged list rendering**

Run the existing list tests:

```bash
cargo test -p spacetop ui::tests::task_list
```

Expected: PASS. If snapshots or assertions fail only because helper code now clones owned entities, update helpers without changing visible strings.

- [ ] **Step 4: Pin unchanged graph/header counts**

Run:

```bash
cargo test -p spacetop ui::graph::tests app::tests::stage_counts
```

Expected: PASS. Done counts still include archived completed work only after archive counting has run, matching pre-P1 behavior.

- [ ] **Step 5: Pin unchanged preview rendering**

Run:

```bash
cargo test -p spacetop ui::tests::preview ui::tests::worktree
```

Expected: PASS. Worktree diff preview must still prefer `worktree_source` and `main_body`.

- [ ] **Step 6: Commit**

```bash
git add crates/spacetop/src/ui crates/spacetop/src/app
git commit -m "refactor(tui): render list and preview from query-backed state"
```

---

## Task 7: Prove watcher reload rebuilds the index

**Files:**
- Modify: `crates/spacetop/src/app/tests.rs`
- Modify: `crates/spacetop/tests/readme_reload.rs` if needed

- [ ] **Step 1: Add a reload test for index replacement**

Add a test near existing reload tests:

```rust
#[test]
fn reload_replaces_index_contents() {
    let root = PathBuf::from("/tmp/spacetop-index-reload-test");
    let first = snapshot_with_items(vec![item_at(
        root.join("001-first.md"),
        "001",
        "first",
        "plan",
    )]);
    let mut state = OverviewState::from_snapshot(root.clone(), first);
    assert_eq!(state.visible_items().len(), 1);

    let second = snapshot_with_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    let index = spacetop_core::index::WorkflowIndex::from_sources(
        spacetop_core::sources::WorkflowSources {
            active: second,
            archive: spacetop_core::sources::ArchiveSnapshot::empty(),
        },
    );
    state.reload_from_index(index);

    let ids: Vec<String> = state
        .visible_items()
        .iter()
        .map(|entity| entity.id.clone())
        .collect();
    assert_eq!(ids, ["001", "002"]);
}
```

- [ ] **Step 2: Run app reload tests**

Run: `cargo test -p spacetop app::tests::reload_replaces_index_contents`

Expected: PASS after Task 5.

- [ ] **Step 3: Run watcher/readme integration tests**

Run:

```bash
cargo test --workspace readme_reload
```

Expected: PASS. The watcher signal still calls app reload, and app reload now rebuilds the index.

- [ ] **Step 4: Commit**

```bash
git add crates/spacetop/src/app/tests.rs crates/spacetop/tests/readme_reload.rs
git commit -m "test: prove reload swaps workflow index contents"
```

---

## Task 8: Documentation and completion gates

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/development-policy.md`
- Modify: `README.md` if it mentions app data flow

- [ ] **Step 1: Update docs for the P1 boundary**

Add one sentence to the code map in `AGENTS.md` and `docs/development-policy.md`:

```markdown
`crates/spacetop-core/src/index.rs`, `query.rs`, and `sources.rs` own the v2 index/query spine; TUI code must consume `WorkflowIndex` through query methods instead of inferring schema rules from raw vectors.
```

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo test --workspace
make lint
cargo test -p spacetop-core --test no_terminal_deps
```

Expected: all PASS.

- [ ] **Step 3: Commit docs**

```bash
git add AGENTS.md docs/development-policy.md README.md
git commit -m "docs: document workflow index and query API boundary"
```

## Definition of done (P1)

- [ ] `WorkflowIndex` exists in `spacetop-core`.
- [ ] `query(EntityQuery)` returns owned `Entity` values.
- [ ] Core query DTOs are serde-serializable.
- [ ] `EntityQuery` supports status, text, and typed field filters.
- [ ] `timeline`, `metrics`, and `activity(since)` exist and return documented unavailable states.
- [ ] Fixture-backed golden tests cover active, archived, text, field-filter, parse-error, and unavailable-history paths.
- [ ] TUI list, preview, graph/header counts, and archive scope render from query-backed state.
- [ ] Archived row order and archive-loaded UI behavior remain unchanged.
- [ ] Archive parse errors are collected by the archive loader and surfaced only after archive scope is loaded.
- [ ] Watcher reloads rebuild the whole index.
- [ ] `cargo test --workspace` passes.
- [ ] `make lint` passes.
- [ ] `spacetop-core` still has no terminal dependencies.
