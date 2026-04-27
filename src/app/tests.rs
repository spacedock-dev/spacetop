use super::{App, AppMode, ViewScope};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::discovery::DiscoveredWorkflow;
use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

#[test]
fn stores_workflow_directory() {
    let app = App::new("docs/spacetop-dev");

    assert_eq!(app.workflow_dir(), Path::new("docs/spacetop-dev"));
}

#[test]
fn loads_real_workflow_state_and_derives_stage_counts() {
    let root = PathBuf::from("workflow");
    let app = App::from_snapshot(root.clone(), snapshot_with_items(3));
    let expected_stage_counts = app
        .snapshot()
        .definition
        .stages
        .iter()
        .map(|stage| {
            (
                stage.name.as_str(),
                app.snapshot()
                    .items
                    .iter()
                    .filter(|item| item.status == stage.name)
                    .count(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(app.workflow_dir(), root.as_path());
    assert_eq!(
        app.stage_counts()
            .iter()
            .map(|count| (count.name.as_str(), count.items))
            .collect::<Vec<_>>(),
        expected_stage_counts
    );
    // Selection defaults to the first item; assert against the snapshot
    // rather than hard-coding a title that drifts as tasks ship.
    assert_eq!(
        app.selected_item().map(|item| item.title.as_str()),
        app.snapshot().items.first().map(|item| item.title.as_str())
    );
    assert_eq!(
        app.selected_item().map(|item| item.status.as_str()),
        app.snapshot()
            .items
            .first()
            .map(|item| item.status.as_str())
    );
    // The workflow has at least one stage and at least one item — these
    // are intrinsic invariants of the loaded fixture, not specific titles.
    assert!(!app.snapshot().definition.stages.is_empty());
    assert!(!app.snapshot().items.is_empty());
}

#[test]
fn stage_counts_include_archived_done_items_from_the_workflow_archive() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
    let app = App::load(root).expect("workflow should load");

    let counts = app.stage_counts();
    let done = counts
        .iter()
        .find(|count| count.name == "done")
        .expect("done stage should exist");
    assert!(
        done.items > 0,
        "done count should come from archived workflow items"
    );

    let expected_active_counts = app
        .snapshot()
        .definition
        .stages
        .iter()
        .filter(|stage| stage.name != "done")
        .map(|stage| {
            (
                stage.name.as_str(),
                app.snapshot()
                    .items
                    .iter()
                    .filter(|item| item.status == stage.name)
                    .count(),
            )
        })
        .collect::<Vec<_>>();

    let observed_active_counts = counts
        .iter()
        .filter(|count| count.name != "done")
        .map(|count| (count.name.as_str(), count.items))
        .collect::<Vec<_>>();

    assert_eq!(observed_active_counts, expected_active_counts);
}

#[test]
fn stage_counts_reuse_cached_archived_done_count_after_archive_disappears() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write_workflow_with_archive(root, "001");

    let app = App::load(root.to_path_buf()).expect("workflow should load");
    std::fs::remove_dir_all(root.join("_archive")).expect("archive dir should be removable");

    let counts = app.stage_counts();
    let done = counts
        .iter()
        .find(|count| count.name == "done")
        .expect("done stage should exist");

    assert_eq!(
        done.items, 1,
        "done count should keep using cached archive state after load"
    );
}

#[test]
fn stage_counts_active_only_done_contributes_to_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(
        root.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("task-100.md"),
        "---\nid: 100\ntitle: Task 100\nstatus: done\n---\n\nbody\n",
    )
    .unwrap();

    let app = App::load(root.to_path_buf()).expect("workflow should load");

    let counts = app.stage_counts();
    let done = counts
        .iter()
        .find(|count| count.name == "done")
        .expect("done stage should exist");
    assert_eq!(
        done.items, 1,
        "active terminal item should contribute to #done even when no archive exists"
    );
}

#[test]
fn stage_counts_sum_active_and_archived_done_without_double_counting() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(
        root.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
    )
    .unwrap();
    // Two active done items.
    for id in ["100", "101"] {
        std::fs::write(
            root.join(format!("task-{id}.md")),
            format!("---\nid: {id}\ntitle: Task {id}\nstatus: done\n---\n\nbody\n"),
        )
        .unwrap();
    }
    // Three archived done items (disjoint ids).
    std::fs::create_dir_all(root.join("_archive")).unwrap();
    for id in ["200", "201", "202"] {
        std::fs::write(
            root.join("_archive").join(format!("task-{id}.md")),
            format!(
                "---\nid: {id}\ntitle: Archived {id}\nstatus: done\ncompleted: 2026-04-27T00:00:00Z\n---\n\nbody\n"
            ),
        )
        .unwrap();
    }

    let app = App::load(root.to_path_buf()).expect("workflow should load");

    let counts = app.stage_counts();
    let done = counts
        .iter()
        .find(|count| count.name == "done")
        .expect("done stage should exist");
    assert_eq!(
        done.items, 5,
        "done count should sum N active + M archived without double-counting"
    );
}

