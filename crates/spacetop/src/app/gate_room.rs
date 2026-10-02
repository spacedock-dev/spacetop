use super::OverviewState;
use crossterm::event::KeyCode;
use spacetop_core::domain::{GateAttempt, GateRecord, GateRoomView, RoomDiagnostic, RoomProblem};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomFocus {
    Attempts,
    Items,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GateBrowser {
    pub entity_path: PathBuf,
    pub workflow_dir: PathBuf,
    pub dependencies: Vec<PathBuf>,
    pub attempts: Vec<(GateRecord, GateAttempt)>,
    pub selected: Option<(String, String)>,
    pub focus: RoomFocus,
    pub item: usize,
    pub view: GateRoomView,
    pub preview: Option<String>,
    pub scroll: usize,
    pub wrap: bool,
    pub notice: Option<String>,
}
impl GateBrowser {
    pub fn new(state: &OverviewState) -> Option<Self> {
        let entity = state.selected_item()?;
        let selected = spacetop_core::gates::selected_attempt(&entity.gates, &entity.status)
            .map(|(r, a)| (r.id.clone(), a.id.clone()));
        let mut browser = Self {
            entity_path: entity.path,
            workflow_dir: state.workflow_dir.clone(),
            dependencies: Vec::new(),
            attempts: Vec::new(),
            selected,
            focus: RoomFocus::Attempts,
            item: 0,
            view: GateRoomView::failed(RoomDiagnostic::new(
                RoomProblem::Missing,
                "no recorded attempts",
            )),
            preview: None,
            scroll: 0,
            wrap: true,
            notice: None,
        };
        browser.refresh(state);
        Some(browser)
    }
    pub fn attempt(&self) -> Option<&(GateRecord, GateAttempt)> {
        self.attempts.iter().find(|(r, a)| {
            self.selected
                .as_ref()
                .is_some_and(|(g, t)| g == &r.id && t == &a.id)
        })
    }
    pub fn refresh(&mut self, state: &OverviewState) {
        let prior_item = self
            .view
            .briefing
            .as_ref()
            .and_then(|b| b.items.get(self.item))
            .map(|i| i.id.clone());
        let entity = state
            .index()
            .query(spacetop_core::query::EntityQuery {
                scope: state.current_query_scope(),
                ..Default::default()
            })
            .into_iter()
            .find(|e| e.path == self.entity_path);
        let prior_selection = self.selected.clone();
        let had_preview = self.preview.take().is_some();
        let Some(entity) = entity else {
            self.dependencies.clear();
            self.attempts.clear();
            self.selected = None;
            self.view = GateRoomView::failed(RoomDiagnostic::new(
                RoomProblem::Missing,
                "selected entity disappeared",
            ));
            return;
        };
        self.attempts = state
            .index()
            .gate_attempts(&entity)
            .into_iter()
            .map(|(r, a)| (r.clone(), a.clone()))
            .collect();
        if !self.attempts.iter().any(|(r, a)| {
            self.selected
                .as_ref()
                .is_some_and(|(g, t)| g == &r.id && t == &a.id)
        }) {
            if self.selected.is_some() {
                self.notice = Some("Selected attempt disappeared; showing current attempt".into());
            }
            self.selected = spacetop_core::gates::selected_attempt(&entity.gates, &entity.status)
                .or_else(|| self.attempts.last().map(|(r, a)| (r, a)))
                .map(|(r, a)| (r.id.clone(), a.id.clone()));
        }
        self.view = match &self.selected {
            Some((g, a)) => state.index().gate_room(&entity, g, a),
            None => GateRoomView::failed(RoomDiagnostic::new(
                RoomProblem::Missing,
                "no recorded attempts",
            )),
        };
        self.dependencies = self
            .attempt()
            .map(|(_, a)| {
                spacetop_core::gate_room::local_dependencies(
                    state.index().definition(),
                    &entity,
                    a,
                    &self.view,
                )
            })
            .unwrap_or_default();
        self.item = self
            .view
            .briefing
            .as_ref()
            .map(|b| {
                prior_item
                    .and_then(|id| b.items.iter().position(|i| i.id == id))
                    .unwrap_or(self.item)
                    .min(b.items.len().saturating_sub(1))
            })
            .unwrap_or(0);
        if had_preview && prior_selection == self.selected {
            if let (Some((g, a)), Some(item)) = (
                &self.selected,
                self.view
                    .briefing
                    .as_ref()
                    .and_then(|b| b.items.get(self.item)),
            ) {
                match state.index().gate_room_item(&entity, g, a, &item.id) {
                    Ok(bytes) => self.preview = String::from_utf8(bytes).ok(),
                    Err(e) => self.notice = Some(format!("Preview invalidated: {e}")),
                }
            }
        }
        self.scroll = self
            .scroll
            .min(self.content().chars().count().saturating_sub(1));
    }
    /// Returns true only when the browser itself should close.
    pub fn key(&mut self, code: KeyCode, state: &OverviewState) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.preview.take().is_some() {
                    self.scroll = 0;
                } else {
                    return true;
                }
            }
            KeyCode::Tab => {
                self.focus = if self.focus == RoomFocus::Attempts {
                    RoomFocus::Items
                } else {
                    RoomFocus::Attempts
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1, state),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1, state),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(5),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(5),
            KeyCode::Char('w') => self.wrap = !self.wrap,
            KeyCode::Enter if self.focus == RoomFocus::Items => self.open_item(state),
            _ => {}
        }
        false
    }
    fn move_selection(&mut self, delta: isize, state: &OverviewState) {
        self.preview = None;
        self.scroll = 0;
        if self.focus == RoomFocus::Items {
            let len = self
                .view
                .briefing
                .as_ref()
                .map(|b| b.items.len())
                .unwrap_or(0);
            self.item = self
                .item
                .saturating_add_signed(delta)
                .min(len.saturating_sub(1));
        } else if !self.attempts.is_empty() {
            let position = self
                .attempts
                .iter()
                .position(|(r, a)| {
                    self.selected
                        .as_ref()
                        .is_some_and(|(g, t)| g == &r.id && t == &a.id)
                })
                .unwrap_or(0);
            let (r, a) = &self.attempts[position
                .saturating_add_signed(delta)
                .min(self.attempts.len() - 1)];
            self.selected = Some((r.id.clone(), a.id.clone()));
            self.item = 0;
            self.refresh(state);
        }
    }
    fn open_item(&mut self, state: &OverviewState) {
        let Some(entity) = state
            .index()
            .query(spacetop_core::query::EntityQuery {
                scope: state.current_query_scope(),
                ..Default::default()
            })
            .into_iter()
            .find(|e| e.path == self.entity_path)
        else {
            self.refresh(state);
            return;
        };
        let Some((gate, attempt)) = &self.selected else {
            return;
        };
        let Some(item) = self
            .view
            .briefing
            .as_ref()
            .and_then(|b| b.items.get(self.item))
        else {
            return;
        };
        let result = state
            .index()
            .gate_room_item(&entity, gate, attempt, &item.id);
        match result {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => self.preview = Some(text),
                Err(_) => {
                    self.notice =
                        Some("Verified bytes are not UTF-8 text; preview unavailable".into())
                }
            },
            Err(e) => {
                self.refresh(state);
                self.notice = Some(e.to_string());
            }
        }
        self.scroll = 0;
    }
    pub fn content(&self) -> String {
        if let Some(preview) = &self.preview {
            return preview.clone();
        }
        let mut text = String::new();
        for d in &self.view.diagnostics {
            text.push_str(&format!("{d}\n"));
        }
        if let Some(notice) = &self.notice {
            text.push_str(&format!("{notice}\n"));
        }
        if let Some(briefing) = &self.view.briefing {
            text.push_str(&format!("Question: {}\n", briefing.question));
        }
        if let Some((record, attempt)) = self.attempt() {
            text.push_str(&format!(
                "Stage: {}\nGate: {}\nAttempt: {}\n",
                record.stage, record.id, attempt.id
            ));
            if let Some(resolution) = &attempt.resolution {
                text.push_str(&format!(
                    "Recorded {:?} by {} at {}\nReason: {}\n",
                    resolution.decision, resolution.by, resolution.at, resolution.reason
                ));
                if let Some(conn) = &resolution.conn {
                    text.push_str(&format!(
                        "Conn quote: {}\nConn source: {}\n",
                        conn.quote, conn.source
                    ));
                }
                if !resolution.includes.is_empty() {
                    text.push_str(&format!(
                        "Included annotations (recorded refs): {}\n",
                        resolution.includes.join(", ")
                    ));
                }
            } else {
                text.push_str("Recorded decision: none\n");
            }
            if let Some(w) = &attempt.withdrawal {
                text.push_str(&format!(
                    "Withdrawal by {} at {}: {}\n",
                    w.by, w.at, w.reason
                ));
            }
            if let Some(a) = &attempt.application {
                text.push_str(&format!(
                    "Application: {:?} -> {}\n",
                    a.state, a.target_stage
                ));
            }
        }
        if let Some(briefing) = &self.view.briefing {
            for (position, item) in briefing.items.iter().enumerate() {
                text.push_str(&format!(
                    "{} {:?}: {}\nURI: {}\nRevision: {}\n",
                    if position == self.item { ">" } else { " " },
                    item.kind,
                    item.id,
                    item.uri,
                    item.revision
                ));
                if let Some(summary) = &item.summary {
                    text.push_str(&format!("Summary: {summary}\n"));
                }
            }
        }
        text.push_str("Recorded attribution is not person authentication.\n");
        text
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::{App, AppMode};
    use crossterm::event::{KeyEvent, KeyModifiers};
    use spacetop_core::{domain::*, parser};
    use std::{fs, path::PathBuf};
    pub(crate) fn setup() -> (tempfile::TempDir, WorkflowSnapshot) {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        fs::create_dir(root.join("room")).unwrap();
        for name in [
            "index.json",
            "gate-review.md",
            "entity-snapshot.md",
            "contract-snapshot.md",
        ] {
            fs::copy(
                fixture.join("gate_rooms").join(name),
                root.join("room").join(name),
            )
            .unwrap();
        }
        let mut definition =
            parser::parse_workflow_readme(&fixture.join("durable-gates/README.md")).unwrap();
        definition.root = root.clone();
        definition.storage = WorkflowStorage::SingleRoot;
        fs::write(
            root.join("e.md"),
            "---\nid: 3k\ntitle: Test\nstatus: validation\n---\nBody",
        )
        .unwrap();
        let mut entity =
            parser::parse_work_item(&root.join("e.md"), &["validation".into()], None).unwrap();
        let briefing = GateBriefing {
            id: "briefing:docs-dev:3k:validation:attempt-1:revision-1".into(),
            digest: spacetop_core::parser::gate_room::canonical_digest(
                &fs::read(root.join("room/index.json")).unwrap(),
            )
            .unwrap(),
            request_digest: None,
            room_ref: "room".into(),
        };
        let attempt = GateAttempt {
            id: "attempt-1".into(),
            briefing: briefing.clone(),
            resolution: Some(GateResolution {
                record_type: "Resolution".into(),
                id: "resolution-1".into(),
                briefing: briefing.id,
                by: "agent:first-officer".into(),
                at: "2026-10-01T00:00:00Z".into(),
                decision: GateDecision::Approve,
                reason: "Exact candidate accepted".into(),
                conn: Some(GateConn {
                    quote: "Please proceed".into(),
                    source: "conversation".into(),
                }),
                includes: vec!["annotation:recorded".into()],
            }),
            withdrawal: None,
            application: Some(GateApplication {
                target_stage: "done".into(),
                state: GateApplicationState::Pending,
            }),
        };
        let mut older = attempt.clone();
        older.id = "attempt-older".into();
        older.withdrawal = Some(GateWithdrawal {
            by: "person:captain".into(),
            at: "2026-09-30T00:00:00Z".into(),
            reason: "Replace candidate".into(),
        });
        older.resolution = None;
        older.application = None;
        entity.gates = GateData::Valid {
            document: GateDocument {
                version: 1,
                records: vec![GateRecord {
                    id: "gate:validation".into(),
                    stage: "validation".into(),
                    attempts: vec![older, attempt],
                }],
            },
            warnings: Vec::new(),
        };
        (
            temp,
            WorkflowSnapshot {
                definition,
                items: vec![entity],
                parse_errors: Vec::new(),
            },
        )
    }
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    #[test]
    fn b_navigation_preview_back_and_mutation_keys_are_inert() {
        let (_temp, snapshot) = setup();
        let mut app = App::from_snapshot(snapshot.definition.root.clone(), snapshot);
        press(&mut app, KeyCode::Char('B'));
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!("expected browser")
        };
        assert!(browser.view.verified());
        assert_eq!(browser.selected.as_ref().unwrap().1, "attempt-1");
        press(&mut app, KeyCode::Up);
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert_eq!(browser.selected.as_ref().unwrap().1, "attempt-older");
        assert!(browser.content().contains("Withdrawal by person:captain"));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Enter);
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert_eq!(
            browser.preview.as_deref(),
            Some("Exact candidate review.\n")
        );
        for code in [
            KeyCode::Char('Y'),
            KeyCode::Char('a'),
            KeyCode::Char('o'),
            KeyCode::Char('P'),
            KeyCode::Right,
        ] {
            press(&mut app, code);
        }
        assert!(!app.take_pending_sync());
        assert!(app.take_pending_open_file().is_none());
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode(), AppMode::GateRoom { .. }));
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode(), AppMode::Overview(_)));
    }
    #[test]
    fn reload_preserves_identity_focus_and_invalidates_preview() {
        let (_temp, mut snapshot) = setup();
        let root = snapshot.definition.root.clone();
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        press(&mut app, KeyCode::Char('B'));
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Enter);
        app.reload_from_snapshot(snapshot.clone());
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert_eq!(browser.focus, RoomFocus::Items);
        assert!(browser.preview.is_some());
        fs::write(root.join("room/gate-review.md"), "tampered").unwrap();
        app.reload_from_snapshot(snapshot.clone());
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert!(browser.preview.is_none());
        assert!(browser
            .notice
            .as_ref()
            .unwrap()
            .contains("ArtifactDigestMismatch"));
        let GateData::Valid { document, .. } = &mut snapshot.items[0].gates else {
            panic!()
        };
        document.records[0].attempts.pop();
        app.reload_from_snapshot(snapshot.clone());
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert_eq!(browser.selected.as_ref().unwrap().1, "attempt-older");
        assert!(browser.notice.as_ref().unwrap().contains("disappeared"));
        snapshot.items.clear();
        app.reload_from_snapshot(snapshot);
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert!(!browser.view.verified());
        assert!(browser.selected.is_none());
    }
    #[test]
    fn recorded_fields_remain_visible_without_room_and_cover_decisions_application_states() {
        let (_temp, mut snapshot) = setup();
        fs::remove_file(snapshot.definition.root.join("room/index.json")).unwrap();
        for decision in [
            GateDecision::Approve,
            GateDecision::Revise,
            GateDecision::Hold,
        ] {
            for application in [
                GateApplicationState::Pending,
                GateApplicationState::Consumed,
                GateApplicationState::Superseded,
            ] {
                let GateData::Valid { document, .. } = &mut snapshot.items[0].gates else {
                    panic!()
                };
                let a = &mut document.records[0].attempts[1];
                a.resolution.as_mut().unwrap().decision = decision;
                a.application.as_mut().unwrap().state = application;
                let state = OverviewState::from_snapshot(
                    snapshot.definition.root.clone(),
                    snapshot.clone(),
                );
                let browser = GateBrowser::new(&state).unwrap();
                let text = browser.content();
                assert!(!browser.view.verified());
                for part in [
                    "agent:first-officer",
                    "2026-10-01",
                    "Exact candidate accepted",
                    "Please proceed",
                    "conversation",
                    "annotation:recorded",
                    "not person authentication",
                ] {
                    assert!(text.contains(part), "{text}");
                }
                assert!(text.contains(&format!("Recorded {decision:?}")));
                assert!(text.contains(&format!("Application: {application:?}")));
            }
        }
    }
    #[test]
    fn b_is_reserved_and_empty_selection_does_not_open() {
        let (_temp, mut snapshot) = setup();
        snapshot.items.clear();
        let mut app = App::from_snapshot(snapshot.definition.root.clone(), snapshot);
        press(&mut app, KeyCode::Char('B'));
        assert!(matches!(app.mode(), AppMode::Overview(_)));
        let mut config = spacetop_core::config::SpacetopConfig::default();
        config.keybindings.search = "B".into();
        let keys = crate::app::keys::ResolvedKeymap::from_config(&config);
        assert!(keys.warnings().iter().any(|w| w.contains("reserved")));
    }
}
