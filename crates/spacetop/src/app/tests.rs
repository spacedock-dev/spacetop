use super::{App, AppMode, HistoryWorkerResult, ViewScope};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use spacetop_core::discovery::DiscoveredWorkflow;
use spacetop_core::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};
use spacetop_core::sources::ArchiveSnapshot;

#[test]
fn stores_workflow_directory() {
    let app = App::new("docs/spacetop-dev");

    assert_eq!(app.workflow_dir(), Path::new("docs/spacetop-dev"));
}

#[test]
fn app_stores_config_for_key_handling() {
    let config = spacetop_core::config::SpacetopConfig {
        keybindings: spacetop_core::config::KeybindingConfig {
            search: "f".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let app = App::new_with_config("/tmp/workflow", config.clone());

    assert_eq!(app.config(), &config);
}

#[test]
fn app_stores_config_warnings_for_status_rendering() {
    let warning = spacetop_core::config::ConfigWarning {
        message: "failed to parse config: test".to_string(),
    };

    let app = App::new_with_config_warnings(
        "/tmp/workflow",
        spacetop_core::config::SpacetopConfig::default(),
        vec![warning.clone()],
    );

    assert_eq!(app.config_warnings(), &[warning]);
}

#[test]
fn config_default_scope_applies_when_session_has_no_saved_scope() {
    let config = spacetop_core::config::SpacetopConfig {
        defaults: spacetop_core::config::DefaultsConfig {
            scope: spacetop_core::config::DefaultScope::Archived,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut state = overview_state_with_active_and_archived_items();

    state.apply_config_defaults(&config);

    assert_eq!(state.view_scope(), ViewScope::Archived);
}

#[test]
fn session_scope_overrides_config_default_scope() {
    let config = spacetop_core::config::SpacetopConfig {
        defaults: spacetop_core::config::DefaultsConfig {
            scope: spacetop_core::config::DefaultScope::Archived,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut state = overview_state_with_active_and_archived_items();

    state.apply_config_defaults(&config);
    state.apply_session(&spacetop_core::session_state::WorkflowSession {
        selected_entity_id: None,
        scope: spacetop_core::session_state::WorkflowScope::Active,
    });

    assert_eq!(state.view_scope(), ViewScope::Active);
}

#[test]
fn overview_applies_saved_selected_entity() {
    let root = PathBuf::from("/tmp/spacetop-session-restore-test");
    let snapshot = snapshot_from_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    let mut state = OverviewState::from_snapshot(root, snapshot);

    state.apply_session(&spacetop_core::session_state::WorkflowSession {
        selected_entity_id: Some("002".to_string()),
        scope: spacetop_core::session_state::WorkflowScope::Active,
    });

    assert_eq!(state.selected_item().expect("selected").id, "002");
}

#[test]
fn app_applies_session_state_by_canonical_workflow_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("workflow");
    std::fs::create_dir_all(&root).expect("workflow dir");
    let app = app_with_two_items_at(&root);
    let key = spacetop_core::session_state::WorkflowSessionKey::from_workflow_dir(&root)
        .expect("session key");
    let state = spacetop_core::session_state::SessionState {
        workflows: BTreeMap::from([(
            key.as_str().to_string(),
            spacetop_core::session_state::WorkflowSession {
                selected_entity_id: Some("002".to_string()),
                scope: spacetop_core::session_state::WorkflowScope::Active,
            },
        )]),
    };
    let mut app = app;

    app.apply_session_state(state);

    assert_eq!(app.selected_item().expect("selected").id, "002");
}

#[test]
fn app_session_state_snapshot_uses_canonical_workflow_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("workflow");
    std::fs::create_dir_all(&root).expect("workflow dir");
    let mut app = app_with_two_items_at(&root);
    app.handle_key(key(KeyCode::Down));

    let state = app.session_state_snapshot();
    let key = spacetop_core::session_state::WorkflowSessionKey::from_workflow_dir(&root)
        .expect("session key");
    let saved = state
        .workflows
        .get(key.as_str())
        .expect("workflow session saved");

    assert_eq!(saved.selected_entity_id.as_deref(), Some("002"));
    assert_eq!(
        saved.scope,
        spacetop_core::session_state::WorkflowScope::Active
    );
}

#[test]
fn app_session_state_overrides_config_default_scope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("workflow");
    std::fs::create_dir_all(root.join("_archive")).expect("archive dir");
    let mut snapshot = snapshot_from_items(vec![item_at(
        root.join("001-first.md"),
        "001",
        "first",
        "plan",
    )]);
    snapshot.definition.root = root.clone();
    let config = spacetop_core::config::SpacetopConfig {
        defaults: spacetop_core::config::DefaultsConfig {
            scope: spacetop_core::config::DefaultScope::Archived,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut app = App::from_snapshot_with_config(root.clone(), snapshot, config);
    assert_eq!(app.view_scope(), ViewScope::Archived);
    let key = spacetop_core::session_state::WorkflowSessionKey::from_workflow_dir(&root)
        .expect("session key");

    app.apply_session_state(spacetop_core::session_state::SessionState {
        workflows: BTreeMap::from([(
            key.as_str().to_string(),
            spacetop_core::session_state::WorkflowSession {
                selected_entity_id: None,
                scope: spacetop_core::session_state::WorkflowScope::Active,
            },
        )]),
    });

    assert_eq!(app.view_scope(), ViewScope::Active);
}

#[test]
fn loads_real_workflow_state_and_derives_stage_counts() {
    let root = PathBuf::from("workflow");
    let app = App::from_snapshot(root.clone(), snapshot_with_items(3));
    let snapshot = app.snapshot();
    let expected_stage_counts = snapshot
        .definition
        .stages
        .iter()
        .map(|stage| {
            (
                stage.name.as_str(),
                snapshot
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
        app.selected_item().map(|item| item.title),
        snapshot.items.first().map(|item| item.title.clone())
    );
    assert_eq!(
        app.selected_item().map(|item| item.status),
        snapshot.items.first().map(|item| item.status.clone())
    );
    // The workflow has at least one stage and at least one item — these
    // are intrinsic invariants of the loaded fixture, not specific titles.
    assert!(!snapshot.definition.stages.is_empty());
    assert!(!snapshot.items.is_empty());
}

#[test]
fn stage_counts_include_archived_done_items_from_the_workflow_archive() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let mut app = App::load(root).expect("workflow should load");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char('a')));

    let counts = app.stage_counts();
    let done = counts
        .iter()
        .find(|count| count.name == "done")
        .expect("done stage should exist");
    assert!(
        done.items > 0,
        "done count should come from archived workflow items"
    );

    let snapshot = app.snapshot();
    let expected_active_counts = snapshot
        .definition
        .stages
        .iter()
        .filter(|stage| stage.name != "done")
        .map(|stage| {
            (
                stage.name.as_str(),
                snapshot
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

    let mut app = App::load(root.to_path_buf()).expect("workflow should load");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char('a')));
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

    let mut app = App::load(root.to_path_buf()).expect("workflow should load");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char('a')));

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

    let mut app = App::load(root.to_path_buf()).expect("workflow should load");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char('a')));

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
    // Page step is viewport-relative (viewport - 1); pin a 7-row body so a
    // page == 6 rows and the offsets below stay deterministic.
    app.as_overview().unwrap().preview_viewport_height.set(7);
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
    // Viewport-relative page step: 7-row body => a page is 6 rows.
    app.as_overview().unwrap().preview_viewport_height.set(7);
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
fn preview_to_top_and_bottom_set_extremes() {
    let mut state =
        super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    state.preview_open = true;
    state.max_preview_scroll.set(42);

    state.scroll_preview_to_bottom();
    assert_eq!(state.preview_scroll(), 42, "G jumps to the clamped max");

    state.scroll_preview_to_top();
    assert_eq!(state.preview_scroll(), 0, "g jumps to the top");
}

#[test]
fn preview_to_bottom_then_page_up_is_not_stuck() {
    // Regression R1: scroll_preview_to_bottom must store the real max (not
    // usize::MAX), so a following page-up moves off the bottom instead of
    // computing MAX - step (still past bottom) and appearing to no-op.
    let mut state =
        super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    state.preview_open = true;
    state.max_preview_scroll.set(10);
    state.preview_viewport_height.set(5); // page step = 4

    state.scroll_preview_to_bottom();
    assert_eq!(state.preview_scroll(), 10);

    state.scroll_preview_up();
    assert!(
        state.preview_scroll() < 10,
        "page-up after G must move off the bottom (offset not poisoned)"
    );
}

#[test]
fn preview_scroll_reclamps_after_viewport_shrinks() {
    // Regression R4: a terminal resize that shrinks the doc's overflow lowers
    // max_preview_scroll. The next scroll must re-clamp the stored offset to
    // the new max BEFORE applying its delta — never leave it stranded high.
    let mut state =
        super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    state.preview_open = true;
    state.max_preview_scroll.set(100);
    state.preview_viewport_height.set(20);
    state.scroll_preview_to_bottom();
    assert_eq!(state.preview_scroll(), 100);

    // Resize: far less content overflows now.
    state.max_preview_scroll.set(10);
    state.scroll_preview_up();
    assert!(
        state.preview_scroll() <= 10,
        "stored offset must be re-clamped to the fresh max before the delta"
    );
}

#[test]
fn preview_scroll_keys_noop_when_doc_fits_pane() {
    // max_preview_scroll == 0 means the doc fits; every scroll key is a no-op.
    let mut state =
        super::OverviewState::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(1));
    state.preview_open = true;
    state.max_preview_scroll.set(0);
    state.preview_viewport_height.set(20);

    state.scroll_preview_down();
    state.scroll_preview_to_bottom();
    assert_eq!(
        state.preview_scroll(),
        0,
        "no scrolling when the document fits the pane"
    );
}

#[test]
fn space_and_b_page_scroll_the_preview() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    app.handle_key(key(KeyCode::Enter));
    // Viewport-relative page step: 7-row body => a page is 6 rows.
    app.as_overview().unwrap().preview_viewport_height.set(7);

    app.handle_key(key(KeyCode::Char(' ')));
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 6);
    assert_eq!(
        app.selected_index(),
        0,
        "Space must not move task selection"
    );

    app.handle_key(key(KeyCode::Char('b')));
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 0);
    assert_eq!(app.selected_index(), 0, "b must not move task selection");
}

