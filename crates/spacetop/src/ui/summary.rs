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
    // Plain strings only. Generated Markdown links and escapes have no active
    // interpretation here; recorded metadata remains in the existing header.
    let text = format!("{}\n\n{}", presentation.text, gate_body);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let count = paragraph.line_count(area.width);
    state
        .max_preview_scroll
        .set(count.saturating_sub(area.height as usize));
    state.preview_viewport_height.set(area.height as usize);
    state.max_preview_scroll_x.set(0);
    frame.render_widget(
        paragraph.scroll((
            state
                .preview_scroll()
                .min(state.max_preview_scroll.get())
                .min(u16::MAX as usize) as u16,
            0,
        )),
        area,
    );
}
