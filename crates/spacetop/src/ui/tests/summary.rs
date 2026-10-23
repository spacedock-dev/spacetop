use super::*;
use crate::app::summary::{
    tests::{fixture, key},
    SummaryCompletion,
};

fn frame(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    buffer_text(terminal.backend().buffer())
}
#[test]
fn summary_disclosure_running_complete_and_original_render() {
    let (mut app, _dir) = fixture();
    key(&mut app, KeyCode::Char('S'));
    let disclosure = frame(&app, 160, 55);
    assert!(disclosure.contains("usage charges"));
    assert!(disclosure.contains("claude-haiku-5-5"));
    assert!(disclosure.contains("status:"));
    key(&mut app, KeyCode::Enter);
    let request = app.summary.take_request().unwrap();
    let running = frame(&app, 160, 55);
    assert!(running.contains("Generating summary"));
    app.complete_summary(SummaryCompletion {
        nonce: request.nonce,
        key: request.snapshot.key,
        result: Ok("Reported work; review rejected. Human check not recorded.".into()),
    });
    let completed = frame(&app, 160, 55);
    assert!(!completed.contains("Generating summary"));
    assert!(completed.contains("review rejected"));
    assert!(completed.contains("status:"));
    key(&mut app, KeyCode::Char('O'));
    assert!(frame(&app, 160, 55).contains("Rejected review"));
    // Narrow rendering remains bounded and scrollable, without a second request.
    key(&mut app, KeyCode::Char('S'));
    assert!(frame(&app, 40, 15).contains("AI Summary"));
    assert!(app.summary.take_request().is_none());
}