#[test]
fn g_and_shift_g_jump_to_preview_ends() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    app.handle_key(key(KeyCode::Enter));
    app.as_overview().unwrap().max_preview_scroll.set(10);

    app.handle_key(key(KeyCode::Char('G')));
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 10);

    app.handle_key(key(KeyCode::Char('g')));
    assert_eq!(app.as_overview().unwrap().preview_scroll(), 0);
}

#[test]
fn jk_and_arrows_still_move_selection_with_preview_open() {
    // Regression R2: opening the preview must NOT steal j/k/arrows — they stay
    // task navigation in the list-driven model.
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.as_overview().unwrap().preview_open());

    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(
        app.selected_index(),
        1,
        "j moves selection with preview open"
    );
    app.handle_key(key(KeyCode::Down));
    assert_eq!(
        app.selected_index(),
        2,
        "Down moves selection with preview open"
    );
    app.handle_key(key(KeyCode::Char('k')));
    assert_eq!(app.selected_index(), 1, "k moves selection back");
    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.selected_index(), 0, "Up moves selection back");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
    let mut app = App::load(root).expect("workflow should load");
    assert_eq!(app.view_scope(), ViewScope::Active);
    assert!(app.archived_count().is_none());

    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert!(app.archived_count().is_some());
    assert!(!app.archived_items().is_empty());
    // Selected item should be an archived entry. The row preserves the
    // frontmatter status that existed before archival, so it is not necessarily
    // `done`.
    let selected = app.selected_item().expect("selected archived item");
    assert!(
        selected
            .path
            .components()
            .any(|part| part.as_os_str() == "_archive"),
        "selected archived item path should live under _archive, got {:?}",
        selected.path
    );

    app.handle_key(key(KeyCode::Char('a')));
    assert_eq!(app.view_scope(), ViewScope::Active);
}

