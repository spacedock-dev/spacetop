use std::path::PathBuf;

use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier},
    Terminal,
};

use super::{assign_stage_colors, fit_path_to_width, markdown, phase_col, render, stage_color};
use crate::app::{App, OverviewSession, OverviewState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spacetop_core::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};
use spacetop_core::index::{CommitId, CommitTime, StageEvent, WorkflowIndex};
use spacetop_core::query::HistoryUnavailable;

mod chrome;
mod code_blocks;
mod colors;
mod overview;
mod paths;
mod preview;
mod task_list;
mod worktree;

fn app_with_items(items: Vec<Entity>) -> App {
    let root = PathBuf::from("/tmp/spacetop-test");
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            stages: vec![StageDefinition {
                name: "design".to_string(),
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
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items,
        parse_errors: Vec::new(),
    };
    let mut app = App::from_snapshot(root, snapshot);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app
}

fn app_with_active_session_marker(mut items: Vec<Entity>, entity_id: &str) -> App {
    let root = PathBuf::from("/tmp/spacetop-test");
    for item in &mut items {
        if item.id == entity_id && item.worktree.is_none() {
            item.worktree = Some(format!(".worktrees/task-{entity_id}"));
        }
    }
    let snapshot = WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: root.clone(),
            stages: vec![StageDefinition {
                name: "design".to_string(),
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
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items,
        parse_errors: Vec::new(),
    };
    let mut state = OverviewState::from_snapshot(root.clone(), snapshot);
    let repo_root = state.repo_root.clone();
    state.apply_session_activity_result(crate::app::SessionActivityWorkerResult {
        workflow_dir: root.clone(),
        repo_root: repo_root.clone(),
        result: Ok(spacetop_core::domain::SessionScanReport {
            workflow_dir: root,
            repo_root,
            scanned_roots: Vec::new(),
            errors: Vec::new(),
            attributions: vec![spacetop_core::domain::EntitySessionAttribution {
                entity_id: entity_id.to_string(),
                evidence: vec![spacetop_core::domain::AgentSessionEvidence {
                    agent: spacetop_core::domain::AgentKind::Codex,
                    session_id: "session-065".to_string(),
                    display_name: Some("Mendel".to_string()),
                    confidence: spacetop_core::domain::AttributionConfidence::High,
                    run_state: spacetop_core::domain::AgentSessionState::Running,
                    latest_activity_unix: Some(1_718_000_000),
                    matched_worktree: Some(PathBuf::from(format!(".worktrees/task-{entity_id}"))),
                }],
            }],
        }),
    });
    let mut app = App::from_session(OverviewSession::single(state, true));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app
}

fn item(id: &str, title: &str, body: &str) -> Entity {
    Entity {
        path: PathBuf::from(format!("/tmp/{id}.md")),
        id: id.to_string(),
        title: title.to_string(),
        status: "design".to_string(),
        source: Some("test".to_string()),
        started: None,
        completed: None,
        verdict: None,
        score: None,
        worktree: None,
        issue: None,
        pr: None,
        body: body.to_string(),
        worktree_source: None,
        main_body: None,
    }
}

fn snapshot_with_body(id: &str, title: &str, body: &str) -> WorkflowSnapshot {
    WorkflowSnapshot {
        definition: WorkflowDefinition {
            root: PathBuf::from("/tmp/ww-test"),
            stages: vec![StageDefinition {
                name: "design".to_string(),
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
            stage_colors: std::collections::HashMap::new(),
            stage_prose: std::collections::HashMap::new(),
            transitions: Vec::new(),
        },
        items: vec![item(id, title, body)],
        parse_errors: Vec::new(),
    }
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("")
}

fn render_text(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| render(frame, app)).expect("draw");
    buffer_text(terminal.backend().buffer())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn stage_event(entity_id: &str, from: Option<&str>, to: &str, at: i64) -> StageEvent {
    StageEvent {
        entity_id: entity_id.to_string(),
        from: from.map(str::to_string),
        to: to.to_string(),
        at: CommitTime(at),
        commit: CommitId(format!("{at:040}")),
    }
}

fn p3_snapshot() -> WorkflowSnapshot {
    let mut snapshot = snapshot_with_body("050", "Roadmap capability views", "roadmap body");
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
            name: "verify".to_string(),
            initial: false,
            terminal: false,
            gate: true,
            fresh: false,
            feedback_to: Some("plan".to_string()),
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
    snapshot.items[0].status = "plan".to_string();
    snapshot.items[0].issue = Some("https://example.test/issues/50".to_string());
    snapshot.items[0].pr = Some("https://example.test/pulls/50".to_string());
    snapshot.items[0].worktree = Some(".worktrees/p3".to_string());
    snapshot.items.push(Entity {
        path: PathBuf::from("/tmp/051.md"),
        id: "051".to_string(),
        title: "Verify renderer".to_string(),
        status: "verify".to_string(),
        source: Some("test".to_string()),
        started: None,
        completed: None,
        verdict: None,
        score: None,
        worktree: None,
        issue: None,
        pr: None,
        body: "verify body".to_string(),
        worktree_source: None,
        main_body: None,
    });
    snapshot
}

fn app_with_history(result: Result<Vec<StageEvent>, HistoryUnavailable>) -> App {
    let root = PathBuf::from("/tmp/p3-ui");
    let snapshot = p3_snapshot();
    let mut index = WorkflowIndex::from_sources(spacetop_core::sources::WorkflowSources {
        active: snapshot,
        archive: spacetop_core::sources::ArchiveSnapshot::empty(),
    });
    index.replace_history_result(result);
    let mut state = OverviewState::empty(root);
    state.reload_from_index(index);
    App::from_session(OverviewSession::single(state, true))
}

#[test]
fn search_overlay_renders_query_and_matching_entity() {
    let mut app = App::from_snapshot(PathBuf::from("/tmp/p3-ui"), p3_snapshot());
    app.handle_key(key(KeyCode::Char('/')));
    for ch in "roadmap".chars() {
        app.handle_key(key(KeyCode::Char(ch)));
    }

    let text = render_text(&app, 100, 32);

    assert!(text.contains("Search"));
    assert!(text.contains("roadmap"));
    assert!(text.contains("Roadmap capability views"));
}

#[test]
fn command_palette_overlay_renders_commands() {
    let mut app = App::from_snapshot(PathBuf::from("/tmp/p3-ui"), p3_snapshot());
    app.handle_key(key(KeyCode::Char(':')));

    let text = render_text(&app, 100, 32);

    assert!(text.contains("Command"));
    assert!(text.contains("metrics"));
    assert!(text.contains("activity"));
    assert!(text.contains("timeline"));
    assert!(text.contains("relations"));
}

#[test]
fn timeline_view_renders_unavailable_loading_empty_and_events() {
    let mut app = app_with_history(Err(HistoryUnavailable::ShallowClone));
    app.handle_key(key(KeyCode::Char('T')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("Timeline"));
    assert!(text.contains("history unavailable: shallow clone"));

    let reason = HistoryUnavailable::MetadataError {
        path: "docs/workflow/001.md".to_string(),
        message: "missing status".to_string(),
    };
    let message = reason.user_message();
    let mut app = app_with_history(Err(reason));
    app.handle_key(key(KeyCode::Char('T')));
    let text = render_text(&app, 220, 32);
    assert!(text.contains(&message));

    let mut app = app_with_history(Err(HistoryUnavailable::Loading));
    app.handle_key(key(KeyCode::Char('T')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("history is loading"));

    let mut app = app_with_history(Ok(Vec::new()));
    app.handle_key(key(KeyCode::Char('T')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("Timeline"));
    assert!(text.contains("No timeline events"));

    let mut app = app_with_history(Ok(vec![
        stage_event("050", None, "plan", 100),
        stage_event("050", Some("plan"), "verify", 160),
    ]));
    app.handle_key(key(KeyCode::Char('T')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("plan"));
    assert!(text.contains("verify"));
}

#[test]
fn metrics_view_renders_unavailable_and_populated_metrics() {
    let mut app = app_with_history(Err(HistoryUnavailable::ShallowClone));
    app.handle_key(key(KeyCode::Char('M')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("Metrics"));
    assert!(text.contains("history unavailable: shallow clone"));

    let reason = HistoryUnavailable::MetadataError {
        path: "docs/workflow/001.md".to_string(),
        message: "missing status".to_string(),
    };
    let message = reason.user_message();
    let mut app = app_with_history(Err(reason));
    app.handle_key(key(KeyCode::Char('M')));
    let text = render_text(&app, 220, 32);
    assert!(text.contains(&message));

    let mut app = app_with_history(Ok(vec![
        stage_event("050", None, "plan", 100),
        stage_event("050", Some("plan"), "verify", 160),
        stage_event("050", Some("verify"), "done", 220),
        stage_event("051", None, "verify", 180),
    ]));
    app.handle_key(key(KeyCode::Char('M')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("completed"));
    assert!(text.contains("throughput"));
    assert!(text.contains("stage dwell"));
    assert!(text.contains("cycle time"));
    assert!(text.contains("WIP"));
    assert!(text.contains("verify"));
}

#[test]
fn activity_view_renders_unavailable_and_newest_events_first() {
    let mut app = app_with_history(Err(HistoryUnavailable::ShallowClone));
    app.handle_key(key(KeyCode::Char('A')));
    let text = render_text(&app, 100, 32);
    assert!(text.contains("Activity"));
    assert!(text.contains("history unavailable: shallow clone"));

    let reason = HistoryUnavailable::MetadataError {
        path: "docs/workflow/001.md".to_string(),
        message: "missing status".to_string(),
    };
    let message = reason.user_message();
    let mut app = app_with_history(Err(reason));
    app.handle_key(key(KeyCode::Char('A')));
    let text = render_text(&app, 220, 32);
    assert!(text.contains(&message));

    let mut app = app_with_history(Ok(vec![
        stage_event("050", None, "plan", 100),
        stage_event("051", None, "verify", 200),
    ]));
    app.handle_key(key(KeyCode::Char('A')));
    let rendered = render_text(&app, 100, 32);
    let newer = rendered.find("051").expect("newer event");
    let older = rendered.find("050").expect("older event");
    assert!(newer < older, "newest activity should render first");
    assert!(rendered.contains("verify"));
}

#[test]
fn relations_view_renders_typed_entity_details() {
    let mut app = app_with_history(Ok(Vec::new()));
    app.handle_key(key(KeyCode::Char('R')));

    let text = render_text(&app, 100, 32);

    assert!(text.contains("Relations"));
    assert!(text.contains("050"));
    assert!(text.contains("Roadmap capability views"));
    assert!(text.contains("issue"));
    assert!(text.contains("pr"));
    assert!(text.contains("feedback-to"));
    assert!(text.contains(".worktrees/p3"));
}

fn find_text_starting_after(buffer: &ratatui::buffer::Buffer, needle: &str, min_x: u16) -> bool {
    find_text(buffer, needle)
        .into_iter()
        .any(|(x, _y)| x >= min_x)
}

fn find_styled_text<F>(buffer: &ratatui::buffer::Buffer, needle: &str, predicate: F) -> bool
where
    F: Fn(ratatui::style::Style) -> bool,
{
    let chars: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
    find_text(buffer, needle).into_iter().any(|(x, y)| {
        chars
            .iter()
            .enumerate()
            .all(|(offset, _)| predicate(buffer[(x + offset as u16, y)].style()))
    })
}

fn find_text(buffer: &ratatui::buffer::Buffer, needle: &str) -> Vec<(u16, u16)> {
    let chars: Vec<String> = needle.chars().map(|c| c.to_string()).collect();
    let mut matches = Vec::new();
    if chars.is_empty() {
        return matches;
    }
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            if x + chars.len() as u16 > buffer.area.width {
                continue;
            }
            if chars
                .iter()
                .enumerate()
                .all(|(i, c)| buffer[(x + i as u16, y)].symbol() == c.as_str())
            {
                matches.push((x, y));
            }
        }
    }
    matches
}

// --- Help popup behaviour ---
