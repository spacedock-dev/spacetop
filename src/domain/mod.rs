use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::style::Color;

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
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    let r_lin = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
    let g_lin = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
    let b_lin = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;
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
            let (r, g, b) = oklch_to_srgb(0.78, 0.12, hue);
            (s.name.clone(), Color::Rgb(r, g, b))
        })
        .collect()
}

/// Map a stage name to a stable fallback color for archived/unknown stages
/// not found in the graph-aware color map. Uses named terminal colors as a
/// lightweight fallback when oklch-derived sRGB is unavailable.
pub fn stage_color(stage_name: &str) -> Color {
    match stage_name {
        "design" => Color::Blue,
        "plan" => Color::Cyan,
        "implement" => Color::Yellow,
        "review" | "feedback" => Color::Magenta,
        "done" | "complete" | "completed" | "shipped" => Color::Green,
        "blocked" | "rejected" | "failed" => Color::Red,
        other => {
            // Deterministic fallback over an expanded palette for unknown stages.
            const PALETTE: &[Color] = &[
                Color::Blue,
                Color::Cyan,
                Color::Yellow,
                Color::Magenta,
                Color::Green,
                Color::LightBlue,
                Color::LightCyan,
                Color::LightYellow,
                Color::LightMagenta,
                Color::LightGreen,
                Color::Red,
                Color::LightRed,
                Color::White,
            ];
            let idx = other
                .bytes()
                .fold(0usize, |a, b| a.wrapping_mul(33).wrapping_add(b as usize))
                % PALETTE.len();
            PALETTE[idx]
        }
    }
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

    #[test]
    fn assign_stage_colors_returns_rgb_for_all_stages() {
        let stages = vec![
            StageDefinition { name: "design".to_string(), initial: true, terminal: false, gate: false, fresh: false, feedback_to: None, worktree: false, concurrency: None },
            StageDefinition { name: "plan".to_string(), initial: false, terminal: false, gate: false, fresh: false, feedback_to: None, worktree: false, concurrency: None },
            StageDefinition { name: "implement".to_string(), initial: false, terminal: false, gate: false, fresh: false, feedback_to: None, worktree: true, concurrency: None },
            StageDefinition { name: "review".to_string(), initial: false, terminal: false, gate: true, fresh: false, feedback_to: Some("implement".to_string()), worktree: false, concurrency: None },
            StageDefinition { name: "done".to_string(), initial: false, terminal: true, gate: false, fresh: false, feedback_to: None, worktree: false, concurrency: None },
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