#[test]
fn archive_parse_errors_surface_only_after_archive_scope_loads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write_workflow(root, "001");
    std::fs::create_dir_all(root.join("_archive")).unwrap();
    std::fs::write(
        root.join("_archive").join("bad.md"),
        "---\nid: [\n---\n\nbroken\n",
    )
    .unwrap();

    let mut app = App::load(root.to_path_buf()).expect("workflow should load");
    assert!(!app.as_overview().unwrap().archive_loaded);
    assert!(app.as_overview().unwrap().parse_errors().is_empty());

    app.handle_key(key(KeyCode::Char('a')));

    let overview = app.as_overview().unwrap();
    assert_eq!(overview.view_scope(), ViewScope::Archived);
    assert!(overview.archive_loaded);
    assert_eq!(overview.parse_errors().len(), 1);
    assert!(overview.parse_errors()[0]
        .message
        .contains("malformed YAML"));
}

#[test]
fn archive_reload_clamps_archived_selection_with_scope_aware_index() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write_workflow(root, "001");
    std::fs::create_dir_all(root.join("_archive")).unwrap();
    std::fs::write(
        root.join("_archive").join("new.md"),
        "---\nid: new\ntitle: Archived New\nstatus: done\ncompleted: 2026-04-28T00:00:00Z\n---\n\nnew body\n",
    )
    .unwrap();
    std::fs::write(
        root.join("_archive").join("old.md"),
        "---\nid: old\ntitle: Archived Old\nstatus: done\ncompleted: 2026-04-27T00:00:00Z\n---\n\nold body\n",
    )
    .unwrap();

    let mut app = App::load(root.to_path_buf()).expect("workflow should load");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert_eq!(app.selected_index(), 1);
    assert_eq!(
        app.selected_item().map(|item| item.title),
        Some("Archived Old".to_string())
    );

    std::fs::remove_file(root.join("_archive").join("old.md")).unwrap();
    app.reload().expect("reload should succeed");

    assert_eq!(app.view_scope(), ViewScope::Archived);
    assert_eq!(app.selected_index(), 0);
    assert_eq!(
        app.selected_item().map(|item| item.title),
        Some("Archived New".to_string())
    );
}

#[test]
fn archived_view_selection_is_independent_of_active_selection() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
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
            stage_prose: HashMap::new(),
            transitions: Vec::new(),
        },
        items: (0..count)
            .map(|index| Entity {
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
                worktree_source: None,
                main_body: None,
            })
            .collect(),
        parse_errors: Vec::new(),
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
            stage_prose: HashMap::new(),
            transitions: Vec::new(),
        },
        items: paths
            .iter()
            .enumerate()
            .map(|(index, p)| Entity {
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
                worktree_source: None,
                main_body: None,
            })
            .collect(),
        parse_errors: Vec::new(),
    }
}

fn item_at(path: PathBuf, id: &str, title: &str, status: &str) -> Entity {
    Entity {
        path,
        id: id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        source: Some("test".to_string()),
        started: None,
        completed: None,
        verdict: None,
        score: Some(0.5),
        worktree: None,
        issue: None,
        pr: None,
        body: format!("Body excerpt for {title}."),
        worktree_source: None,
        main_body: None,
    }
}

fn snapshot_from_items(items: Vec<Entity>) -> WorkflowSnapshot {
    let mut snapshot = snapshot_with_items(0);
    snapshot.items = items;
    snapshot
}

fn app_with_two_items_at(root: &Path) -> App {
    let mut snapshot = snapshot_from_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    snapshot.definition.root = root.to_path_buf();
    App::from_snapshot(root.to_path_buf(), snapshot)
}

