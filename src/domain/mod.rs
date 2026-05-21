use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::style::Color;

const STAGE_LIGHTNESS: f32 = 0.78;
const STAGE_CHROMA: f32 = 0.12;

/// Convert an oklch color to an sRGB triple (r, g, b) each in [0, 255].
///
/// Pipeline: oklch → oklab → linear-sRGB → gamma-sRGB → u8.
/// No external crates required; the conversion is ~15 lines of pure Rust.
pub fn oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> (u8, u8, u8) {
    let h_rad = h_deg.to_radians();
    // oklch → oklab
    let a = c * h_rad.cos();
    let b = c * h_rad.sin();
    // oklab → linear-sRGB via the published 3×3 matrix
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    let r_lin = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g_lin = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let b_lin = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    // Apply sRGB gamma transfer function
    let gamma = |c: f32| -> f32 {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    let r = (gamma(r_lin) * 255.0).round() as u8;
    let g = (gamma(g_lin) * 255.0).round() as u8;
    let b = (gamma(b_lin) * 255.0).round() as u8;
    (r, g, b)
}

/// Assign a color to each stage using oklch-derived colors.
///
/// All stages share lightness=0.78 and chroma=0.12; hue varies evenly by
/// stage index across [0°, 360°). This produces perceptually uniform, muted
/// colors that are distinct regardless of stage name or order.
///
/// The returned `HashMap<String, Color>` maps stage name → assigned color.
pub fn assign_stage_colors(stages: &[StageDefinition]) -> HashMap<String, Color> {
    let n = stages.len();
    if n == 0 {
        return HashMap::new();
    }
    let step = 360.0 / n as f32;
    stages
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let hue = i as f32 * step;
            let (r, g, b) = oklch_to_srgb(STAGE_LIGHTNESS, STAGE_CHROMA, hue);
            (s.name.clone(), Color::Rgb(r, g, b))
        })
        .collect()
}

/// Map a stage name to a stable fallback color for archived/unknown stages
/// not found in the graph-aware color map. Derives a deterministic hue from
/// the stage name's bytes, then converts oklch (lightness=0.78, chroma=0.12)
/// to `Color::Rgb` — so the fallback path never emits named `Color::*` variants.
pub fn stage_color(stage_name: &str) -> Color {
    let hue = stable_stage_hue(stage_name);
    let (r, g, b) = oklch_to_srgb(STAGE_LIGHTNESS, STAGE_CHROMA, hue);
    Color::Rgb(r, g, b)
}

fn stable_stage_hue(stage_name: &str) -> f32 {
    // Hash the stage name bytes to a stable hue in [0°, 360°).
    let hash = stage_name.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    });
    (hash % 360) as f32
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    pub root: PathBuf,
    pub stages: Vec<StageDefinition>,
    pub id_style: Option<String>,
    pub entity_type: Option<String>,
    pub entity_label: Option<String>,
    pub entity_label_plural: Option<String>,
    /// Graph-aware color assignment: stage name → color.
    /// Populated at parse time by `assign_stage_colors`. Empty until populated.
    pub stage_colors: HashMap<String, Color>,
    /// Per-stage README prose extracted from the `### {stage}` blocks under
    /// the `## Stages` heading. Populated at parse time by
    /// `parse_stage_prose`. Empty for fixtures and synthetic definitions
    /// constructed in tests.
    pub stage_prose: HashMap<String, String>,
    /// Declared `stages.transitions` edges from the README frontmatter. Empty
    /// when the workflow omits a `transitions:` block — in that case the
    /// renderer should consult `effective_transitions()` which synthesises
    /// the implicit linear chain.
    pub transitions: Vec<StageTransition>,
}

impl WorkflowDefinition {
    /// Look up the graph-aware color for a stage by name (O(1) HashMap lookup).
    /// Falls back to the name-based `stage_color()` function when the stage
    /// is not found in the map (e.g. archived items from an older workflow version).
    pub fn stage_color_for(&self, stage_name: &str) -> Color {
        self.stage_colors
            .get(stage_name)
            .copied()
            .unwrap_or_else(|| stage_color(stage_name))
    }

    /// Returns the edge set the renderer should draw. When `transitions` is
    /// non-empty, returns a clone of the declared edges verbatim. When empty,
    /// synthesises the implicit linear chain from `stages` declaration order
    /// (`stages[0] → stages[1], stages[1] → stages[2], …`) so workflows
    /// without an explicit `transitions:` block keep rendering as today.
    pub fn effective_transitions(&self) -> Vec<StageTransition> {
        if !self.transitions.is_empty() {
            return self.transitions.clone();
        }
        if self.stages.len() < 2 {
            return Vec::new();
        }
        self.stages
            .windows(2)
            .map(|pair| StageTransition {
                from: pair[0].name.clone(),
                to: pair[1].name.clone(),
                label: None,
            })
            .collect()
    }
}