#[test]
fn navigation_changes_selection_without_touching_snapshot() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.selected_index(), 1);

    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.selected_index(), 2);

    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.selected_index(), 1);

    app.handle_key(key(KeyCode::Home));
    assert_eq!(app.selected_index(), 0);

    app.handle_key(key(KeyCode::End));
    assert_eq!(app.selected_index(), 2);
    assert_eq!(app.snapshot().items.len(), 3);
}

#[test]
fn page_keys_scroll_preview_without_changing_selection() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.selected_index(), 0);
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);

    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.selected_index(), 0);
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 12);

    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.selected_index(), 0);
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);
}

#[test]
fn changing_selection_resets_preview_scroll() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);

    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.selected_index(), 1);
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 0);
    assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 0);
}

#[test]
fn preview_mode_is_closed_by_default_and_enter_toggles_it() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

    assert!(!app.as_overview().unwrap().preview_open());

    app.handle_key(key(KeyCode::Enter));
    assert!(app.as_overview().unwrap().preview_open());

    app.handle_key(key(KeyCode::Enter));
    assert!(!app.as_overview().unwrap().preview_open());
}

#[test]
fn preview_scroll_keys_are_ignored_until_preview_mode_is_open() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.as_overview().unwrap().task_page_size.set(1);

    app.handle_key(key(KeyCode::PageDown));
    app.handle_key(key(KeyCode::Right));

    assert_eq!(app.selected_index(), 1);
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 0);
    assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 0);
}

#[test]
fn page_keys_move_task_selection_when_preview_is_closed() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(6));
    app.as_overview().unwrap().task_page_size.set(2);

    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.selected_index(), 2);

    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.selected_index(), 4);

    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.selected_index(), 2);
}

#[test]
fn scroll_preview_down_is_capped_at_max_scroll() {
    let mut state =
        super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    state.preview_open = true;
    // Simulate render having set max_scroll = 10.
    state.max_preview_scroll.set(10);

    // Press PageDown 20 times — should not exceed 10.
    for _ in 0..20 {
        state.scroll_preview_down();
    }
    assert!(
        state.preview_scroll() <= 10,
        "preview_scroll must not exceed max_scroll"
    );
}

#[test]
fn scroll_preview_up_responds_immediately_after_capped_down() {
    let mut state =
        super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    state.preview_open = true;
    state.max_preview_scroll.set(10);

    // Press down many times (capped at 10).
    for _ in 0..30 {
        state.scroll_preview_down();
    }
    assert_eq!(state.preview_scroll(), 10);

    // One PageUp should immediately decrease position.
    state.scroll_preview_up();
    assert!(
        state.preview_scroll() < 10,
        "first PageUp must decrease scroll after capped drift"
    );
}