fn overview_state_with_active_and_archived_items() -> OverviewState {
    let root = PathBuf::from("/tmp/spacetop-config-default-test");
    let mut snapshot = snapshot_from_items(vec![item_at(
        root.join("001-active.md"),
        "001",
        "active",
        "plan",
    )]);
    snapshot.definition.root = root.clone();
    snapshot.definition.stages = vec![
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
    let mut state = OverviewState::from_snapshot(root.clone(), snapshot);
    state.index = state.index.clone().with_archive(ArchiveSnapshot {
        entities: vec![item_at(
            root.join("_archive").join("999-done.md"),
            "999",
            "archived",
            "done",
        )],
        parse_errors: Vec::new(),
        error: None,
    });
    state.archive_loaded = true;
    state.archived_done_count = Some(1);
    state
}

#[test]
fn reload_from_index_preserves_selection_by_slug() {
    let root = PathBuf::from("/tmp/spacetop-index-test");
    let first = snapshot_from_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    let mut state = OverviewState::from_snapshot(root.clone(), first);
    state.select_next();
    assert_eq!(state.selected_item().expect("selected").id, "002");

    let second = snapshot_from_items(vec![
        item_at(root.join("002-second.md"), "002", "second changed", "plan"),
        item_at(root.join("003-third.md"), "003", "third", "plan"),
    ]);
    let index = spacetop_core::index::WorkflowIndex::from_sources(
        spacetop_core::sources::WorkflowSources {
            active: second,
            archive: spacetop_core::sources::ArchiveSnapshot::empty(),
        },
    );
    state.reload_from_index(index);

    assert_eq!(state.selected_item().expect("selected").id, "002");
}

#[test]
fn reload_replaces_index_contents() {
    let root = PathBuf::from("/tmp/spacetop-index-reload-test");
    let first = snapshot_from_items(vec![item_at(
        root.join("001-first.md"),
        "001",
        "first",
        "plan",
    )]);
    let mut state = OverviewState::from_snapshot(root.clone(), first);
    assert_eq!(state.visible_items().len(), 1);

    let second = snapshot_from_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    let index = spacetop_core::index::WorkflowIndex::from_sources(
        spacetop_core::sources::WorkflowSources {
            active: second,
            archive: spacetop_core::sources::ArchiveSnapshot::empty(),
        },
    );
    state.reload_from_index(index);

    let ids: Vec<String> = state
        .visible_items()
        .iter()
        .map(|entity| entity.id.clone())
        .collect();
    assert_eq!(ids, ["001", "002"]);
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
    use spacetop_core::sources::ArchiveSnapshot;
    let mut overview = super::OverviewState::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md"]),
    );
    // Force into archived scope with synthetic archived items.
    overview.view_scope = ViewScope::Archived;
    overview.index = overview.index.clone().with_archive(ArchiveSnapshot {
        entities: vec![item_at(
            PathBuf::from("workflow/_archive/old.md"),
            "old",
            "Old",
            "done",
        )],
        parse_errors: Vec::new(),
        error: None,
    });
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
fn reload_records_per_entity_parse_error_without_dropping_other_items() {
    // Minimal real workflow fixture in a tempdir with TWO tasks: one stays
    // valid, the other gets poisoned. Per-entity errors are non-fatal so
    // `reload` returns Ok and the snapshot contains the valid item plus a
    // captured parse error for the bad entity. The README itself is fine,
    // so the workflow still loads.
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
    std::fs::write(
        root.join("task-two.md"),
        "---\nid: 002\ntitle: Two\nstatus: plan\n---\n\nbody\n",
    )
    .unwrap();

    let mut app = App::load(root.to_path_buf()).expect("load ok");
    assert_eq!(app.snapshot().items.len(), 2);
    assert!(app.snapshot().parse_errors.is_empty());

    // Poison one task file: invalid YAML frontmatter.
    std::fs::write(
        root.join("task-one.md"),
        "---\nid: [not valid yaml\nstatus\n---\nbody\n",
    )
    .unwrap();

    let result = app.reload();
    assert!(
        result.is_ok(),
        "per-entity parse failures are now non-fatal: {result:?}"
    );
    assert_eq!(
        app.snapshot().items.len(),
        1,
        "valid task should still appear; got {:?}",
        app.snapshot()
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        app.snapshot().parse_errors.len(),
        1,
        "bad task should be recorded as a parse_error"
    );
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

#[test]
fn reload_from_snapshot_updates_counts_and_clamps_selection() {
    // AC-3: a watcher-driven reload with a different item set must update
    // stage_counts and archived_done_count, and clamp selection without
    // panicking when the visible list shrinks.
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(
        root.join("README.md"),
        "---\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: implement\n    - name: done\n      terminal: true\n---\n",
    )
    .unwrap();
    // Two active tasks (plan, implement) + one archived done.
    std::fs::write(
        root.join("task-001.md"),
        "---\nid: 001\ntitle: A\nstatus: plan\n---\n\nbody\n",
    )
    .unwrap();
    std::fs::write(
        root.join("task-002.md"),
        "---\nid: 002\ntitle: B\nstatus: implement\n---\n\nbody\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("_archive")).unwrap();
    std::fs::write(
        root.join("_archive").join("task-900.md"),
        "---\nid: 900\ntitle: Z\nstatus: done\ncompleted: 2026-04-27T00:00:00Z\n---\n\nbody\n",
    )
    .unwrap();

    let mut app = App::load(root.to_path_buf()).expect("load ok");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char('a')));
    // Advance selection to the last active item.
    app.handle_key(key(KeyCode::End));
    assert_eq!(app.selected_index(), 1);

    let initial_counts: HashMap<String, usize> = app
        .stage_counts()
        .iter()
        .map(|c| (c.name.clone(), c.items))
        .collect();
    assert_eq!(initial_counts.get("plan").copied(), Some(1));
    assert_eq!(initial_counts.get("implement").copied(), Some(1));
    // Archived done contributes to the done count.
    assert_eq!(initial_counts.get("done").copied(), Some(1));
    assert_eq!(
        app.as_overview().unwrap().archived_done_count,
        Some(1),
        "archived done count cached after load"
    );

    // Simulate a post-merge filesystem change: task-002 moves into the
    // archive with status flipped to done, and another archived done is
    // added alongside it. Active list shrinks from 2 → 1.
    std::fs::remove_file(root.join("task-002.md")).unwrap();
    std::fs::write(
        root.join("_archive").join("task-002.md"),
        "---\nid: 002\ntitle: B\nstatus: done\ncompleted: 2026-04-27T00:00:00Z\n---\n\nbody\n",
    )
    .unwrap();
    std::fs::write(
        root.join("_archive").join("task-901.md"),
        "---\nid: 901\ntitle: ZZ\nstatus: done\ncompleted: 2026-04-27T00:00:00Z\n---\n\nbody\n",
    )
    .unwrap();
    // Also flip the remaining active item's status (plan → implement) so
    // stage_counts shifts demonstrably.
    std::fs::write(
        root.join("task-001.md"),
        "---\nid: 001\ntitle: A\nstatus: implement\n---\n\nbody\n",
    )
    .unwrap();

    app.reload().expect("reload ok");
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Char('a')));

    // Selection must clamp without panic: visible list shrank to 1 entry.
    assert_eq!(app.snapshot().items.len(), 1);
    assert_eq!(app.selected_index(), 0);

    let counts: HashMap<String, usize> = app
        .stage_counts()
        .iter()
        .map(|c| (c.name.clone(), c.items))
        .collect();
    assert_eq!(counts.get("plan").copied(), Some(0));
    assert_eq!(counts.get("implement").copied(), Some(1));
    // archived_done_count must be recomputed: now task-900 + task-002 +
    // task-901 = 3 archived done.
    assert_eq!(
        app.as_overview().unwrap().archived_done_count,
        Some(3),
        "archived_done_count refreshed after reload_from_snapshot"
    );
    assert_eq!(counts.get("done").copied(), Some(3));
}

