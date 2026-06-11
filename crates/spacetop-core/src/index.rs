use std::collections::HashMap;
use std::path::Path;

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
    pub items: usize,
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

        let mut index = Self {
            definition,
            active,
            archived,
            active_parse_errors,
            archive_parse_errors,
            archive_error,
            by_id: HashMap::new(),
            by_slug: HashMap::new(),
        };
        index.rebuild_lookup_maps();
        index
    }

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
                let mut items = self
                    .active
                    .iter()
                    .filter(|entity| entity.status == stage.name)
                    .count();
                if stage.name == "done" {
                    items += archived_done_count.unwrap_or(0);
                }
                StageCount {
                    name: stage.name.clone(),
                    items,
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
            .filter(|entity| {
                query
                    .field_filters
                    .iter()
                    .all(|filter| matches_field(entity, filter))
            })
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

    fn rebuild_lookup_maps(&mut self) {
        self.by_id.clear();
        self.by_slug.clear();
        for entity in self.archived.iter().chain(self.active.iter()) {
            self.by_id.insert(entity.id.clone(), entity.clone());
            if let Some(slug) = slug_of(&entity.path) {
                self.by_slug.insert(slug, entity.clone());
            }
        }
    }
}

fn matches_field(entity: &Entity, filter: &FieldFilter) -> bool {
    match filter {
        FieldFilter::HasIssue => entity
            .issue
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        FieldFilter::HasPr => entity
            .pr
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};
    use crate::query::{EntityQuery, EntitySort, FieldFilter, HistoryUnavailable, QueryScope};
    use crate::sources::{ArchiveSnapshot, WorkflowSources};
    use std::fs;
    use std::path::Path;
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

    #[test]
    fn fixture_active_query_sorts_by_numeric_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workflow = temp.path().join("docs/wf");
        write_workflow_readme(&workflow);
        write_entity(&workflow.join("010-query.md"), "010", "Query", "plan", "body");
        write_entity(&workflow.join("002-render.md"), "002", "Render", "review", "body");

        let index = WorkflowIndex::load(&workflow, temp.path()).expect("load index");
        let ids: Vec<String> = index
            .query(EntityQuery::default())
            .into_iter()
            .map(|entity| entity.id)
            .collect();

        assert_eq!(ids, ["002", "010"]);
    }

    #[test]
    fn fixture_archived_query_preserves_parser_archive_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workflow = temp.path().join("docs/wf");
        write_workflow_readme(&workflow);
        let archive = workflow.join("_archive");
        write_entity_with_extra(
            &archive.join("early.md"),
            "001",
            "Early",
            "done",
            "completed: 2026-04-24T14:49:53Z\n",
            "body",
        );
        write_entity_with_extra(
            &archive.join("late.md"),
            "002",
            "Late",
            "done",
            "completed: 2026-04-24T15:00:00Z\n",
            "body",
        );
        write_entity(&archive.join("unknown.md"), "003", "Unknown", "done", "body");

        let index = WorkflowIndex::load(&workflow, temp.path()).expect("load index");
        let archive = WorkflowSources::load_archive(&workflow, index.definition());
        let index = index.with_archive(archive);
        let titles: Vec<String> = index
            .query(EntityQuery {
                scope: QueryScope::Archived,
                sort: EntitySort::ArchiveDefault,
                ..EntityQuery::default()
            })
            .into_iter()
            .map(|entity| entity.title)
            .collect();

        assert_eq!(titles, ["Late", "Early", "Unknown"]);
    }

    #[test]
    fn fixture_text_query_matches_id_title_and_body() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workflow = temp.path().join("docs/wf");
        write_workflow_readme(&workflow);
        write_entity(
            &workflow.join("010-query.md"),
            "010",
            "Query API",
            "plan",
            "body",
        );
        write_entity(
            &workflow.join("020-render.md"),
            "020",
            "Renderer",
            "review",
            "mentions special needle",
        );

        let index = WorkflowIndex::load(&workflow, temp.path()).expect("load index");
        let query_ids = |text: &str| -> Vec<String> {
            index
                .query(EntityQuery {
                    text: Some(text.to_string()),
                    ..EntityQuery::default()
                })
                .into_iter()
                .map(|entity| entity.id)
                .collect()
        };

        assert_eq!(query_ids("010"), ["010"]);
        assert_eq!(query_ids("renderer"), ["020"]);
        assert_eq!(query_ids("needle"), ["020"]);
    }

    #[test]
    fn fixture_field_filters_cover_metadata_and_worktree_provenance() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workflow = temp.path().join("docs/wf");
        write_workflow_readme(&workflow);
        write_entity_with_extra(
            &workflow.join("001-issue.md"),
            "001",
            "Issue",
            "plan",
            "issue: https://example.test/issues/1\nscore: 0.7\n",
            "body",
        );
        write_entity_with_extra(
            &workflow.join("002-pr.md"),
            "002",
            "PR",
            "review",
            "pr: https://example.test/pulls/2\nverdict: pass\nscore: 0.95\n",
            "body",
        );
        let worktree_workflow = temp.path().join(".worktrees/wt/docs/wf");
        write_workflow_readme(&worktree_workflow);
        write_entity(
            &worktree_workflow.join("003-worktree.md"),
            "003",
            "Worktree",
            "plan",
            "body",
        );

        let index = WorkflowIndex::load(&workflow, temp.path()).expect("load index");
        let filtered_ids = |filter: FieldFilter| -> Vec<String> {
            index
                .query(EntityQuery {
                    field_filters: vec![filter],
                    ..EntityQuery::default()
                })
                .into_iter()
                .map(|entity| entity.id)
                .collect()
        };

        assert_eq!(filtered_ids(FieldFilter::HasIssue), ["001"]);
        assert_eq!(filtered_ids(FieldFilter::HasPr), ["002"]);
        assert_eq!(filtered_ids(FieldFilter::HasWorktreeSource), ["003"]);
        assert_eq!(filtered_ids(FieldFilter::Verdict("pass".to_string())), ["002"]);
        assert_eq!(filtered_ids(FieldFilter::MinScore(0.9)), ["002"]);
    }

    #[test]
    fn fixture_archive_snapshot_surfaces_parse_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workflow = temp.path().join("docs/wf");
        write_workflow_readme(&workflow);
        let archive = workflow.join("_archive");
        write_entity(&archive.join("001-good.md"), "001", "Good", "done", "body");
        write_markdown(
            &archive.join("002-broken.md"),
            "---\nid: [\n---\n\nbroken body\n",
        );

        let index = WorkflowIndex::load(&workflow, temp.path()).expect("load index");
        let archive = WorkflowSources::load_archive(&workflow, index.definition());

        assert_eq!(archive.entities.len(), 1);
        assert_eq!(archive.parse_errors.len(), 1);
        assert!(archive.parse_errors[0].message.contains("malformed YAML"));
    }

    #[test]
    fn fixture_history_methods_return_not_implemented_in_p1() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workflow = temp.path().join("docs/wf");
        write_workflow_readme(&workflow);
        write_entity(&workflow.join("001-query.md"), "001", "Query", "plan", "body");

        let index = WorkflowIndex::load(&workflow, temp.path()).expect("load index");

        assert_eq!(
            index.timeline("001"),
            Err(HistoryUnavailable::NotImplemented)
        );
        assert_eq!(index.metrics(), Err(HistoryUnavailable::NotImplemented));
        assert_eq!(
            index.activity(None),
            Err(HistoryUnavailable::NotImplemented)
        );
    }

    fn write_workflow_readme(workflow: &Path) {
        write_markdown(
            &workflow.join("README.md"),
            "---\ncommissioned-by: spacedock@0.20.0\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: review\n      gate: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
        );
    }

    fn write_entity(path: &Path, id: &str, title: &str, status: &str, body: &str) {
        write_entity_with_extra(path, id, title, status, "", body);
    }

    fn write_entity_with_extra(
        path: &Path,
        id: &str,
        title: &str,
        status: &str,
        extra_frontmatter: &str,
        body: &str,
    ) {
        write_markdown(
            path,
            &format!(
                "---\nid: \"{id}\"\ntitle: {title}\nstatus: {status}\n{extra_frontmatter}---\n\n{body}\n"
            ),
        );
    }

    fn write_markdown(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write markdown");
    }
}
