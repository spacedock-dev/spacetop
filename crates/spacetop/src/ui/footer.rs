use ratatui::{
    layout::{Alignment, Rect},
    prelude::{Frame, Line, Span, Style},
    style::Color,
    widgets::Paragraph,
};
use spacetop_core::config::SpacetopConfig;

use crate::app::{CopyFeedback, OverviewSession, ResolvedKeymap, SyncStatus};

/// Marker glyph prefixed to the sync-failed pill label, mirroring the
/// `SUCCESS_MARKER` on success so failure and success read symmetrically.
const SYNC_FAIL_MARKER: char = '\u{26A0}';

/// Marker glyph prefixed to the sync-succeeded pill labels so success is
/// distinguishable from neutral hints without relying on color alone.
const SUCCESS_MARKER: char = '\u{2713}';

/// One-line status footer at the bottom of the dashboard. Each hint is
/// rendered as a pill-style styled span with a subtle background. Pills
/// carry their own foreground color (`status_footer_hints`), so the sync
/// pill reflects its outcome while the neutral key hints stay white. The
/// exact key list adapts to single vs multi sessions.
pub(super) fn render_status_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    config: &SpacetopConfig,
    keymap: &ResolvedKeymap,
    warnings: &[String],
    session: &OverviewSession,
    copy_feedback: Option<CopyFeedback>,
) {
    let hints = status_footer_hints_with_keymap_and_copy(session, keymap, warnings, copy_feedback);
    let pill_bg = crate::ui::color::footer_bg(config);
    let sep_style = Style::default();
    let mut spans: Vec<Span<'_>> = Vec::new();
    for (i, (label, color)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", sep_style));
        }
        let style = Style::default().fg(*color).bg(pill_bg);
        spans.push(Span::styled(label.clone(), style));
    }

    let para = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

/// Build the ordered footer pills for the active session, each paired with
/// its foreground color. The first pill is the sync-status pill (`Syncing…`
/// / `✓ Synced …` / `⚠ Sync failed: …` / `Sync unavailable: …`) colored by
/// outcome when a sync has been attempted, followed by the parse-error
/// count (`⚠ N broken`, red) when any per-entity parse failures are present;
/// the remainder are the static key hints, which stay neutral white.
#[allow(dead_code)]
pub(crate) fn status_footer_hints(session: &OverviewSession) -> Vec<(String, Color)> {
    status_footer_hints_with_keymap(session, &ResolvedKeymap::default(), &[])
}

pub(crate) fn status_footer_hints_with_keymap(
    session: &OverviewSession,
    keymap: &ResolvedKeymap,
    warnings: &[String],
) -> Vec<(String, Color)> {
    status_footer_hints_with_keymap_and_copy(session, keymap, warnings, None)
}

