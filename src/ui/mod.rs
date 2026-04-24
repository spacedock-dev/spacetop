use crossterm::event::Event;
use ratatui::{prelude::Frame, widgets::Paragraph};

pub type TerminalEvent = Event;

pub fn render_placeholder(frame: &mut Frame<'_>) {
    frame.render_widget(
        Paragraph::new("SpaceTop workflow overview is not implemented yet."),
        frame.area(),
    );
}
