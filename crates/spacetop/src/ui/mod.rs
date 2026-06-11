pub(crate) mod color;
mod definition;
mod diff;
pub(crate) mod footer;
mod graph;
mod header;
mod help;
mod layout;
mod list;
mod markdown;
mod picker;
mod preview;
mod tabs;

use crossterm::event::Event;
#[cfg(test)]
use ratatui::style::Color;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Frame,
    widgets::{Clear, Paragraph},
};

use crate::app::{App, AppMode, OverviewSession};
use graph::render_stage_graph;
use layout::{picker_centered, preview_placement, PreviewPlacement};

#[cfg(test)]
pub(crate) use list::phase_col;
#[cfg(test)]
pub(crate) use preview::fit_path_to_width;

pub type TerminalEvent = Event;

pub fn render_placeholder(frame: &mut Frame<'_>) {
    frame.render_widget(
        Paragraph::new("SpaceTop workflow overview is not implemented yet."),
        frame.area(),
    );
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.mode() {
        AppMode::Picker(state) => {
            // Picker overlays a centered dialog; the dashboard responsive-
            // width rule does not apply to picker (it's a one-off chooser).
            let inner = picker_centered(frame.area(), state);
            picker::render_in(frame, inner, state);
        }
        AppMode::Overview(session) => {
            render_overview(frame, frame.area(), session);
        }
        AppMode::PickerOverlay { underlying, picker } => {
            // Draw the underlying overview at full width, then overlay a
            // centered picker dialog atop a `Clear` widget.
            render_overview(frame, frame.area(), underlying);
            let inner = picker_centered(frame.area(), picker);
            frame.render_widget(Clear, inner);
            picker::render_in(frame, inner, picker);
        }
        AppMode::Definition { underlying, scroll } => {
            // Full-pane workflow definition view scoped to the active tab.
            // No tab strip, no graph ribbon, no status footer.
            let definition = underlying.active_state().definition();
            definition::render_in(frame, frame.area(), definition, *scroll);
        }
        AppMode::Search { underlying, .. }
        | AppMode::Timeline { underlying, .. }
        | AppMode::Metrics { underlying, .. }
        | AppMode::Activity { underlying, .. }
        | AppMode::Relations { underlying, .. } => {
            render_overview(frame, frame.area(), underlying);
        }
    }

    if app.help_open() {
        help::render_help_popup(frame, frame.area(), app);
    }
}

/// Map a stage name to a stable color. Thin re-export of `domain::stage_color`
/// so existing direct callers in tests keep compiling without path changes.
#[cfg(test)]
pub(crate) fn stage_color(stage_name: &str) -> Color {
    color::to_color(spacetop_core::domain::stage_color(stage_name))
}

/// Assign graph-aware colors to stages. Thin re-export of
/// `domain::assign_stage_colors` for use from tests and legacy callers.
#[cfg(test)]
pub(crate) fn assign_stage_colors(
    stages: &[spacetop_core::domain::StageDefinition],
) -> std::collections::HashMap<String, Color> {
    spacetop_core::domain::assign_stage_colors(stages)
        .into_iter()
        .map(|(k, v)| (k, color::to_color(v)))
        .collect()
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, session: &OverviewSession) {
    let state = session.active_state();
    let show_tabs = session.is_multi();
    let dashboard_area = if show_tabs {
        tabs::render_workflow_tabs_panel(frame, area, session)
    } else {
        area
    };

    // Vertical layout inside the active workflow panel: header bar (1),
    // graph ribbon (7), main content fills the rest, status footer (1 line).
    let constraints = vec![
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Min(0),
        Constraint::Length(1),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(dashboard_area);

    let header_area = chunks[0];
    let graph_area = chunks[1];
    let content_area = chunks[2];
    let footer_area = chunks[3];
    header::render_header_bar(frame, header_area, state);
    render_stage_graph(frame, graph_area, state);

    if state.preview_open() {
        match preview_placement(dashboard_area) {
            PreviewPlacement::Left => {
                let [list_area, preview_area] = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .areas(content_area);
                list::render_task_list(frame, list_area, state);
                preview::render_preview(frame, preview_area, state, PreviewPlacement::Left);
            }
            PreviewPlacement::Bottom => {
                let [list_area, preview_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                    .areas(content_area);
                list::render_task_list(frame, list_area, state);
                preview::render_preview(frame, preview_area, state, PreviewPlacement::Bottom);
            }
        }
    } else {
        list::render_task_list(frame, content_area, state);
    }

    footer::render_status_footer(frame, footer_area, session);
}

#[cfg(test)]
mod tests;