fn status_footer_hints_with_keymap_and_copy(
    session: &OverviewSession,
    keymap: &ResolvedKeymap,
    warnings: &[String],
    copy_feedback: Option<CopyFeedback>,
) -> Vec<(String, Color)> {
    let preview_open = session.active_state().preview_open();
    let mut hints: Vec<(String, Color)> = Vec::new();
    match copy_feedback {
        Some(CopyFeedback::Succeeded) => {
            hints.push(("\u{2713} ID copied".to_string(), Color::Green));
        }
        Some(CopyFeedback::Failed) => {
            hints.push(("\u{26A0} ID copy failed".to_string(), Color::Red));
        }
        None => {}
    }
    for warning in warnings {
        hints.push((format!("\u{26A0} {warning}"), Color::Yellow));
    }
    if let Some(diagnostic) = session.active_state().topology_diagnostic() {
        hints.push((format!("\u{26A0} {diagnostic}"), Color::Yellow));
    }
    let sync_status = session.active_state().sync_status();
    if let Some(label) = sync_pill_label(sync_status) {
        // `sync_pill_label` returned `Some`, so `sync_status` is `Some`.
        let color = sync_status.map(sync_pill_color).unwrap_or(Color::White);
        hints.push((label, color));
    }
    let broken_count = session.active_state().parse_errors().len();
    if broken_count > 0 {
        hints.push((format!("\u{26A0} {broken_count} broken"), Color::Red));
    }
    hints.push(("?: help".to_string(), Color::White));
    hints.push(("q: quit".to_string(), Color::White));
    if !preview_open && session.is_multi() {
        hints.push((
            "\u{2190}/\u{2192}: switch workflow".to_string(),
            Color::White,
        ));
    }
    if session.is_multi() {
        hints.push(("P: pick workflow".to_string(), Color::White));
    }
    hints.push(("\u{23CE}: toggle preview".to_string(), Color::White));
    hints.push(("a: archive".to_string(), Color::White));
    if preview_open {
        // One compact scroll pill advertises the real keys; the full vocabulary
        // (incl. \u{2190}/\u{2192} horizontal scroll) lives in the help popup so this
        // single center-aligned line stays within ~80 cols.
        hints.push(("scroll: Space/b PgUp/Dn g/G".to_string(), Color::White));
        hints.push(("w: word wrap".to_string(), Color::White));
    } else {
        hints.push(("PgUp/PgDn: page list".to_string(), Color::White));
        hints.push(("s: sort".to_string(), Color::White));
        hints.push((key_hint(keymap.search.label(), "search"), Color::White));
        hints.push((key_hint(keymap.command.label(), "command"), Color::White));
        hints.push((
            format!(
                "{}/{}/{}/{}: views",
                keymap.timeline.label(),
                keymap.metrics.label(),
                keymap.activity.label(),
                keymap.relations.label()
            ),
            Color::White,
        ));
        hints.push(("D: definition".to_string(), Color::White));
        hints.push(("Y: sync".to_string(), Color::White));
    }
    if preview_open {
        hints.push(("o: open".to_string(), Color::White));
    }
    hints
}

fn key_hint(key: &str, action: &str) -> String {
    if key == ":" {
        format!(": {action}")
    } else {
        format!("{key}: {action}")
    }
}

/// Format the sync-status pill label. Success labels carry a leading
/// `✓ ` (`SUCCESS_MARKER`) and failure a leading `⚠ ` (`SYNC_FAIL_MARKER`)
/// so the outcome reads even without color. Strings are stable and pinned
/// by tests. The pill's color comes from [`sync_pill_color`].
pub(crate) fn sync_pill_label(status: Option<&SyncStatus>) -> Option<String> {
    let s = status?;
    let label = match s {
        SyncStatus::InFlight => "Syncing\u{2026}".to_string(),
        SyncStatus::Succeeded { new_commits: 0 } => {
            format!("{SUCCESS_MARKER} Synced (already up to date)")
        }
        SyncStatus::Succeeded { new_commits: 1 } => {
            format!("{SUCCESS_MARKER} Synced (1 new commit)")
        }
        SyncStatus::Succeeded { new_commits } => {
            format!("{SUCCESS_MARKER} Synced ({new_commits} new commits)")
        }
        SyncStatus::SucceededWithState { new_commits: 0 } => {
            format!("{SUCCESS_MARKER} Definition + state synced (already up to date)")
        }
        SyncStatus::SucceededWithState { new_commits: 1 } => {
            format!("{SUCCESS_MARKER} Definition + state synced (1 new commit)")
        }
        SyncStatus::SucceededWithState { new_commits } => {
            format!("{SUCCESS_MARKER} Definition + state synced ({new_commits} new commits)")
        }
        SyncStatus::Partial { message } => format!("{SYNC_FAIL_MARKER} {message}"),
        SyncStatus::Failed { message } => format!("{SYNC_FAIL_MARKER} Sync failed: {message}"),
        SyncStatus::Unavailable { hint } => format!("Sync unavailable: {hint}"),
    };
    Some(label)
}

/// Map a [`SyncStatus`] variant to the sync pill's foreground color so the
/// outcome is readable at a glance: in-flight cyan, success green, failure
/// red, unavailable yellow. Color is derived from the variant, never the
/// label string.
pub(crate) fn sync_pill_color(status: &SyncStatus) -> Color {
    match status {
        SyncStatus::InFlight => Color::Cyan,
        SyncStatus::Succeeded { .. } | SyncStatus::SucceededWithState { .. } => Color::Green,
        SyncStatus::Partial { .. } => Color::Yellow,
        SyncStatus::Failed { .. } => Color::Red,
        SyncStatus::Unavailable { .. } => Color::Yellow,
    }
}