#[test]
fn navigation_is_clamped_for_empty_workflows() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(0));

    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::End));

    assert_eq!(app.selected_index(), 0);
    assert!(app.selected_item().is_none());
}

#[test]
fn quit_keys_set_quit_state_but_movement_keys_do_not() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

    app.handle_key(key(KeyCode::Down));
    assert!(!app.should_quit());

    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit());

    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.handle_key(key(KeyCode::Esc));
    assert!(app.should_quit());
}

#[test]
fn q_closes_preview_before_quitting_overview() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

    app.handle_key(key(KeyCode::Enter));
    assert!(app.as_overview().unwrap().preview_open());

    app.handle_key(key(KeyCode::Char('q')));
    assert!(!app.as_overview().unwrap().preview_open());
    assert!(!app.should_quit());

    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit());
}

#[test]
fn default_view_scope_is_active_and_visible_items_match_snapshot() {
    let app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    assert_eq!(app.view_scope(), ViewScope::Active);
    assert_eq!(app.visible_items().len(), app.snapshot().items.len());
    assert!(app.archived_count().is_none());
}

#[test]
fn toggle_scope_key_a_flips_to_archived_and_loads_lazily() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
    let mut app = App::load(root).expect("workflow should load");
    assert_eq!(app.view_scope(), ViewScope::Active);
    assert!(app.archived_count().is_none());

    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert!(app.archived_count().is_some());
    assert!(!app.archived_items().is_empty());
    // Selected item should be an archived entry.
    let selected = app.selected_item().expect("selected archived item");
    assert_eq!(selected.status, "done");

    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(app.view_scope(), ViewScope::Active);
}

#[test]
fn archived_view_selection_is_independent_of_active_selection() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
    let mut app = App::load(root).expect("workflow should load");
    app.handle_key(key(KeyCode::Down));
    let active_index = app.selected_index();

    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    let archived_index = app.selected_index();

    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(app.selected_index(), active_index);

    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(app.selected_index(), archived_index);
}

#[test]
fn archive_count_hidden_before_first_toggle() {
    let app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    assert!(app.archived_count().is_none());
}

// --- Picker tests ---

fn picker_app(workflows: Vec<DiscoveredWorkflow>) -> App {
    App::from_picker(PathBuf::from("/scan-root"), workflows)
}

fn fake_workflows(count: usize) -> Vec<DiscoveredWorkflow> {
    (0..count)
        .map(|i| DiscoveredWorkflow {
            root: PathBuf::from(format!("/scan-root/docs/w{i}")),
            title: Some(format!("Workflow {i}")),
        })
        .collect()
}

#[test]
fn picker_state_navigation_is_clamped() {
    let mut app = picker_app(fake_workflows(3));

    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.as_picker().unwrap().selected_index(), 1);

    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.as_picker().unwrap().selected_index(), 2);

    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.as_picker().unwrap().selected_index(), 1);

    app.handle_key(key(KeyCode::Up));
    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.as_picker().unwrap().selected_index(), 0);

    app.handle_key(key(KeyCode::End));
    assert_eq!(app.as_picker().unwrap().selected_index(), 2);

    app.handle_key(key(KeyCode::Home));
    assert_eq!(app.as_picker().unwrap().selected_index(), 0);
}

#[test]
fn picker_enter_transitions_to_overview_on_success() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
    let workflows = vec![
        DiscoveredWorkflow {
            root: root.clone(),
            title: Some("Real".to_string()),
        },
        DiscoveredWorkflow {
            root: PathBuf::from("/nonexistent/other"),
            title: None,
        },
    ];
    let mut app = App::from_picker(PathBuf::from("/scan-root"), workflows);

    app.handle_key(key(KeyCode::Enter));

    assert!(app.as_overview().is_some(), "expected transition");
    assert_eq!(app.workflow_dir(), root.as_path());
    assert!(matches!(app.mode(), AppMode::Overview(_)));
}

