use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::domain::{StageDefinition, WorkflowDefinition};

use super::frontmatter::extract_frontmatter;
use super::{display_path, required, ParseError};

pub fn parse_workflow_readme(path: &Path) -> Result<WorkflowDefinition, ParseError> {
    let path_label = display_path(path);
    let contents = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path_label.clone(),
        source,
    })?;
    let (frontmatter, _) = extract_frontmatter(&contents, &path_label)?;
    let raw: RawWorkflowFrontmatter =
        serde_yaml::from_str(frontmatter).map_err(|source| ParseError::MalformedYaml {
            path: path_label.clone(),
            source,
        })?;

    let stage_block = raw.stages.ok_or(ParseError::MissingRequiredField {
        path: path_label,
        field: "stages",
    })?;
    let defaults = stage_block.defaults.unwrap_or_default();
    let mut stages = Vec::with_capacity(stage_block.states.len());
    for raw_stage in stage_block.states {
        let name = required(raw_stage.name, path, "stages.states.name")?;
        stages.push(StageDefinition {
            name,
            initial: raw_stage
                .initial
                .unwrap_or(defaults.initial.unwrap_or(false)),
            terminal: raw_stage
                .terminal
                .unwrap_or(defaults.terminal.unwrap_or(false)),
            gate: raw_stage.gate.unwrap_or(defaults.gate.unwrap_or(false)),
            fresh: raw_stage.fresh.unwrap_or(defaults.fresh.unwrap_or(false)),
            feedback_to: raw_stage.feedback_to.or(defaults.feedback_to.clone()),
            worktree: raw_stage
                .worktree
                .unwrap_or(defaults.worktree.unwrap_or(false)),
            concurrency: raw_stage.concurrency.or(defaults.concurrency),
        });
    }

    let stage_colors = crate::domain::assign_stage_colors(&stages);
    Ok(WorkflowDefinition {
        root: path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        stages,
        id_style: raw.id_style,
        entity_type: raw.entity_type,
        entity_label: raw.entity_label,
        entity_label_plural: raw.entity_label_plural,
        stage_colors,
    })
}

#[derive(Debug, Deserialize)]
struct RawWorkflowFrontmatter {
    #[serde(rename = "id-style")]
    id_style: Option<String>,
    #[serde(rename = "entity-type")]
    entity_type: Option<String>,
    #[serde(rename = "entity-label")]
    entity_label: Option<String>,
    #[serde(rename = "entity-label-plural")]
    entity_label_plural: Option<String>,
    stages: Option<RawStageBlock>,
}

#[derive(Debug, Deserialize)]
struct RawStageBlock {
    defaults: Option<RawStageDefaults>,
    states: Vec<RawStage>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawStageDefaults {
    initial: Option<bool>,
    terminal: Option<bool>,
    gate: Option<bool>,
    fresh: Option<bool>,
    #[serde(rename = "feedback-to")]
    feedback_to: Option<String>,
    worktree: Option<bool>,
    concurrency: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawStage {
    name: Option<String>,
    initial: Option<bool>,
    terminal: Option<bool>,
    gate: Option<bool>,
    fresh: Option<bool>,
    #[serde(rename = "feedback-to")]
    feedback_to: Option<String>,
    worktree: Option<bool>,
    concurrency: Option<u32>,
}
