use crate::app::{summary::Presentation, OverviewState};
use ratatui::{
    layout::Rect,
    prelude::Frame,
    widgets::{Paragraph, Wrap},
};

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &OverviewState,
    presentation: Presentation,
    gate_body: &str,
) {
    // Request state and its action remain visible independently of prose scroll.
    let status_height = area.height.min(2);
    let status_area = Rect {
        height: status_height,
        ..area
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{}\n{}",
            presentation.status, presentation.action_hint
        )),
        status_area,
    );
    let body_area = Rect {
        y: area.y + status_height,
        height: area.height.saturating_sub(status_height),
        ..area
    };
    // Plain strings only. Generated Markdown links and escapes have no active
    // interpretation here; recorded metadata remains in the existing header.
    let text = format!("{}\n\n{}", presentation.text, gate_body);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let count = paragraph.line_count(body_area.width);
    state
        .max_preview_scroll
        .set(count.saturating_sub(body_area.height as usize));
    state.preview_viewport_height.set(body_area.height as usize);
    state.max_preview_scroll_x.set(0);
    frame.render_widget(
        paragraph.scroll((
            state
                .preview_scroll()
                .min(state.max_preview_scroll.get())
                .min(u16::MAX as usize) as u16,
            0,
        )),
        body_area,
    );
}
