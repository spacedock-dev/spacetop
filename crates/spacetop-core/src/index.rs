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