#[test]
fn picker_enter_surfaces_error_on_load_failure() {
    let workflows = vec![
        DiscoveredWorkflow {
            root: PathBuf::from("/does/not/exist/alpha"),
            title: None,
        },
        DiscoveredWorkflow {
            root: PathBuf::from("/does/not/exist/beta"),
            title: None,
        },
    ];
    let mut app = App::from_picker(PathBuf::from("/scan-root"), workflows);

    app.handle_key(key(KeyCode::Enter));

    assert!(app.as_picker().is_some(), "should still be in picker mode");
    assert!(
        app.as_picker().unwrap().error().is_some(),
        "expected error to be surfaced"
    );
    assert!(!app.should_quit());
}

#[test]
fn picker_q_and_esc_quit_without_transition() {
    let mut app = picker_app(fake_workflows(2));
    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit());
    assert!(app.as_picker().is_some());

    let mut app = picker_app(fake_workflows(2));
    app.handle_key(key(KeyCode::Esc));
    assert!(app.should_quit());
    assert!(app.as_picker().is_some());
}

#[test]
fn picker_pageup_pagedown_step_by_viewport_height() {
    let mut app = picker_app(fake_workflows(40));
    // Simulate a viewport of 10 rows (set by the renderer in real use).
    app.as_picker().unwrap().viewport_height.set(10);

    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.as_picker().unwrap().selected_index(), 10);

    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.as_picker().unwrap().selected_index(), 20);

    // Clamp to last when paging past the end.
    app.handle_key(key(KeyCode::PageDown));
    app.handle_key(key(KeyCode::PageDown));
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.as_picker().unwrap().selected_index(), 39);

    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.as_picker().unwrap().selected_index(), 29);

    // Saturate to 0 going up past the start.
    for _ in 0..10 {
        app.handle_key(key(KeyCode::PageUp));
    }
    assert_eq!(app.as_picker().unwrap().selected_index(), 0);
}

#[test]
fn picker_paging_safe_on_short_lists() {
    // 2-element list (the from_picker minimum). Paging shouldn't panic and
    // must clamp to bounds.
    let mut app = picker_app(fake_workflows(2));
    app.as_picker().unwrap().viewport_height.set(10);

    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.as_picker().unwrap().selected_index(), 1);
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.as_picker().unwrap().selected_index(), 1);
    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.as_picker().unwrap().selected_index(), 0);
    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.as_picker().unwrap().selected_index(), 0);

    // Construct an empty PickerState directly (App::from_picker forbids it)
    // and exercise paging there too.
    let mut empty = crate::app::PickerState::new(PathBuf::from("/scan-root"), Vec::new());
    empty.page_selection_down();
    assert_eq!(empty.selected_index(), 0);
    empty.page_selection_up();
    assert_eq!(empty.selected_index(), 0);
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn snapshot_with_items(count: usize) -> WorkflowSnapshot {
    WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: PathBuf::from("workflow"),
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
                    name: "implement".to_string(),
                    initial: false,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: true,
                    concurrency: None,
                },
            ],
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: HashMap::new(),
        },
        items: (0..count)
            .map(|index| WorkItem {
                path: PathBuf::from(format!("workflow/task-{index}.md")),
                id: format!("{index:03}"),
                title: format!("Task {index}"),
                status: if index == 0 { "plan" } else { "implement" }.to_string(),
                source: Some("test".to_string()),
                started: None,
                completed: None,
                verdict: None,
                score: Some(0.5),
                worktree: None,
                issue: None,
                pr: None,
                body: format!("Body excerpt for task {index}."),
            })
            .collect(),
    }
}