/// A declared edge in the workflow's `stages.transitions` block.
///
/// `from` and `to` are stage names that must (when well-formed) match entries
/// under `stages.states`. The renderer treats unmatched names as no-ops.
/// `label` is the optional `label:` on the transition row — typically a verb
/// describing the trigger (e.g. `reject`, `promote`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTransition {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
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
    /// Set to the worktree file path when this row is sourced from a worktree
    /// (worktree-only item) or has a divergent worktree copy that replaced the
    /// main body during merge. `None` for plain main-tracked rows.
    pub worktree_source: Option<PathBuf>,
    /// Original root body when a divergent worktree body replaced it. `None`
    /// when bodies match or there is no root copy (worktree-only item).
    pub main_body: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowSnapshot {
    pub definition: WorkflowDefinition,
    pub items: Vec<WorkItem>,
    /// Per-entity parse failures captured during a non-strict load. Empty on
    /// the happy path. The UI surfaces these as synthetic "broken" rows so a
    /// single malformed entity does not prevent the rest of the workflow from
    /// loading.
    pub parse_errors: Vec<EntityParseError>,
}

/// A per-entity parse failure recorded by `load_workflow_dir` when an entity's
/// frontmatter cannot be parsed. Used by the UI to render a synthetic "broken"
/// row and an error preview in place of a normal work item.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityParseError {
    /// File whose parse failed.
    pub path: PathBuf,
    /// `ParseError` Display string. Already contains the file path and the
    /// underlying reason (e.g., `<path>: malformed YAML frontmatter: mapping
    /// values are not allowed in this context at line 7 column 137`).
    pub message: String,
    /// Line number from `serde_yaml::Error::location()` when the underlying
    /// failure is a `MalformedYaml` variant. `None` otherwise.
    pub line: Option<u32>,
    /// Column number from `serde_yaml::Error::location()` when the underlying
    /// failure is a `MalformedYaml` variant. `None` otherwise.
    pub column: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklch_palette_produces_rgb_values() {
        // Five evenly-spaced hues around the oklch wheel (lightness=0.78, chroma=0.12).
        let hues = [0.0_f32, 72.0, 144.0, 216.0, 288.0];
        let mut results = Vec::new();
        for h in hues {
            let (r, g, b) = oklch_to_srgb(0.78, 0.12, h);
            results.push(Color::Rgb(r, g, b));
        }
        // All results must be distinct Color::Rgb values.
        let distinct: std::collections::HashSet<Color> = results.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            5,
            "five evenly-spaced oklch hues must produce 5 distinct Rgb values, got {:?}",
            results
        );
    }

    fn mk_stage(name: &str) -> StageDefinition {
        StageDefinition {
            name: name.to_string(),
            initial: false,
            terminal: false,
            gate: false,
            fresh: false,
            feedback_to: None,
            worktree: false,
            concurrency: None,
        }
    }

    fn mk_definition(stages: Vec<StageDefinition>, transitions: Vec<StageTransition>) -> WorkflowDefinition {
        WorkflowDefinition {
            root: PathBuf::new(),
            stages,
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: HashMap::new(),
            stage_prose: HashMap::new(),
            transitions,
        }
    }

    #[test]
    fn effective_transitions_returns_declared_set_when_present() {
        // 3 stages, 2 explicit transitions that skip the middle stage.
        let stages = vec![mk_stage("a"), mk_stage("b"), mk_stage("c")];
        let declared = vec![
            StageTransition {
                from: "a".into(),
                to: "c".into(),
                label: None,
            },
            StageTransition {
                from: "b".into(),
                to: "c".into(),
                label: Some("merge".into()),
            },
        ];
        let wf = mk_definition(stages, declared.clone());
        assert_eq!(wf.effective_transitions(), declared);
    }

    #[test]
    fn effective_transitions_synthesizes_linear_chain_when_absent() {
        let stages = vec![
            mk_stage("a"),
            mk_stage("b"),
            mk_stage("c"),
            mk_stage("d"),
            mk_stage("e"),
        ];
        let wf = mk_definition(stages, Vec::new());
        let out = wf.effective_transitions();
        let expected = vec![
            StageTransition { from: "a".into(), to: "b".into(), label: None },
            StageTransition { from: "b".into(), to: "c".into(), label: None },
            StageTransition { from: "c".into(), to: "d".into(), label: None },
            StageTransition { from: "d".into(), to: "e".into(), label: None },
        ];
        assert_eq!(out, expected);
    }

    #[test]
    fn effective_transitions_for_single_stage_is_empty() {
        let wf = mk_definition(vec![mk_stage("only")], Vec::new());
        assert!(wf.effective_transitions().is_empty());
    }

    #[test]
    fn effective_transitions_for_zero_stages_is_empty() {
        let wf = mk_definition(Vec::new(), Vec::new());
        assert!(wf.effective_transitions().is_empty());
    }

    #[test]
    fn assign_stage_colors_returns_rgb_for_all_stages() {
        let stages = vec![
            StageDefinition {
                name: "design".to_string(),
                initial: true,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: None,
            },
            StageDefinition {
                name: "plan".to_string(),
                initial: false,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: None,
            },
            StageDefinition {
                name: "implement".to_string(),
                initial: false,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: true,
                concurrency: None,
            },
            StageDefinition {
                name: "review".to_string(),
                initial: false,
                terminal: false,
                gate: true,
                fresh: false,
                feedback_to: Some("implement".to_string()),
                worktree: false,
                concurrency: None,
            },
            StageDefinition {
                name: "done".to_string(),
                initial: false,
                terminal: true,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: None,
            },
        ];
        let colors = assign_stage_colors(&stages);
        assert_eq!(colors.len(), 5);
        for (name, color) in &colors {
            assert!(
                matches!(color, Color::Rgb(_, _, _)),
                "stage {name} should have Color::Rgb color, got {color:?}"
            );
        }
    }
}