#[test]
fn pressing_s_cycles_sort_mode() {
    use super::SortMode;
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    assert_eq!(app.as_overview().unwrap().sort_mode(), SortMode::Id);
    app.handle_key(key(KeyCode::Char('s')));
    assert_eq!(app.as_overview().unwrap().sort_mode(), SortMode::Status);
    app.handle_key(key(KeyCode::Char('s')));
    assert_eq!(app.as_overview().unwrap().sort_mode(), SortMode::Id);
}

#[test]
fn pressing_s_does_not_cycle_sort_when_preview_open() {
    use super::SortMode;
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.as_overview().unwrap().preview_open());
    app.handle_key(key(KeyCode::Char('s')));
    assert_eq!(app.as_overview().unwrap().sort_mode(), SortMode::Id);
}

// The `cycle_keys_advance_active_index_in_multi_session` test above
// already covers `needs_first_load` == true for the first activation
// and `needs_first_load` == false for a return; that satisfies the
// "first activation loads exactly once" plan item without a hand-rolled
// counting fake.

// --- Definition view tests (task 041) ---

/// AC-1: `D` from Overview transitions to `AppMode::Definition` with
/// the underlying session preserved verbatim.
#[test]
fn d_from_overview_enters_definition_mode() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    assert!(matches!(app.mode(), AppMode::Overview(_)));
    app.handle_key(key(KeyCode::Char('D')));
    assert!(
        matches!(app.mode(), AppMode::Definition { .. }),
        "expected Definition mode after D"
    );
    assert_eq!(app.definition_scroll(), Some(0));
}

/// AC-1: `Esc` from the Definition view restores the underlying
/// Overview state — selection, view scope, sort mode, and preview
/// open flag must all survive verbatim.
#[test]
fn esc_from_definition_restores_overview_state() {
    use super::SortMode;
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    // Move selection, cycle sort, capture probes.
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('s')));
    let probe_index = app.selected_index();
    let probe_scope = app.view_scope();
    let probe_sort = app.as_overview().unwrap().sort_mode();
    let probe_preview = app.as_overview().unwrap().preview_open();
    assert_eq!(probe_index, 1);
    assert_eq!(probe_sort, SortMode::Status);

    // Open Definition, then Esc back.
    app.handle_key(key(KeyCode::Char('D')));
    assert!(matches!(app.mode(), AppMode::Definition { .. }));
    app.handle_key(key(KeyCode::Esc));

    assert!(matches!(app.mode(), AppMode::Overview(_)));
    assert_eq!(app.selected_index(), probe_index);
    assert_eq!(app.view_scope(), probe_scope);
    assert_eq!(app.as_overview().unwrap().sort_mode(), probe_sort);
    assert_eq!(app.as_overview().unwrap().preview_open(), probe_preview);
}

/// AC-1: pressing `D` again from inside the Definition view closes
/// the view (it's also a toggle).
#[test]
fn d_inside_definition_closes_the_view() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.handle_key(key(KeyCode::Char('D')));
    assert!(app.is_definition());
    app.handle_key(key(KeyCode::Char('D')));
    assert!(!app.is_definition());
    assert!(matches!(app.mode(), AppMode::Overview(_)));
}

/// AC-1: `D` is ignored while the preview pane is open — preview
/// open guards the binding (parallel to the `s` sort binding).
#[test]
fn d_ignored_when_preview_open() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.as_overview().unwrap().preview_open());
    app.handle_key(key(KeyCode::Char('D')));
    assert!(
        matches!(app.mode(), AppMode::Overview(_)),
        "preview_open must guard D"
    );
}

/// AC-1: navigation keys inside Definition mode advance scroll
/// without escaping the mode.
#[test]
fn scroll_keys_advance_definition_scroll() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.handle_key(key(KeyCode::Char('D')));
    assert_eq!(app.definition_scroll(), Some(0));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.definition_scroll(), Some(2));
    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.definition_scroll(), Some(1));
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.definition_scroll(), Some(11));
    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.definition_scroll(), Some(1));
    app.handle_key(key(KeyCode::Home));
    assert_eq!(app.definition_scroll(), Some(0));
    app.handle_key(key(KeyCode::End));
    assert_eq!(app.definition_scroll(), Some(usize::MAX));
}

/// AC-5: in a multi-workflow session, opening Definition on the
/// middle tab and pressing Esc returns to the same active tab with
/// the per-tab `selected_index` unchanged.
#[test]
fn definition_scopes_to_active_tab_and_esc_preserves_index() {
    let (session, _holder, _roots) = multi_session(3);
    let mut app = App::from_session(session);
    // Cycle to middle tab.
    app.handle_key(key(KeyCode::Right));
    let _ = app.take_pending_switch();
    app.materialize_active();
    assert_eq!(app.as_session().unwrap().active_index(), 1);
    let probe_selected = app.selected_index();

    // Open Definition; verify it scopes to the active session.
    app.handle_key(key(KeyCode::Char('D')));
    assert!(app.is_definition());
    assert_eq!(app.as_session().unwrap().active_index(), 1);

    // Esc returns; tab + selection intact.
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.mode(), AppMode::Overview(_)));
    assert_eq!(app.as_session().unwrap().active_index(), 1);
    assert_eq!(app.selected_index(), probe_selected);
}

// --- P3 capability view app-mode tests ---

