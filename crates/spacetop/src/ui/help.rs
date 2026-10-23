use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::App;

pub(super) fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let is_multi = app.as_session().map(|s| s.is_multi()).unwrap_or(false);
    let preview_open = app
        .as_session()
        .map(|s| s.active_state().preview_open())
        .unwrap_or(false);
    let keymap = app.keymap();
    let mut lines = Vec::new();
    if let Some(diagnostic) = app
        .as_session()
        .and_then(|session| session.active_state().topology_diagnostic())
    {
        lines.extend([
            Line::from("Progress files"),
            Line::from(diagnostic.explanation()),
            Line::from(diagnostic.guidance()),
            Line::from("Technical details:"),
            Line::from(diagnostic.to_string()),
        ]);
        if let crate::app::StateTopologyDiagnostic::Unverified { declared_path, .. } = &diagnostic {
            lines.push(Line::from(format!(
                "Configured state location: {}",
                declared_path.display()
            )));
        }
        lines.push(Line::from(""));
    }
    lines.extend([
        Line::from(Span::styled(
            "Spacetop keymap",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("Up / k", "move selection up"),
        key_line("Down / j", "move selection down"),
        key_line("Home", "jump to first item"),
        key_line("End", "jump to last item"),
        key_line("Enter", "toggle preview mode"),
        key_line("a", "toggle active / archived view"),
        key_line("s", "cycle sort mode (when preview closed)"),
        key_line(keymap.search.label(), "search entities"),
        key_line(keymap.command.label(), "open command palette"),
        key_line("B", "browse gate briefings (h: correction rounds)"),
        key_line("D", "open workflow definition"),
        key_line("Y", "sync verified workflow state (fast-forward)"),
        key_line("?", "toggle this help popup"),
        key_line("Esc", "close help"),
    ]);
    if preview_open {
        lines.push(key_line("Space / PgDn", "page preview down"));
        lines.push(key_line("b / PgUp", "page preview up"));
        lines.push(key_line("g / G", "preview top / bottom"));
        lines.push(key_line("w", "toggle word wrap"));
        lines.push(key_line("o", "open file in $EDITOR"));
    } else {
        lines.push(key_line("PageUp", "page list up"));
        lines.push(key_line("PageDown", "page list down"));
        lines.push(key_line(
            keymap.timeline.label(),
            "entity timeline (preview closed)",
        ));
        lines.push(key_line(
            keymap.metrics.label(),
            "metrics view (preview closed)",
        ));
        lines.push(key_line(
            keymap.activity.label(),
            "activity feed (preview closed)",
        ));
        lines.push(key_line(
            keymap.relations.label(),
            "entity relations (preview closed)",
        ));
    }
    if preview_open {
        lines.push(key_line("\u{2192} / Right", "scroll preview right"));
        lines.push(key_line("\u{2190} / Left", "scroll preview left"));
    } else if is_multi {
        lines.push(key_line("\u{2192} / Right", "switch to next workflow"));
        lines.push(key_line("\u{2190} / Left", "switch to previous workflow"));
    }
    if is_multi {
        lines.push(key_line("P", "pick workflow"));
    }
    // Mouse block (task 057). The Shift+drag line is load-bearing: the app
    // holds mouse capture for its lifetime and relies on the standard
    // terminal convention (iTerm2 / Terminal.app / kitty / WezTerm) that
    // Shift+left-drag bypasses capture for native selection/copy. The
    // string is pinned by a chrome test — update both together.
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Mouse",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(key_line("Click", "select row + open preview"));
    lines.push(key_line("Double-click ID", "copy full ID"));
    lines.push(key_line("Wheel", "scroll panel under cursor"));
    lines.push(key_line("Drag divider", "resize list/preview split"));
    lines.push(key_line("Shift+drag", "native terminal text selection"));
    lines.extend([
        Line::from(""),
        Line::from("  needs-attention: focus recorded reasons; all-tasks: leave focus"),
        Line::from("  Enter: report/details; B: review evidence"),
        Line::from("  Missing or unverified information is not a recorded blocker."),
        Line::from("  Activity, age and process absence never imply attention."),
        Line::from("  a leaves attention focus and opens archives."),
        Line::from("Status vocabulary"),
        Line::from("awaiting-captain: waiting for your decision."),
        Line::from("approved-awaiting-merge: approval is recorded; merging is still pending."),
        Line::from("FO (first officer) means workflow coordinator: the agent that coordinates workflow steps."),
        Line::from("A recorded running · FO value identifies that handler; it does not mean the task is complete or ready for your decision."),
        Line::from("Idle or missing activity does not prove task completion or coordinator health."),
        Line::from("The Activity view shows recorded stage changes, not current coordinator status."),
        Line::from("Decide in your Spacedock workflow session, not in Spacetop. The action destination is not known here; check with your workflow coordinator."),
        Line::from("B inspects recorded evidence only. It does not authenticate recorded actors. Spacetop cannot approve requests, move tasks, merge work or repair checkouts."),
    ]);

    if let Some(freshness) = app.freshness() {
        lines.push(Line::from("Freshness (this session only)"));
        lines.push(Line::from("Local read: last successful read of local workflow files. Item parse warnings still apply. Local changes do not imply Git sync."));
        lines.push(Line::from("Git definition / Git state: last successful explicit Y sync of each checkout. Unknown means not observed this session, including after restart."));
        lines.push(Line::from("A successful Git sync does not prove the remote has no newer work. Failed or skipped attempts preserve older successes. Same checkout has no separate state sync."));
        let mut observations = vec![
            ("Local read", &freshness.local),
            ("Git definition", &freshness.definition),
        ];
        if freshness.split_root {
            observations.push(("Git state", &freshness.state));
        } else {
            lines.push(Line::from("Git state: same checkout"));
        }
        for (label, observation) in observations {
            lines.push(Line::from(format!(
                "{label}: {}",
                super::footer::observation_age(observation, std::time::Instant::now())
            )));
            match &observation.latest {
                crate::app::freshness::Attempt::Failed(reason) => {
                    lines.push(Line::from(format!("Latest failed: {reason}")))
                }
                crate::app::freshness::Attempt::Skipped(reason) => {
                    lines.push(Line::from(format!("Latest skipped: {reason}")))
                }
                _ => {}
            }
        }
    }

    let popup_w = area.width.min(64);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let content_height = paragraph.line_count(popup_w.saturating_sub(2));
    let popup_h = content_height
        .saturating_add(3)
        .min(usize::from(area.height)) as u16;
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_w) / 2,
        y: area.y + area.height.saturating_sub(popup_h) / 2,
        width: popup_w,
        height: popup_h,
    };
    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup);
    let body = Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    };
    let max_scroll = content_height
        .saturating_sub(usize::from(body.height))
        .min(usize::from(u16::MAX));
    let scroll = app.help_scroll.get().min(max_scroll);
    app.help_scroll.set(scroll);
    app.help_max_scroll.set(max_scroll);
    app.help_page_height.set(usize::from(body.height));
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    frame.render_widget(paragraph.scroll((scroll as u16, 0)), body);
    if inner.height > 0 {
        let hint = Rect {
            y: inner.y + inner.height - 1,
            height: 1,
            ..inner
        };
        frame.render_widget(
            Paragraph::new("?/Esc: close | Up/Down PgUp/PgDn Home/End: scroll"),
            hint,
        );
    }
}

fn key_line(key: &str, description: &str) -> Line<'static> {
    Line::from(format!("  {key:<14} {description}"))
}
