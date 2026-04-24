mod graph;
mod picker;

use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Frame, Line, Modifier, Span, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, AppMode, OverviewState};
use graph::render_stage_graph;

pub type TerminalEvent = Event;

pub fn render_placeholder(frame: &mut Frame<'_>) {
    frame.render_widget(
        Paragraph::new("SpaceTop workflow overview is not implemented yet."),
        frame.area(),
    );
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.mode() {
        AppMode::Picker(state) => picker::render(frame, state),
        AppMode::Overview(state) => render_overview(frame, state),
    }
}

fn render_overview(frame: &mut Frame<'_>, state: &OverviewState) {
    let [graph_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .areas(frame.area());
    let [list_area, preview_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .areas(content_area);

    render_stage_graph(frame, graph_area, state);
    frame.render_widget(task_list(state), list_area);
    frame.render_widget(preview(state), preview_area);
}

fn task_list(app: &OverviewState) -> Paragraph<'_> {
    let lines = if app.snapshot().items.is_empty() {
        vec![Line::from("No work items found.")]
    } else {
        app.snapshot()
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let marker = if index == app.selected_index() {
                    ">"
                } else {
                    " "
                };
                let line = format!("{marker} {} [{}] {}", item.id, item.status, item.title);
                if index == app.selected_index() {
                    Line::from(Span::styled(
                        line,
                        Style::default().add_modifier(Modifier::REVERSED),
                    ))
                } else {
                    Line::from(line)
                }
            })
            .collect()
    };

    Paragraph::new(lines)
        .block(Block::default().title("Tasks").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

fn preview(app: &OverviewState) -> Paragraph<'_> {
    let Some(item) = app.selected_item() else {
        return Paragraph::new("Select a work item to inspect it.")
            .block(Block::default().title("Preview").borders(Borders::ALL));
    };

    let score = item
        .score
        .map(|score| format!("{score:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    let source = item.source.as_deref().unwrap_or("n/a");
    let body_excerpt = item
        .body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("\n");

    let lines = vec![
        Line::from(Span::styled(
            item.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("status: {}", item.status)),
        Line::from(format!("score: {score}")),
        Line::from(format!("source: {source}")),
        Line::from(format!("path: {}", item.path.display())),
        Line::from(""),
        Line::from(body_excerpt),
    ];

    Paragraph::new(lines)
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{backend::TestBackend, Terminal};

    use super::render;
    use crate::app::App;

    #[test]
    fn renders_real_workflow_summary_task_list_and_preview() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev");
        let app = App::load(root).expect("workflow should load");
        let selected = app
            .selected_item()
            .expect("real workflow has a selected item");
        let mut terminal =
            Terminal::new(TestBackend::new(140, 30)).expect("test terminal should be created");

        terminal
            .draw(|frame| render(frame, &app))
            .expect("render should succeed");

        let rendered = buffer_text(terminal.backend().buffer());
        // The graph block carries the Workflow title and each stage name.
        assert!(rendered.contains("Workflow"));
        for stage in &app.snapshot().definition.stages {
            assert!(
                rendered.contains(stage.name.as_str()),
                "missing stage name {}",
                stage.name
            );
        }
        assert!(rendered.contains(&selected.title));
        assert!(rendered.contains(&format!("status: {}", selected.status)));
        assert!(rendered.contains(&format!(
            "score: {}",
            selected
                .score
                .map(|score| format!("{score:.2}"))
                .unwrap_or_else(|| "n/a".to_string())
        )));
        assert!(rendered.contains(&format!(
            "source: {}",
            selected.source.as_deref().unwrap_or("n/a")
        )));
        assert!(rendered.contains("Build the first read-only"));
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }
}