#[test]
fn slash_from_overview_enters_search_mode_and_esc_restores_state() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    app.handle_key(key(KeyCode::Down));
    let probe_index = app.selected_index();

    app.handle_key(key(KeyCode::Char('/')));
    assert!(matches!(
        app.mode(),
        AppMode::Search {
            state,
            ..
        } if state.mode() == super::SearchMode::Search
    ));

    app.handle_key(key(KeyCode::Char('q')));
    assert!(matches!(app.mode(), AppMode::Search { .. }));
    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Esc));

    assert!(matches!(app.mode(), AppMode::Overview(_)));
    assert_eq!(app.selected_index(), probe_index);
}

#[test]
fn search_overlay_question_mark_opens_help_and_esc_keeps_overlay_usable() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

    app.handle_key(key(KeyCode::Char('/')));
    app.handle_key(key(KeyCode::Char('?')));

    assert!(app.help_open(), "? should open help from Search overlay");
    assert!(
        matches!(
            app.mode(),
            AppMode::Search {
                state,
                ..
            } if state.mode() == super::SearchMode::Search && state.query().is_empty()
        ),
        "? must not be inserted into the search query"
    );

    app.handle_key(key(KeyCode::Esc));
    assert!(!app.help_open(), "Esc should close help first");
    assert!(matches!(app.mode(), AppMode::Search { .. }));

    app.handle_key(key(KeyCode::Char('1')));
    assert!(matches!(
        app.mode(),
        AppMode::Search {
            state,
            ..
        } if state.query() == "1"
    ));

    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(app.mode(), AppMode::Overview(_)));
}

#[test]
fn command_overlay_question_mark_opens_help_and_esc_keeps_overlay_usable() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));

    app.handle_key(key(KeyCode::Char(':')));
    app.handle_key(key(KeyCode::Char('?')));

    assert!(app.help_open(), "? should open help from Command overlay");
    assert!(
        matches!(
            app.mode(),
            AppMode::Search {
                state,
                ..
            } if state.mode() == super::SearchMode::Command && state.query().is_empty()
        ),
        "? must not be inserted into the command query"
    );

    app.handle_key(key(KeyCode::Esc));
    assert!(!app.help_open(), "Esc should close help first");
    assert!(matches!(app.mode(), AppMode::Search { .. }));

    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.mode(), AppMode::Metrics { .. }));
}

#[test]
fn search_overlay_selection_and_activation_are_bounded_to_visible_results() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(10));

    app.handle_key(key(KeyCode::Char('/')));
    app.handle_key(key(KeyCode::Char('T')));
    for _ in 0..20 {
        app.handle_key(key(KeyCode::Down));
    }

    assert!(matches!(
        app.mode(),
        AppMode::Search {
            state,
            ..
        } if state.selected_index() == 7
    ));

    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.selected_item().map(|entity| entity.id),
        Some("007".to_string()),
        "Enter should activate the last visible search result, not an off-screen match"
    );
}

#[test]
fn command_palette_dispatches_metrics_activity_timeline_and_relations() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));

    app.handle_key(key(KeyCode::Char(':')));
    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.mode(), AppMode::Metrics { .. }));
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char(':')));
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.mode(), AppMode::Activity { .. }));
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char(':')));
    app.handle_key(key(KeyCode::Char('t')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(
        app.mode(),
        AppMode::Timeline { entity_id, .. } if entity_id == "000"
    ));
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char(':')));
    app.handle_key(key(KeyCode::Char('r')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(
        app.mode(),
        AppMode::Relations { entity_id, .. } if entity_id == "000"
    ));
}

#[test]
fn p3_view_keys_open_read_only_modes_for_selected_entity() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.handle_key(key(KeyCode::Down));

    app.handle_key(key(KeyCode::Char('T')));
    assert!(matches!(
        app.mode(),
        AppMode::Timeline { entity_id, .. } if entity_id == "001"
    ));
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char('M')));
    assert!(matches!(app.mode(), AppMode::Metrics { .. }));
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char('A')));
    assert!(matches!(app.mode(), AppMode::Activity { .. }));
    app.handle_key(key(KeyCode::Esc));

    app.handle_key(key(KeyCode::Char('R')));
    assert!(matches!(
        app.mode(),
        AppMode::Relations { entity_id, .. } if entity_id == "001"
    ));
}

#[test]
fn p3_modes_preserve_underlying_session_plumbing() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(3));
    app.handle_key(key(KeyCode::Down));
    let probe_index = app.selected_index();
    let probe_dir = app.workflow_dir().to_path_buf();

    for open_key in ['T', 'M', 'A', 'R'] {
        let mut app = app.clone();
        app.handle_key(key(KeyCode::Char(open_key)));
        assert_eq!(app.workflow_dir(), probe_dir.as_path());
        assert_eq!(app.selected_index(), probe_index);
        assert!(
            app.history_worker_request().is_some(),
            "{open_key} mode must preserve history worker access"
        );
        app.set_sync_status(super::SyncStatus::InFlight);
        assert_eq!(app.sync_status(), Some(&super::SyncStatus::InFlight));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode(), AppMode::Overview(_)));
        assert_eq!(app.selected_index(), probe_index);
    }
}

#[test]
fn p3_full_pane_modes_open_help_and_esc_still_restores_overview() {
    for open_key in P3_FULL_PANE_KEYS {
        let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
        open_p3_full_pane_mode(&mut app, open_key);

        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.help_open(), "{open_key} mode must open help with ?");
        assert_p3_full_pane_mode(&app, open_key);

        app.handle_key(key(KeyCode::Esc));
        assert!(!app.help_open(), "Esc must close help first");
        assert_p3_full_pane_mode(&app, open_key);

        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode(), AppMode::Overview(_)));
    }
}