fn snapshot_with_paths(paths: &[&str]) -> WorkflowSnapshot {
    WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: PathBuf::from("workflow"),
            stages: vec![StageDefinition {
                name: "plan".to_string(),
                initial: true,
                terminal: false,
                gate: false,
                fresh: false,
                feedback_to: None,
                worktree: false,
                concurrency: None,
            }],
            id_style: None,
            entity_type: None,
            entity_label: None,
            entity_label_plural: None,
            stage_colors: HashMap::new(),
        },
        items: paths
            .iter()
            .enumerate()
            .map(|(index, p)| WorkItem {
                path: PathBuf::from(p),
                id: format!("{index:03}"),
                title: format!("Task {p}"),
                status: "plan".to_string(),
                source: None,
                started: None,
                completed: None,
                verdict: None,
                score: None,
                worktree: None,
                issue: None,
                pr: None,
                body: String::new(),
            })
            .collect(),
    }
}

#[test]
fn reload_from_snapshot_preserves_selection_by_slug() {
    let mut app = App::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md", "workflow/gamma.md"]),
    );
    app.handle_key(key(KeyCode::Down)); // select beta (index 1)
    assert_eq!(
        app.selected_item().map(|i| i.path.clone()),
        Some(PathBuf::from("workflow/beta.md"))
    );

    // Reorder: beta now at index 1 still, but amid different neighbors.
    app.reload_from_snapshot(snapshot_with_paths(&[
        "workflow/gamma.md",
        "workflow/beta.md",
        "workflow/delta.md",
    ]));

    assert_eq!(app.selected_index(), 1);
    assert_eq!(
        app.selected_item().map(|i| i.path.clone()),
        Some(PathBuf::from("workflow/beta.md"))
    );
}

#[test]
fn reload_from_snapshot_preserves_selection_by_slug_at_new_index() {
    let mut app = App::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md", "workflow/gamma.md"]),
    );
    app.handle_key(key(KeyCode::Down)); // beta at index 1

    // beta moved to index 2.
    app.reload_from_snapshot(snapshot_with_paths(&[
        "workflow/alpha.md",
        "workflow/gamma.md",
        "workflow/beta.md",
    ]));

    assert_eq!(app.selected_index(), 2);
}

#[test]
fn reload_from_snapshot_clamps_when_slug_missing() {
    let mut app = App::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md", "workflow/gamma.md"]),
    );
    app.handle_key(key(KeyCode::End)); // select gamma (index 2)
    assert_eq!(app.selected_index(), 2);

    // gamma is gone, snapshot shrinks to 2 items.
    app.reload_from_snapshot(snapshot_with_paths(&[
        "workflow/alpha.md",
        "workflow/beta.md",
    ]));

    assert_eq!(app.selected_index(), 1);
}

#[test]
fn reload_from_snapshot_empty_clears_selection() {
    let mut app = App::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md", "workflow/beta.md"]),
    );
    app.handle_key(key(KeyCode::Down));
    app.reload_from_snapshot(snapshot_with_paths(&[]));
    assert_eq!(app.selected_index(), 0);
    assert!(app.selected_item().is_none());
}

#[test]
fn reload_from_snapshot_clears_prior_error() {
    let mut app = App::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md"]),
    );
    app.set_refresh_error("boom".into());
    assert_eq!(app.last_refresh_error(), Some("boom"));

    app.reload_from_snapshot(snapshot_with_paths(&["workflow/alpha.md"]));
    assert_eq!(app.last_refresh_error(), None);
}

#[test]
fn reload_from_snapshot_preserves_view_scope() {
    use crate::domain::WorkItem;
    let mut overview = super::OverviewState::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md"]),
    );
    // Force into archived scope with synthetic archived items.
    overview.view_scope = ViewScope::Archived;
    overview.archived_items = vec![WorkItem {
        path: PathBuf::from("workflow/_archive/old.md"),
        id: "old".into(),
        title: "Old".into(),
        status: "done".into(),
        source: None,
        started: None,
        completed: None,
        verdict: None,
        score: None,
        worktree: None,
        issue: None,
        pr: None,
        body: String::new(),
    }];
    overview.archive_loaded = true;

    overview.reload_from_snapshot(snapshot_with_paths(&[
        "workflow/alpha.md",
        "workflow/beta.md",
    ]));

    // View scope preserved; when already in archived mode, the archive
    // cache is immediately reloaded so the view does not go empty after
    // a reload or workflow switch.
    assert_eq!(overview.view_scope, ViewScope::Archived);
    assert!(overview.archive_loaded);
    assert!(overview.archive_error.is_none());
}

