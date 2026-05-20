use super::*;

#[test]
fn stage_color_assigns_distinct_colors_for_known_stages() {
    let design = super::stage_color("design");
    let plan = super::stage_color("plan");
    let implement = super::stage_color("implement");
    let review = super::stage_color("review");
    let done = super::stage_color("done");
    let all = [design, plan, implement, review, done];
    // All returned colors must be Color::Rgb (no named-color variants).
    for c in &all {
        assert!(
            matches!(c, Color::Rgb(_, _, _)),
            "stage_color() must return Color::Rgb, got {c:?}"
        );
    }
    // All colors must be distinct.
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b, "stage colors should be distinct");
        }
    }
}
fn make_stage(name: &str, feedback_to: Option<&str>) -> crate::domain::StageDefinition {
    crate::domain::StageDefinition {
        name: name.to_string(),
        initial: false,
        terminal: false,
        gate: false,
        fresh: false,
        feedback_to: feedback_to.map(|s| s.to_string()),
        worktree: false,
        concurrency: None,
    }
}

#[test]
fn graph_coloring_no_adjacent_same_color() {
    // 4-stage workflow: alpha → beta → gamma → delta
    // with gamma feedback_to: alpha
    // Adjacent pairs: (0,1), (1,2), (2,3), (2,0) via feedback
    let stages = vec![
        make_stage("alpha", None),
        make_stage("beta", None),
        make_stage("gamma", Some("alpha")),
        make_stage("delta", None),
    ];
    let colors = super::assign_stage_colors(&stages);
    assert_eq!(colors.len(), 4);
    let c = |name: &str| *colors.get(name).unwrap();
    assert_ne!(c("alpha"), c("beta"), "alpha vs beta must differ");
    assert_ne!(c("beta"), c("gamma"), "beta vs gamma must differ");
    assert_ne!(c("gamma"), c("delta"), "gamma vs delta must differ");
    assert_ne!(
        c("gamma"),
        c("alpha"),
        "gamma vs alpha (feedback edge) must differ"
    );
}

#[test]
fn graph_coloring_linear_path_spreads_across_palette() {
    // For typical 5-stage workflows we prefer a richer palette than a
    // minimal 2-color alternation, while still keeping adjacent stages distinct.
    let stages = vec![
        make_stage("a", None),
        make_stage("b", None),
        make_stage("c", None),
        make_stage("d", None),
        make_stage("e", None),
    ];
    let colors = super::assign_stage_colors(&stages);
    let distinct: std::collections::HashSet<Color> = colors.values().copied().collect();
    assert!(
        distinct.len() >= 5,
        "5-stage linear workflow should use at least 5 colors, got {} distinct: {:?}",
        distinct.len(),
        distinct
    );
    // Adjacent constraint still holds.
    for i in 0..stages.len() - 1 {
        let ca = colors[&stages[i].name];
        let cb = colors[&stages[i + 1].name];
        assert_ne!(
            ca,
            cb,
            "adjacent stages {} and {} must differ",
            stages[i].name,
            stages[i + 1].name
        );
    }
}

#[test]
fn graph_coloring_produces_distinct_rgb_colors_for_standard_workflow() {
    // Standard spacetop-dev 5-stage workflow.
    // All stages should get distinct Color::Rgb values derived from oklch.
    let stages = vec![
        {
            let mut s = make_stage("design", None);
            s.initial = true;
            s
        },
        make_stage("plan", None),
        {
            let mut s = make_stage("implement", None);
            s.worktree = true;
            s
        },
        {
            let mut s = make_stage("review", Some("implement"));
            s.gate = true;
            s
        },
        {
            let mut s = make_stage("done", None);
            s.terminal = true;
            s
        },
    ];
    let colors = super::assign_stage_colors(&stages);
    // All 5 stage colors must be Color::Rgb variants (oklch-derived).
    for stage_name in &["design", "plan", "implement", "review", "done"] {
        let color = colors[*stage_name];
        assert!(
            matches!(color, Color::Rgb(_, _, _)),
            "stage {stage_name} color should be Color::Rgb, got {color:?}"
        );
    }
    // All 5 colors must be distinct.
    let distinct: std::collections::HashSet<Color> = colors.values().copied().collect();
    assert_eq!(
        distinct.len(),
        5,
        "5 stages should have 5 distinct colors, got {:?}",
        distinct
    );
}
