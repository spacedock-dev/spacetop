use std::path::PathBuf;

use ratatui::style::Color;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    pub root: PathBuf,
    pub stages: Vec<StageDefinition>,
    pub id_style: Option<String>,
    pub entity_type: Option<String>,
    pub entity_label: Option<String>,
    pub entity_label_plural: Option<String>,
    /// Graph-aware color assignment for each stage (indexed by stage position).
    /// Populated at parse time by `assign_stage_colors`. Empty until populated.
    pub stage_colors: Vec<Color>,
}

impl WorkflowDefinition {
    /// Look up the graph-aware color for a stage by name.
    /// Falls back to the name-based `stage_color()` function when the stage
    /// is not found in the current definition (e.g. archived items from an
    /// older workflow version).
    pub fn stage_color_for(&self, stage_name: &str) -> Color {
        self.stages
            .iter()
            .position(|s| s.name == stage_name)
            .and_then(|i| self.stage_colors.get(i).copied())
            .unwrap_or_else(|| crate::ui::stage_color(stage_name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDefinition {
    pub name: String,
    pub initial: bool,
    pub terminal: bool,
    pub gate: bool,
    pub fresh: bool,
    pub feedback_to: Option<String>,
    pub worktree: bool,
    pub concurrency: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkItem {
    pub path: PathBuf,
    pub id: String,
    pub title: String,
    pub status: String,
    pub source: Option<String>,
    pub started: Option<String>,
    pub completed: Option<String>,
    pub verdict: Option<String>,
    pub score: Option<f64>,
    pub worktree: Option<String>,
    pub issue: Option<String>,
    pub pr: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowSnapshot {
    pub definition: WorkflowDefinition,
    pub items: Vec<WorkItem>,
}
