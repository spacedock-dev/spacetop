mod activity;
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
mod metrics;
mod picker;
mod preview;
mod relations;
mod search;
mod tabs;
mod timeline;

use crossterm::event::Event;
#[cfg(test)]
use ratatui::style::Color;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Frame,
    widgets::{Clear, Paragraph},
};
use spacetop_core::config::SpacetopConfig;

use crate::app::{App, AppMode, OverviewSession, ResolvedKeymap};
use graph::render_stage_graph;
use layout::{picker_centered, preview_placement, split_content};

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
    let warning_messages = app.warning_messages();
    match app.mode() {
        AppMode::Picker(state) => {
            // Picker overlays a centered dialog; the dashboard responsive-
            // width rule does not apply to picker (it's a one-off chooser).
            let inner = picker_centered(frame.area(), state);
            picker::render_in(frame, inner, state);
        }
        AppMode::Overview(session) => {
            render_overview(
                frame,
                frame.area(),
                app.config(),
                app.keymap(),
                &warning_messages,
                session,
            );
        }
        AppMode::PickerOverlay { underlying, picker } => {
            // Draw the underlying overview at full width, then overlay a
            // centered picker dialog atop a `Clear` widget.
            render_overview(
                frame,
                frame.area(),
                app.config(),
                app.keymap(),
                &warning_messages,
                underlying,
            );
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
        AppMode::Search { underlying, state } => {
            render_overview(
                frame,
                frame.area(),
                app.config(),
                app.keymap(),
                &warning_messages,
                underlying,
            );
            search::render_overlay(frame, frame.area(), underlying, state);
        }
        AppMode::Timeline {
            underlying,
            entity_id,
            scroll,
        } => {
            timeline::render_in(frame, frame.area(), underlying, entity_id, *scroll);
        }
        AppMode::Metrics { underlying, scroll } => {
            metrics::render_in(frame, frame.area(), underlying, *scroll);
        }
        AppMode::Activity { underlying, scroll } => {
            activity::render_in(frame, frame.area(), underlying, *scroll);
        }
        AppMode::Relations {
            underlying,
            entity_id,
            scroll,
        } => {
            relations::render_in(frame, frame.area(), underlying, entity_id, *scroll);
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

fn render_overview(
    frame: &mut Frame<'_>,
    area: Rect,
    config: &SpacetopConfig,
    keymap: &ResolvedKeymap,
    warnings: &[String],
    session: &OverviewSession,
) {
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

    // Record the geometry the widgets are drawn with as render-facts on
    // app state (Cell interior mutability, same precedent as
    // `max_preview_scroll`). Mouse hit-testing reads only these, so click
    // coordinates cannot drift from drawn rows by construction.
    state.content_rect.set(content_area);
    if state.preview_open() {
        let placement = preview_placement(dashboard_area);
        let (list_area, preview_area) =
            split_content(content_area, placement, state.split_percent(placement));
        state.preview_rect.set(preview_area);
        list::render_task_list(frame, list_area, config, state);
        preview::render_preview(frame, preview_area, state, placement);
    } else {
        state.preview_rect.set(Rect::default());
        list::render_task_list(frame, content_area, config, state);
    }

    footer::render_status_footer(frame, footer_area, config, keymap, warnings, session);
}

#[cfg(test)]
mod tests;
