use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::style::Color;

/// Ordered palette for graph-aware coloring. This fixed set is chosen to give
/// distinct colors for typical workflows; feedback edges can fan into a stage,
/// so the graph's maximum degree is not bounded by 3.
const GRAPH_PALETTE: &[Color] = &[
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

/// Return the preferred color for a well-known stage name, or `None` for
/// unknown stage names. Used as a hint by `assign_stage_colors`.
fn preferred_color(stage_name: &str) -> Option<Color> {
    match stage_name {
        "design" => Some(Color::Blue),
        "plan" => Some(Color::Cyan),
        "implement" => Some(Color::Yellow),
        "review" | "feedback" => Some(Color::Magenta),
        "done" | "complete" | "completed" | "shipped" => Some(Color::Green),
        "blocked" | "rejected" | "failed" => Some(Color::Red),
        _ => None,
    }
}

/// Assign a color to each stage using a graph-aware, palette-spreading pass.
///
/// Algorithm:
/// 1. Build an undirected adjacency set from linear edges (i → i+1) and
///    feedback edges (stage with `feedback_to` → named target, both directions).
/// 2. For each stage in definition order, pick the preferred color (from
///    `preferred_color`) if it does not conflict with any neighbor's color.
/// 3. For unknown names, start from a stage-specific palette offset so a
///    typical 5-stage linear workflow uses a broader set of colors instead
///    of collapsing to a 2-color alternation.
/// 4. When the primary palette is exhausted, cycle via deterministic hash
///    until a non-conflicting color is found.
///
/// The returned `HashMap<String, Color>` maps stage name → assigned color.
pub fn assign_stage_colors(stages: &[StageDefinition]) -> HashMap<String, Color> {
    let n = stages.len();
    if n == 0 {
        return HashMap::new();
    }

    // Build undirected adjacency: for each stage, the set of adjacent indices.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        // Linear forward edge: i → i+1
        if i + 1 < n {
            adj[i].push(i + 1);
            adj[i + 1].push(i);
        }
        // Feedback edge: stage[i].feedback_to → some stage j
        if let Some(target_name) = &stages[i].feedback_to {
            if let Some(j) = stages.iter().position(|s| &s.name == target_name) {
                if j != i {
                    if !adj[i].contains(&j) {
                        adj[i].push(j);
                    }
                    if !adj[j].contains(&i) {
                        adj[j].push(i);
                    }
                }
            }
        }
    }

    let mut assigned: Vec<Option<Color>> = vec![None; n];

    for i in 0..n {
        // Collect colors already used by neighbors.
        let neighbor_colors: std::collections::HashSet<Color> = adj[i]
            .iter()
            .filter_map(|&j| assigned[j])
            .collect();

        // Try preferred color first, then palette, then hash fallback.
        let chosen = pick_color(i, &stages[i].name, &neighbor_colors);
        assigned[i] = Some(chosen);
    }

    stages
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.clone(), assigned[i].unwrap_or(Color::White)))
        .collect()
}

/// Pick a non-conflicting color for stage `i` with name `stage_name`.
///
/// Priority:
/// 1. Preferred color if not in `neighbor_colors`.
/// 2. First non-conflicting palette entry, starting from a stage-specific
///    offset to spread sequential stages across the palette.
/// 3. Hash-based deterministic fallback cycling through extended colors until
///    a non-conflicting one is found (rare path for high-degree stages).
fn pick_color(
    stage_index: usize,
    stage_name: &str,
    neighbor_colors: &std::collections::HashSet<Color>,
) -> Color {
    // Try preferred first.
    if let Some(pref) = preferred_color(stage_name) {
        if !neighbor_colors.contains(&pref) {
            return pref;
        }
    }

    // Spread unknown stages across the palette instead of minimizing to a
    // tiny repeating set on simple linear workflows.
    if !GRAPH_PALETTE.is_empty() {
        let name_hash = stage_name
            .bytes()
            .fold(0usize, |a, b| a.wrapping_mul(33).wrapping_add(b as usize));
        let start = (stage_index + name_hash) % GRAPH_PALETTE.len();
        for offset in 0..GRAPH_PALETTE.len() {
            let candidate = GRAPH_PALETTE[(start + offset) % GRAPH_PALETTE.len()];
            if !neighbor_colors.contains(&candidate) {
                return candidate;
            }
        }
    }

    // Palette exhausted: hash-based cycle until we find a non-conflicting color.
    // Uses the same deterministic approach as `stage_color` fallback.
    const EXTENDED: &[Color] = &[
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
        Color::Gray,
        Color::DarkGray,
    ];
    let mut attempt = stage_index;
    loop {
        let candidate = EXTENDED[attempt % EXTENDED.len()];
        if !neighbor_colors.contains(&candidate) {
            return candidate;
        }
        attempt += 1;
    }
}

/// Map a stage name to a stable color. Recognises the conventional Spacedock
/// stage names; falls back to a deterministic palette index for anything else
/// so unknown workflows still get distinct colors per stage.
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
