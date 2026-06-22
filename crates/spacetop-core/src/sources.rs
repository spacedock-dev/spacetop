use std::path::Path;

use crate::domain::{Entity, EntityParseError, WorkflowDefinition, WorkflowSnapshot};
use crate::parser::{
    load_archived_items_with_errors, load_workflow_dir, resolve_entity_dir, ParseError,
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

impl WorkflowSources {
    pub fn load_active(workflow_dir: &Path, repo_root: &Path) -> Result<Self, ParseError> {
        let active = WorkingTreeSource::load(workflow_dir, repo_root)?;
        Ok(Self {
            active,
            archive: ArchiveSnapshot::empty(),
        })
    }

    pub fn load_archive(workflow_dir: &Path, definition: &WorkflowDefinition) -> ArchiveSnapshot {
        let allowed_statuses = definition
            .stages
            .iter()
            .map(|stage| stage.name.clone())
            .collect::<Vec<_>>();
        // Archives live under the resolved entity dir. For single-root workflows
        // this equals `workflow_dir`; for split-root it is the state checkout.
        // `workflow_dir` (the discovered definition dir) and `definition.root`
        // name the same directory, so resolution is consistent with the active
        // scan in `load_workflow_dir`.
        let entity_dir = resolve_entity_dir(workflow_dir, definition.state.as_deref());
        ArchiveSource::load(
            &entity_dir,
            &allowed_statuses,
            definition.id_style.as_deref(),
        )
    }
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
                state: None,
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
        let error = ParseError::MissingFrontmatter {
            path: "README.md".to_string(),
        };
        let archive = ArchiveSnapshot::from_error(error);
        assert!(archive.entities.is_empty());
        assert!(archive.error.unwrap().contains("README.md"));
    }
}