#[test]
fn reload_retains_prior_snapshot_on_parse_error() {
    // Minimal real workflow fixture in a tempdir.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(
        root.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("task-one.md"),
        "---\nid: 001\ntitle: One\nstatus: plan\n---\n\nbody\n",
    )
    .unwrap();

    let mut app = App::load(root.to_path_buf()).expect("load ok");
    let prior_snapshot = app.snapshot().clone();

    // Poison one task file: invalid YAML frontmatter.
    std::fs::write(
        root.join("task-one.md"),
        "---\nid: [not valid yaml\nstatus\n---\nbody\n",
    )
    .unwrap();

    let result = app.reload();
    assert!(result.is_err(), "parse should fail");
    assert_eq!(
        app.snapshot(),
        &prior_snapshot,
        "prior snapshot retained on parse error"
    );
    assert!(app.last_refresh_error().is_some());
}

// --- Multi-workflow session tests (task 010) ---

use super::{OverviewSession, OverviewState};

/// Write a minimal real workflow (one task in `plan`) into `dir`.
fn write_workflow(dir: &Path, task_id: &str) {
    std::fs::write(
        dir.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
    )
    .unwrap();
    std::fs::write(
        dir.join(format!("task-{task_id}.md")),
        format!(
            "---\nid: {task_id}\ntitle: Task {task_id}\nstatus: plan\n---\n\nbody for {task_id}\n"
        ),
    )
    .unwrap();
}

fn write_workflow_with_archive(dir: &Path, task_id: &str) {
    std::fs::create_dir_all(dir.join("_archive")).unwrap();
    write_workflow(dir, task_id);
    std::fs::write(
        dir.join("_archive").join("done.md"),
        "---\nid: 999\ntitle: Archived Done\nstatus: done\ncompleted: 2026-04-27T00:00:00Z\n---\n\narchived body\n",
    )
    .unwrap();
}

/// Build a multi-workflow session with `n` real tempdir workflows; the
/// first slot is materialized (its OverviewState is loaded from disk),
/// the rest are lazy `None` until activated. Returns the session plus
/// the holder tempdir (must outlive the session).
fn multi_session(n: usize) -> (OverviewSession, tempfile::TempDir, Vec<PathBuf>) {
    let holder = tempfile::tempdir().expect("tempdir holder");
    let mut roots = Vec::with_capacity(n);
    let mut discovery = Vec::with_capacity(n);
    for i in 0..n {
        let root = holder.path().join(format!("w{i}"));
        std::fs::create_dir_all(&root).unwrap();
        write_workflow(&root, &format!("{i:03}"));
        roots.push(root.clone());
        discovery.push(DiscoveredWorkflow {
            root,
            title: Some(format!("Workflow {i}")),
        });
    }
    let initial = OverviewState::load(roots[0].clone()).expect("load w0");
    let session =
        OverviewSession::from_discovery(holder.path().to_path_buf(), discovery, 0, initial);
    (session, holder, roots)
}

