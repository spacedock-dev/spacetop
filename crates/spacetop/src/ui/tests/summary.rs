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

#[test]
fn v090_2_progress_and_cancel_stay_visible_at_40x15_after_scrolling() {
    let (mut app, _dir) = fixture();
    key(&mut app, KeyCode::Char('S'));
    key(&mut app, KeyCode::Enter);
    let request = app.summary.take_request().unwrap();
    for scroll in [None, Some(KeyCode::Char('G')), Some(KeyCode::PageDown)] {
        if let Some(code) = scroll {
            key(&mut app, code);
        }
        let visible = frame(&app, 40, 15);
        assert!(
            visible.contains("Generating"),
            "active progress must be visible: {visible}"
        );
        assert!(
            visible.contains("Esc: cancel"),
            "cancel hint must stay visible: {visible}"
        );
        assert!(app.summary.take_request().is_none());
    }
    app.complete_summary(SummaryCompletion {
        nonce: request.nonce,
        key: request.snapshot.key,
        result: Ok("Completed summary".into()),
    });
    let visible = frame(&app, 40, 15);
    assert!(!visible.contains("Generating"));
    assert!(visible.contains("Summary ready"));
    key(&mut app, KeyCode::Char('O'));
    assert!(app.as_overview().unwrap().preview_open());
}

#[test]
fn v090_2_cancel_and_error_stop_fixed_progress_while_scrolled() {
    for failure in [None, Some(spacetop_core::summary::SummaryError::Timeout)] {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        let request = app.summary.take_request().unwrap();
        frame(&app, 40, 15);
        key(&mut app, KeyCode::Char('G'));
        if let Some(error) = failure {
            app.complete_summary(SummaryCompletion {
                nonce: request.nonce,
                key: request.snapshot.key,
                result: Err(error),
            });
        } else {
            key(&mut app, KeyCode::Esc);
        }
        let visible = frame(&app, 40, 15);
        assert!(!visible.contains("Generating"));
        assert!(
            visible.contains("Stopped"),
            "terminal state must be visible: {visible}"
        );
        assert!(!visible.contains("Esc: cancel"));
        assert!(app.summary.take_request().is_none());
    }
}
