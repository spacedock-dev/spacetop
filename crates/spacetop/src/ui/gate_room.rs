use crate::app::gate_room::{GateBrowser, RoomFocus};
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

pub(super) fn render(frame: &mut Frame<'_>, browser: &GateBrowser) {
    let area = frame.area();
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    let status = if browser.preview.is_some() {
        "Item SHA-256 verified"
    } else if browser.view.verified() {
        "Bound Briefing verified"
    } else {
        "UNVERIFIED"
    };
    frame.render_widget(
        Paragraph::new(format!("{status} | read-only")).style(Style::default().fg(
            if browser.view.verified() {
                Color::Green
            } else {
                Color::Yellow
            },
        )),
        layout[0],
    );
    let content = browser.content();
    let body = if area.width >= 70 {
        let columns = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(layout[1]);
        let selected = browser
            .attempts
            .iter()
            .position(|(r, a)| {
                browser
                    .selected
                    .as_ref()
                    .is_some_and(|(g, t)| g == &r.id && t == &a.id)
            })
            .unwrap_or(0);
        let start = selected.saturating_sub(columns[0].height.saturating_sub(2) as usize);
        let attempts = browser
            .attempts
            .iter()
            .skip(start)
            .map(|(r, a)| {
                format!(
                    "{} {} / {}",
                    if browser
                        .selected
                        .as_ref()
                        .is_some_and(|(g, t)| g == &r.id && t == &a.id)
                    {
                        ">"
                    } else {
                        " "
                    },
                    r.stage,
                    a.id
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        frame.render_widget(
            Paragraph::new(format!("Attempts\n{attempts}")).wrap(Wrap { trim: false }),
            columns[0],
        );
        columns[1]
    } else {
        layout[1]
    };
    let lines = if browser.wrap {
        wrap_content(&content, body.width)
    } else {
        content.lines().map(str::to_owned).collect::<Vec<_>>()
    };
    let scroll = browser
        .scroll
        .min(lines.len().saturating_sub(body.height as usize));
    frame.render_widget(
        Paragraph::new(lines.into_iter().map(Line::raw).collect::<Vec<_>>())
            .scroll((scroll.min(u16::MAX as usize) as u16, 0)),
        body,
    );
    let footer = if area.width < 35 {
        "Esc: back | PgUp/Dn".into()
    } else {
        format!(
            "Esc: back | Tab: {:?} | j/k select | Enter item | PgUp/Dn | w wrap",
            if browser.focus == RoomFocus::Attempts {
                RoomFocus::Items
            } else {
                RoomFocus::Attempts
            }
        )
    };
    frame.render_widget(Paragraph::new(footer), layout[2]);
}

// Hard-wrap the read-only text once so scroll limits and drawn rows agree,
// including Unicode and long URI segments. No markdown/provider interpretation.
fn wrap_content(content: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    let mut result = Vec::new();
    for line in content.lines() {
        let mut row = String::new();
        let mut columns = 0;
        for ch in line.chars() {
            let size = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if !row.is_empty() && columns + size > width {
                result.push(std::mem::take(&mut row));
                columns = 0;
            }
            row.push(ch);
            columns += size;
        }
        result.push(row);
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::app::{gate_room::tests::setup, App};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    fn text(app: &App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| crate::ui::render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    }
    #[test]
    fn room_details_and_failure_exit_remain_usable_at_three_sizes() {
        let (_temp, snapshot) = setup();
        let root = snapshot.definition.root.clone();
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
        let output = text(&app, 80, 24);
        for part in [
            "Bound Briefing verified",
            "Question:",
            "Should this exact candidate",
            "Recorded Approve",
            "agent:first-officer",
            "Conn quote:",
            "Application: Pending",
            "Artifact:",
            "Esc: back",
        ] {
            assert!(output.contains(part), "missing {part}: {output}");
        }

        for (w, h) in [(80, 24), (40, 12), (20, 6)] {
            assert!(text(&app, w, h).contains("Esc: back"));
        }
        std::fs::remove_file(root.join("room/index.json")).unwrap();
        app.reload_from_snapshot(snapshot);
        for (w, h) in [(80, 24), (40, 12), (20, 6)] {
            let output = text(&app, w, h);
            assert!(output.contains("UNVERIFIED"));
            assert!(output.contains("Missing:"));
            assert!(output.contains("Esc: back"));
        }
    }
}

#[cfg(test)]
mod unicode_tests {
    use crate::app::{gate_room::tests::setup, App};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    use spacetop_core::domain::GateData;
    #[test]
    fn long_unicode_question_reason_and_links_wrap_and_scroll() {
        let (_temp, mut snapshot) = setup();
        let path = snapshot.definition.root.join("room/index.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        json["question"] = format!("請檢查 candidate 🙂 {}", "證據很長".repeat(50)).into();
        json["artifacts"][0]["uri"] =
            format!("https://example.invalid/{}", "參考".repeat(50)).into();
        let bytes = serde_json::to_vec(&json).unwrap();
        std::fs::write(path, &bytes).unwrap();
        let GateData::Valid { document, .. } = &mut snapshot.items[0].gates else {
            panic!()
        };
        let attempt = &mut document.records[0].attempts[1];
        attempt.briefing.digest =
            spacetop_core::parser::gate_room::canonical_digest(&bytes).unwrap();
        attempt.resolution.as_mut().unwrap().reason = "理由 🙂".repeat(100);
        let mut app = App::from_snapshot(snapshot.definition.root.clone(), snapshot);
        app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
        for (width, height) in [(80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Esc: back"));
            assert!(text.contains("Question:"));
        }
        for _ in 0..20 {
            app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
    }
}
