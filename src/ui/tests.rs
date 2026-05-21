use std::path::PathBuf;

use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier},
    Terminal,
};

use super::{assign_stage_colors, fit_path_to_width, markdown, phase_col, render, stage_color};
use crate::app::App;
use crate::domain::{StageDefinition, WorkItem, WorkflowDefinition, WorkflowSnapshot};

mod chrome;
mod code_blocks;
mod colors;
mod overview;
mod paths;
mod preview;
mod task_list;
mod worktree;

fn app_with_items(items: Vec<WorkItem>) -> App {
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
        },
        items,
    };
    let mut app = App::from_snapshot(root, snapshot);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app
}

fn item(id: &str, title: &str, body: &str) -> WorkItem {
    WorkItem {
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
        },
        items: vec![item(id, title, body)],
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