#[test]
fn p3_full_pane_modes_switch_workflows_left_and_right() {
    for open_key in P3_FULL_PANE_KEYS {
        let (session, _holder, roots) = multi_session(3);
        let mut app = App::from_session(session);
        open_p3_full_pane_mode(&mut app, open_key);

        app.handle_key(key(KeyCode::Right));
        let switch = app
            .take_pending_switch()
            .unwrap_or_else(|| panic!("{open_key} mode must emit switch on Right"));
        assert_eq!(switch.target_index, 1);
        assert!(switch.needs_first_load);
        assert_eq!(app.as_session().unwrap().active_index(), 1);
        assert_p3_full_pane_mode(&app, open_key);

        app.materialize_active();
        assert_eq!(app.workflow_dir(), roots[1].as_path());
        assert_eq!(
            app.selected_item().map(|entity| entity.id),
            Some("001".to_string())
        );
        assert_p3_full_pane_mode(&app, open_key);

        app.handle_key(key(KeyCode::Left));
        let switch = app
            .take_pending_switch()
            .unwrap_or_else(|| panic!("{open_key} mode must emit switch on Left"));
        assert_eq!(switch.target_index, 0);
        assert!(!switch.needs_first_load);
        assert_eq!(app.as_session().unwrap().active_index(), 0);
    }
}

#[test]
fn p3_full_pane_modes_can_open_picker_overlay() {
    for open_key in P3_FULL_PANE_KEYS {
        let (session, _holder, _roots) = multi_session(2);
        let mut app = App::from_session(session);
        open_p3_full_pane_mode(&mut app, open_key);

        app.handle_key(key(KeyCode::Char('P')));
        assert!(
            app.take_pending_overlay_open(),
            "{open_key} mode must schedule picker overlay with P"
        );
        let same_list = app.as_session().unwrap().discovery().to_vec();
        app.open_picker_overlay_with(Ok(same_list));
        assert!(app.is_overlay(), "{open_key} mode must open picker overlay");

        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode(), AppMode::Overview(_)));
        assert!(!app.should_quit());
    }
}

const P3_FULL_PANE_KEYS: [char; 4] = ['T', 'M', 'A', 'R'];

fn open_p3_full_pane_mode(app: &mut App, open_key: char) {
    app.handle_key(key(KeyCode::Char(open_key)));
    assert_p3_full_pane_mode(app, open_key);
}

fn assert_p3_full_pane_mode(app: &App, open_key: char) {
    let mode_matches = matches!(
        (open_key, app.mode()),
        ('T', AppMode::Timeline { .. })
            | ('M', AppMode::Metrics { .. })
            | ('A', AppMode::Activity { .. })
            | ('R', AppMode::Relations { .. })
    );
    assert!(mode_matches, "expected full-pane P3 mode for {open_key}");
}

// ---- Task 046: Sync action plumbing ----

#[test]
fn y_keypress_records_pending_sync_when_preview_closed() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    assert!(!app.take_pending_sync(), "pending_sync starts false");
    app.handle_key(key(KeyCode::Char('Y')));
    assert!(app.take_pending_sync(), "Y must set pending_sync");
    assert!(
        !app.take_pending_sync(),
        "take_pending_sync drains the flag"
    );
}

#[test]
fn y_keypress_is_noop_when_preview_open() {
    let mut app = App::from_snapshot(PathBuf::from("workflow"), snapshot_with_items(2));
    app.handle_key(key(KeyCode::Enter));
    assert!(app.as_overview().unwrap().preview_open());
    app.handle_key(key(KeyCode::Char('Y')));
    assert!(
        !app.take_pending_sync(),
        "Y must NOT set pending_sync when preview is open"
    );
}

#[test]
fn set_sync_status_routes_to_active_overview_and_survives_reload_from_snapshot() {
    use super::SyncStatus;
    let mut app = App::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_paths(&["workflow/alpha.md"]),
    );
    assert!(app.sync_status().is_none());
    app.set_sync_status(SyncStatus::Succeeded { new_commits: 3 });
    match app.sync_status() {
        Some(SyncStatus::Succeeded { new_commits }) => assert_eq!(*new_commits, 3),
        other => panic!("expected Succeeded(3), got {other:?}"),
    }
    // Reload preserves the sync status — it's an out-of-band UI signal,
    // not part of the parsed snapshot.
    app.reload_from_snapshot(snapshot_with_paths(&[
        "workflow/alpha.md",
        "workflow/beta.md",
    ]));
    assert!(
        matches!(
            app.sync_status(),
            Some(SyncStatus::Succeeded { new_commits: 3 })
        ),
        "sync_status must survive reload_from_snapshot, got {:?}",
        app.sync_status()
    );
}

#[test]
fn load_marks_history_loading_without_hiding_active_items() {
    let holder = tempfile::tempdir().expect("tempdir");
    let root = holder.path().join("workflow");
    std::fs::create_dir_all(&root).unwrap();
    write_workflow(&root, "001");

    let app = App::load(root.clone()).expect("load");

    assert_eq!(app.visible_items().len(), 1);
    assert_eq!(
        app.as_overview().unwrap().index().timeline("001"),
        Err(spacetop_core::query::HistoryUnavailable::Loading)
    );
    let request = app.history_worker_request().expect("history request");
    assert_eq!(request.workflow_dir, root);
    assert_eq!(request.workflow_rel, "");
}

#[test]
fn apply_history_result_populates_loaded_overview_timeline() {
    use spacetop_core::index::{CommitId, CommitTime, StageEvent};

    let holder = tempfile::tempdir().expect("tempdir");
    let root = holder.path().join("workflow");
    std::fs::create_dir_all(&root).unwrap();
    write_workflow(&root, "001");
    let mut app = App::load(root.clone()).expect("load");
    assert_eq!(
        app.as_overview().unwrap().index().timeline("001"),
        Err(spacetop_core::query::HistoryUnavailable::Loading)
    );

    app.apply_history_result(HistoryWorkerResult {
        workflow_dir: root,
        result: Ok(vec![StageEvent {
            entity_id: "001".to_string(),
            from: None,
            to: "plan".to_string(),
            at: CommitTime(100),
            commit: CommitId("a".repeat(40)),
        }]),
    });

    let timeline = app
        .as_overview()
        .unwrap()
        .index()
        .timeline("001")
        .expect("timeline");
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].to, "plan");
}