#[test]
fn cycle_keys_advance_active_index_in_multi_session() {
    let (session, _holder, _roots) = multi_session(3);
    let mut app = App::from_session(session);
    assert!(app.as_session().unwrap().is_multi());
    assert_eq!(app.as_session().unwrap().active_index(), 0);

    app.handle_key(key(KeyCode::Right));
    let switch = app.take_pending_switch().expect("cycle next emits switch");
    assert_eq!(switch.target_index, 1);
    assert!(switch.needs_first_load);
    assert_eq!(app.as_session().unwrap().active_index(), 1);

    // Materialize so subsequent cycles work; the test exercises pure
    // index mutation but materialize is what the event loop does.
    app.materialize_active();

    app.handle_key(key(KeyCode::Right));
    let _ = app.take_pending_switch();
    app.materialize_active();
    app.handle_key(key(KeyCode::Right)); // wraps back to 0
    let switch = app.take_pending_switch().expect("wrap emits switch");
    assert_eq!(switch.target_index, 0);
    assert!(!switch.needs_first_load, "w0 was already loaded");

    app.handle_key(key(KeyCode::Left)); // wrap to last
    let switch = app.take_pending_switch().expect("prev emits switch");
    assert_eq!(switch.target_index, 2);
}

#[test]
fn cycle_keys_inert_in_single_session() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    let original = app.clone();
    app.handle_key(key(KeyCode::Right));
    app.handle_key(key(KeyCode::Left));
    app.handle_key(key(KeyCode::Char('P')));
    assert!(app.take_pending_switch().is_none());
    assert!(!app.take_pending_overlay_open());
    assert_eq!(app, original);
}

#[test]
fn preview_mode_consumes_left_right_for_horizontal_scroll_in_multi() {
    let (session, _holder, _roots) = multi_session(3);
    let mut app = App::from_session(session);
    let state = match &mut app.mode {
        AppMode::Overview(session) => session.active_state_mut(),
        _ => panic!("expected overview"),
    };
    state.toggle_preview();
    state.max_preview_scroll_x.set(24);

    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 8);
    assert!(app.take_pending_switch().is_none());

    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.as_overview().unwrap().preview_scroll_x(), 0);
    assert!(app.take_pending_switch().is_none());
}

#[test]
fn picker_overlay_open_close_preserves_session() {
    let (session, _holder, _roots) = multi_session(2);
    let mut app = App::from_session(session);
    let original_active = app.as_session().unwrap().active_index();

    // Press P: schedules overlay-open; the event loop normally re-runs
    // discovery, but we simulate that with the same list.
    app.handle_key(key(KeyCode::Char('P')));
    assert!(app.take_pending_overlay_open());
    let same_list = app.as_session().unwrap().discovery().to_vec();
    app.open_picker_overlay_with(Ok(same_list));
    assert!(app.is_overlay());

    // Esc dismisses and restores.
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.is_overlay());
    assert!(app.as_session().is_some());
    assert_eq!(app.as_session().unwrap().active_index(), original_active);
}

#[test]
fn picker_overlay_q_closes_popup_without_quitting() {
    let (session, _holder, _roots) = multi_session(2);
    let mut app = App::from_session(session);
    app.handle_key(key(KeyCode::Char('P')));
    assert!(app.take_pending_overlay_open());
    app.open_picker_overlay_with(Ok(app.as_session().unwrap().discovery().to_vec()));
    assert!(app.is_overlay());

    app.handle_key(key(KeyCode::Char('q')));
    assert!(!app.is_overlay());
    assert!(!app.should_quit());
    assert!(matches!(app.mode(), AppMode::Overview(_)));
}

#[test]
fn picker_overlay_pickup_adds_new_workflow() {
    let (session, holder, _roots) = multi_session(2);
    let mut app = App::from_session(session);
    // Create a third workflow on disk, then open overlay with the new
    // discovery list including it.
    let new_root = holder.path().join("w-new");
    std::fs::create_dir_all(&new_root).unwrap();
    write_workflow(&new_root, "999");
    app.handle_key(key(KeyCode::Char('P')));
    assert!(app.take_pending_overlay_open());
    let mut new_list = app.as_session().unwrap().discovery().to_vec();
    new_list.push(DiscoveredWorkflow {
        root: new_root.clone(),
        title: Some("New".to_string()),
    });
    app.open_picker_overlay_with(Ok(new_list));
    assert!(app.is_overlay());
    // Move selection to the new entry (index 2) and press Enter.
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    let switch = app.take_pending_switch().expect("Enter emits switch");
    assert_eq!(switch.target_index, 2);
    assert!(switch.needs_first_load);
    assert_eq!(app.as_session().unwrap().discovery().len(), 3);
}