#[test]
fn apply_history_result_surfaces_exact_unavailable_reason() {
    let holder = tempfile::tempdir().expect("tempdir");
    let root = holder.path().join("workflow");
    std::fs::create_dir_all(&root).unwrap();
    write_workflow(&root, "001");
    let mut app = App::load(root.clone()).expect("load");

    app.apply_history_result(HistoryWorkerResult {
        workflow_dir: root,
        result: Err(spacetop_core::query::HistoryUnavailable::ShallowClone),
    });

    let index = app.as_overview().unwrap().index();
    assert_eq!(
        index.timeline("001"),
        Err(spacetop_core::query::HistoryUnavailable::ShallowClone)
    );
    assert_eq!(
        index.metrics(),
        Err(spacetop_core::query::HistoryUnavailable::ShallowClone)
    );
    assert_eq!(
        index.activity(None),
        Err(spacetop_core::query::HistoryUnavailable::ShallowClone)
    );
}

#[test]
fn stale_history_result_for_other_workflow_is_ignored() {
    use spacetop_core::index::{CommitId, CommitTime, StageEvent};

    let holder = tempfile::tempdir().expect("tempdir");
    let root = holder.path().join("workflow");
    std::fs::create_dir_all(&root).unwrap();
    write_workflow(&root, "001");
    let mut app = App::load(root.clone()).expect("load");

    app.apply_history_result(HistoryWorkerResult {
        workflow_dir: holder.path().join("other"),
        result: Ok(vec![StageEvent {
            entity_id: "001".to_string(),
            from: None,
            to: "plan".to_string(),
            at: CommitTime(100),
            commit: CommitId("a".repeat(40)),
        }]),
    });

    assert_eq!(
        app.as_overview().unwrap().index().timeline("001"),
        Err(spacetop_core::query::HistoryUnavailable::Loading)
    );
}

// ---- Task 042: parse_errors surfaced on OverviewState + selectable rows ----

#[test]
fn overview_state_exposes_parse_errors_from_snapshot() {
    use crate::app::{OverviewState, SelectedRow};
    use spacetop_core::domain::EntityParseError;

    let root = PathBuf::from("workflow");
    let mut snapshot = snapshot_with_paths(&["workflow/good-1.md"]);
    snapshot.parse_errors.push(EntityParseError {
        path: PathBuf::from("workflow/bad.md"),
        message: "workflow/bad.md: malformed YAML frontmatter: mapping values are not allowed in this context at line 7 column 137".to_string(),
        line: Some(7),
        column: Some(137),
    });
    let state = OverviewState::from_snapshot(root, snapshot);
    assert_eq!(state.parse_errors().len(), 1);
    assert_eq!(
        state.parse_errors()[0].path,
        PathBuf::from("workflow/bad.md")
    );

    // Selection at index 0 is the work item; index 1 is the broken row.
    assert!(matches!(state.selected_row(), Some(SelectedRow::Item(_))));
    let mut state = state;
    state.select_next();
    match state.selected_row() {
        Some(SelectedRow::Broken(err)) => {
            assert_eq!(err.line, Some(7));
            assert_eq!(err.column, Some(137));
        }
        other => panic!("expected SelectedRow::Broken, got {other:?}"),
    }
}

/// AC-3 unit lock: when the active workflow's directory no longer exists,
/// `reload_with_rediscovery` installs a synthetic empty `OverviewState` with
/// `last_refresh_error` set and does not panic. This pins the "removed active"
/// branch without depending on the live `notify` backend.
#[test]
fn reload_with_rediscovery_handles_removed_active_workflow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let scan_root = dir.path();

    // Two minimal workflows under the scan root so re-discovery finds the
    // surviving sibling after we remove the active one.
    let alpha = scan_root.join("docs/alpha");
    let beta = scan_root.join("docs/beta");
    for (root, slug) in [(&alpha, "alpha"), (&beta, "beta")] {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(
            root.join("README.md"),
            "---\ncommissioned-by: spacedock@0.10.1\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n",
        )
        .unwrap();
        std::fs::write(
            root.join(format!("task-{slug}.md")),
            format!("---\nid: 001\ntitle: T{slug}\nstatus: plan\n---\n\nbody\n"),
        )
        .unwrap();
    }

    let workflows = spacetop_core::discovery::discover_workflows(scan_root).expect("discover");
    assert_eq!(workflows.len(), 2);
    let first_root = workflows[0].root.clone();
    let initial = super::OverviewState::load(first_root.clone()).expect("load initial");
    let session =
        super::OverviewSession::from_discovery(scan_root.to_path_buf(), workflows, 0, initial);
    let mut app = super::App::from_session(session);

    // Remove the active workflow's directory.
    std::fs::remove_dir_all(&first_root).expect("remove active workflow");

    // Reload: rediscovery prunes the gone workflow, surviving sibling becomes
    // active and is loaded.
    app.reload_with_rediscovery()
        .expect("reload should not error");

    let session = app.as_session().expect("session");
    assert_eq!(
        session.discovery().len(),
        1,
        "removed workflow must drop from discovery"
    );
    assert_ne!(
        session.active_dir(),
        first_root.as_path(),
        "active must move off the removed workflow"
    );
    // The surviving workflow's snapshot has the expected stages.
    let snapshot = app.snapshot();
    let stages: Vec<String> = snapshot
        .definition
        .stages
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(stages, vec!["plan", "done"]);
}