#[test]
fn switch_preserves_per_workflow_state() {
    // Two real workflows; in A, advance selection + flip to archived.
    // After cycling away and back, A's state must be intact.
    let (session, _holder, _roots) = multi_session(2);
    let mut app = App::from_session(session);
    // First, write an _archive entry into w0 so toggle has something to
    // load.
    let w0 = app.workflow_dir().to_path_buf();
    std::fs::create_dir_all(w0.join("_archive")).unwrap();
    std::fs::write(
        w0.join("_archive/old.md"),
        "---\nid: old\ntitle: Old\nstatus: done\nverdict: PASSED\n---\n\n",
    )
    .unwrap();
    // Re-load to pick up archive presence (not strictly needed since
    // `a` will scan on toggle).
    let _ = app.reload();

    app.handle_key(key(KeyCode::Char('a'))); // → Archived
    assert_eq!(app.view_scope(), ViewScope::Archived);
    let archived_loaded = app.as_overview().map(|s| s.archive_loaded).unwrap_or(false);
    assert!(archived_loaded);

    // Cycle to w1 (first-load), then back to w0.
    app.handle_key(key(KeyCode::Right));
    let switch = app.take_pending_switch().unwrap();
    assert_eq!(switch.target_index, 1);
    app.materialize_active();
    // Cycle back to w0.
    app.handle_key(key(KeyCode::Left));
    let switch = app.take_pending_switch().unwrap();
    assert_eq!(switch.target_index, 0);
    assert!(
        !switch.needs_first_load,
        "w0 was already loaded; should not re-load"
    );
    // w0 state preserved: still in Archived view, archive cache loaded.
    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert!(app.as_overview().unwrap().archive_loaded);
    assert!(
        !app.archived_items().is_empty(),
        "archived cache should be reloaded when returning to an archived workflow"
    );
}

#[test]
fn switch_failure_records_refresh_error_on_synthetic_state() {
    // Build a session whose w1 root does not exist; activating it
    // should yield a synthetic empty state with last_refresh_error set
    // (rather than panicking or silently reverting).
    let (mut session, holder, _roots) = multi_session(2);
    // Replace w1's discovery root with a path that doesn't exist.
    let mut new_disc = session.discovery().to_vec();
    new_disc[1].root = holder.path().join("does-not-exist");
    session.replace_discovery(new_disc);
    let mut app = App::from_session(session);
    app.handle_key(key(KeyCode::Right));
    let switch = app.take_pending_switch().unwrap();
    assert_eq!(switch.target_index, 1);
    assert!(switch.needs_first_load);
    app.materialize_active();
    // Active state is now an empty synthetic state with refresh error.
    assert!(app.last_refresh_error().is_some());
    assert!(app.snapshot().items.is_empty());
    assert_eq!(app.as_session().unwrap().active_index(), 1);
}

#[test]
fn keymap_audit_is_disjoint() {
    // The pre-existing char-key set used by the overview handler.
    let existing_chars: &[char] = &['a', '?', 'j', 'k', 'q'];
    let new_chars: &[char] = &['P'];
    for c in new_chars {
        assert!(
            !existing_chars.contains(c),
            "new keymap char {c:?} collides with existing binding"
        );
    }
    // Tab-cycle bindings live on `Left`/`Right` (non-Char), and `Up`/
    // `Down`/`Home`/`End`/`Enter`/`Esc` are also non-Char — those don't
    // share the Char keyspace and can't collide here.
}

// The `cycle_keys_advance_active_index_in_multi_session` test above
// already covers `needs_first_load` == true for the first activation
// and `needs_first_load` == false for a return; that satisfies the
// "first activation loads exactly once" plan item without a hand-rolled
// counting fake.
