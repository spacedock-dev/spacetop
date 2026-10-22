use super::OverviewState;
use crossterm::event::KeyCode;
use spacetop_core::{domain::*, query::EntityQuery};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistorySelection {
    Record(ReviewRecordIdentity),
    Item { round: RoundIdentity, id: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipSelection {
    source: ReviewRecordIdentity,
    target: ReviewRecordIdentity,
    direction: RelationshipDirection,
}
impl RelationshipSelection {
    fn matches(&self, edge: &RecordedRelationship) -> bool {
        self.source == edge.source && self.target == edge.target && self.direction == edge.direction
    }
}
impl From<&RecordedRelationship> for RelationshipSelection {
    fn from(edge: &RecordedRelationship) -> Self {
        Self {
            source: edge.source.clone(),
            target: edge.target.clone(),
            direction: edge.direction,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoryBrowser {
    pub facts: CorrectionHistory,
    pub round: Option<RoundIdentity>,
    pub selection: Option<HistorySelection>,
    pub records_focus: bool,
    pub stack: Vec<ReviewRecordIdentity>,
    pub relationship: Option<RelationshipSelection>,
    pub preview: Option<String>,
    pub notice: Option<String>,
    initialized: bool,
}
impl HistoryBrowser {
    pub fn selected_round(&self) -> Option<&CorrectionRound> {
        self.facts.correction_round(self.round.as_ref()?)
    }
    pub fn refresh(&mut self, facts: CorrectionHistory) {
        let prior = self
            .stack
            .last()
            .and_then(|id| self.facts.correction_record(id))
            .cloned();
        if self.preview.take().is_some() {
            self.notice =
                Some("Preview invalidated by refresh; Enter revalidates selected evidence".into());
        }
        self.facts = facts;
        if !self.initialized {
            self.round = self
                .facts
                .rounds
                .iter()
                .find(|r| r.provenance == RoundProvenance::CurrentPointer)
                .or_else(|| self.facts.rounds.last())
                .map(|r| r.identity.clone());
            self.selection = self.selections().into_iter().next();
            self.initialized = true;
        } else {
            if self
                .round
                .as_ref()
                .is_some_and(|id| self.facts.correction_round(id).is_none())
            {
                self.round = None;
                self.notice = Some("Selected round disappeared; select a round explicitly".into());
            }
            if self
                .selection
                .as_ref()
                .is_some_and(|s| !self.selections().contains(s))
            {
                self.selection = None;
                self.notice = Some("Selected record/item disappeared; select explicitly".into());
            }
            if prior
                .as_ref()
                .is_some_and(|r| self.facts.correction_record(&r.identity) != Some(r))
            {
                self.stack.clear();
                self.notice = Some(
                    "Record detail invalidated: selected evidence changed or disappeared".into(),
                );
            } else {
                self.stack
                    .retain(|id| self.facts.correction_record(id).is_some());
            }
        }
        if self
            .relationship
            .as_ref()
            .is_some_and(|selected| !self.edges().iter().any(|edge| selected.matches(edge)))
        {
            self.relationship = None;
            if !self.stack.is_empty() {
                self.notice = Some(
                    "Selected relationship disappeared; select a relationship explicitly".into(),
                );
            }
        }
    }
    fn selections(&self) -> Vec<HistorySelection> {
        let Some(round) = self.selected_round() else {
            return Vec::new();
        };
        let mut items = round
            .records
            .iter()
            .map(|r| HistorySelection::Record(r.identity.clone()))
            .collect::<Vec<_>>();
        if let Some(briefing) = &round.briefing {
            items.extend(briefing.items.iter().map(|i| HistorySelection::Item {
                round: round.identity.clone(),
                id: i.id.clone(),
            }));
        }
        items
    }
    pub fn edges(&self) -> Vec<RecordedRelationship> {
        let Some(id) = self.stack.last() else {
            return Vec::new();
        };
        match self.facts.correction_relationships(id) {
            CorrectionRelationships::Recorded(edges) => edges,
            _ => Vec::new(),
        }
    }
    /// Returns true only when Back has exhausted this browser's navigation stack.
    pub fn key(&mut self, code: KeyCode, state: &OverviewState, path: &Path) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.preview.take().is_some() {
                    return false;
                }
                if let Some(id) = self.stack.pop() {
                    self.selection = Some(HistorySelection::Record(
                        self.stack.last().cloned().unwrap_or(id),
                    ));
                    self.relationship = self.edges().first().map(RelationshipSelection::from);
                } else {
                    return true;
                }
            }
            KeyCode::Tab if self.stack.is_empty() && self.preview.is_none() => {
                self.records_focus = !self.records_focus
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Enter => self.open(state, path),
            _ => {}
        }
        false
    }
    fn move_selection(&mut self, delta: isize) {
        if self.preview.is_some() {
            return;
        }
        if !self.stack.is_empty() {
            let edges = self.edges();
            let index = self
                .relationship
                .as_ref()
                .and_then(|selected| edges.iter().position(|edge| selected.matches(edge)));
            self.relationship = edges
                .get(
                    index
                        .map_or(0, |i| i.saturating_add_signed(delta))
                        .min(edges.len().saturating_sub(1)),
                )
                .map(RelationshipSelection::from);
        } else if self.records_focus {
            let selections = self.selections();
            let index = self
                .selection
                .as_ref()
                .and_then(|s| selections.iter().position(|i| i == s));
            self.selection = selections
                .get(
                    index
                        .map_or(0, |i| i.saturating_add_signed(delta))
                        .min(selections.len().saturating_sub(1)),
                )
                .cloned();
        } else {
            let rounds = &self.facts.rounds;
            let index = self
                .round
                .as_ref()
                .and_then(|s| rounds.iter().position(|r| &r.identity == s));
            self.round = rounds
                .get(
                    index
                        .map_or(0, |i| i.saturating_add_signed(delta))
                        .min(rounds.len().saturating_sub(1)),
                )
                .map(|r| r.identity.clone());
            self.selection = self.selections().into_iter().next();
        }
    }
    fn open(&mut self, state: &OverviewState, path: &Path) {
        let Some(entity) = state
            .index()
            .query(EntityQuery {
                scope: state.current_query_scope(),
                ..Default::default()
            })
            .into_iter()
            .find(|e| e.evidence_origin.as_deref().unwrap_or(&e.path) == path)
        else {
            self.notice = Some("Entity disappeared".into());
            return;
        };
        if !self.stack.is_empty() {
            let edges = self.edges();
            let Some(edge) = self
                .relationship
                .as_ref()
                .and_then(|selected| edges.iter().find(|edge| selected.matches(edge)))
            else {
                return;
            };
            if edge.validation != RelationshipValidation::Valid {
                self.notice = Some("Invalid relationship: target is not navigable".into());
                return;
            }
            let target = if edge.direction == RelationshipDirection::Incoming {
                edge.source.clone()
            } else {
                edge.target.clone()
            };
            if self.revalidate_record(state, &entity, &target) {
                self.stack.push(target);
                self.relationship = self.edges().first().map(RelationshipSelection::from);
            }
            return;
        }
        match self.selection.clone() {
            Some(HistorySelection::Record(id))
                if self.stack.is_empty() && self.revalidate_record(state, &entity, &id) =>
            {
                self.stack.push(id);
                self.relationship = self.edges().first().map(RelationshipSelection::from);
            }
            Some(HistorySelection::Item { round, id }) => {
                let Some(round) = self.facts.correction_round(&round) else {
                    return;
                };
                let Some(item) = round
                    .briefing
                    .as_ref()
                    .and_then(|b| b.items.iter().find(|i| i.id == id))
                else {
                    return;
                };
                match spacetop_core::correction_history::read_item(
                    state.index().definition(),
                    &entity,
                    round,
                    item,
                ) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(text) => self.preview = Some(text),
                        Err(_) => self.notice = Some("Verified bytes are not UTF-8 text".into()),
                    },
                    Err(error) => {
                        self.preview = None;
                        self.notice = Some(error.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    fn revalidate_record(
        &mut self,
        state: &OverviewState,
        entity: &Entity,
        id: &ReviewRecordIdentity,
    ) -> bool {
        let fresh = state.index().correction_rounds(entity);
        let old_round = self.facts.correction_round(&id.round);
        let new_round = fresh.correction_round(&id.round);
        let valid = old_round
            .zip(new_round)
            .is_some_and(|(a, b)| a.digest.is_some() && a.digest == b.digest)
            && self.facts.correction_record(id).is_some()
            && self.facts.correction_record(id) == fresh.correction_record(id)
            && self.stack.last().is_none_or(|source| {
                self.facts.correction_record(source) == fresh.correction_record(source)
            });
        self.refresh(fresh);
        if !valid {
            self.stack.clear();
            self.notice = Some("Selected evidence changed; detail was not opened".into());
        }
        valid
    }
    pub fn status(&self) -> &'static str {
        match self.selected_round() {
            Some(r) if r.provenance == RoundProvenance::CurrentPointer && r.briefing.is_some() => {
                "CURRENT bound"
            }
            Some(r) if r.provenance == RoundProvenance::CurrentPointer => "CURRENT UNVERIFIED",
            Some(r) if r.briefing.is_some() => "RETAINED identity verified",
            Some(_) => "RETAINED UNVERIFIED",
            None => "NO ROUND",
        }
    }
    pub fn round_lines(&self) -> String {
        self.facts
            .rounds
            .iter()
            .map(|r| {
                format!(
                    "{} {} / {} [{}] {:?}",
                    if self.round.as_ref() == Some(&r.identity) {
                        ">"
                    } else {
                        " "
                    },
                    r.identity.stage,
                    r.identity.cycle,
                    if r.provenance == RoundProvenance::CurrentPointer {
                        "current"
                    } else {
                        "retained"
                    },
                    r.decision()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn content(&self) -> String {
        if let Some(preview) = &self.preview {
            return preview.clone();
        }
        let mut text = String::new();
        if !self
            .facts
            .rounds
            .iter()
            .any(|r| r.provenance == RoundProvenance::CurrentPointer)
        {
            text.push_str("No current pointer; selection is not current.\n");
        }
        if let Some(notice) = &self.notice {
            text.push_str(&format!("{notice}\n"));
        }
        for diagnostic in &self.facts.diagnostics {
            diagnostic_text(&mut text, diagnostic);
        }
        let Some(round) = self.selected_round() else {
            text.push_str(
                "No correction rounds selected. Legacy entities may have no recorded history.\n",
            );
            return text;
        };
        text.push_str(&format!(
            "Stage: {} | Cycle: {} | {:?}\nReviewer verdict: {:?} | {:?}\n",
            round.identity.stage,
            round.identity.cycle,
            round.provenance,
            round.decision(),
            round.completeness
        ));
        for diagnostic in &round.diagnostics {
            diagnostic_text(&mut text, diagnostic);
        }
        text.push_str("Advisory records: answered is not approval or proof of fixes.\n");
        if let Some(id) = self.stack.last() {
            if let Some(record) = self.facts.correction_record(id) {
                record_text(&mut text, record);
                match self.facts.correction_relationships(id) {
                    CorrectionRelationships::NoRecordedRelationship => {
                        text.push_str("No recorded relationship.\n")
                    }
                    CorrectionRelationships::Unavailable => {
                        text.push_str("Relationships unavailable: incomplete/invalid evidence.\n")
                    }
                    CorrectionRelationships::Recorded(edges) => {
                        for edge in &edges {
                            text.push_str(&format!(
                                "{} includes {:?} {:?} {:?}: {} -> {}\n",
                                if self.relationship.as_ref().is_some_and(|s| s.matches(edge)) {
                                    ">"
                                } else {
                                    " "
                                },
                                edge.direction,
                                edge.role,
                                edge.validation,
                                edge.source.id,
                                edge.target.id
                            ));
                        }
                    }
                }
                if !self.edges().iter().any(|e| {
                    e.direction == RelationshipDirection::Incoming
                        && e.role == RelationshipRole::FollowUp
                        && e.validation == RelationshipValidation::Valid
                }) {
                    text.push_str("No recorded follow-up relationship in the validated prefix; no completion inferred.\n");
                }
            }
        } else {
            if let Some(b) = &round.briefing {
                text.push_str(&format!("Question: {}\n", b.question));
            }
            for record in &round.records {
                text.push_str(
                    if self.selection.as_ref()
                        == Some(&HistorySelection::Record(record.identity.clone()))
                    {
                        "> "
                    } else {
                        "  "
                    },
                );
                record_text(&mut text, record);
            }
            if let Some(b) = &round.briefing {
                for item in &b.items {
                    text.push_str(&format!(
                        "{} {:?}: {}\nURI: {}\nRevision: {} (verified on open)\n",
                        if self.selection.as_ref()
                            == Some(&HistorySelection::Item {
                                round: round.identity.clone(),
                                id: item.id.clone()
                            })
                        {
                            ">"
                        } else {
                            " "
                        },
                        item.kind,
                        item.id,
                        item.uri,
                        item.revision
                    ));
                }
            }
            for reference in &round.references {
                text.push_str(&format!(
                    "Reference {}: {} (label-only, not openable)\n",
                    reference.id, reference.label
                ));
            }
            for edge in &round.invalid_relationships {
                text.push_str(&format!(
                    "Invalid includes (not navigable): {} -> {}\n",
                    edge.source.id, edge.target.id
                ));
            }
        }
        text.push_str("Recorded attribution is not person authentication.\n");
        text
    }
}
fn diagnostic_text(text: &mut String, diagnostic: &ReviewDiagnostic) {
    text.push_str(&format!(
        "{}{}: {}\n",
        diagnostic.path.display(),
        diagnostic
            .line
            .map(|line| format!(":{line}"))
            .unwrap_or_default(),
        diagnostic.problem
    ));
}
fn record_text(text: &mut String, record: &ReviewRecord) {
    text.push_str(&format!(
        "#{} {}\nActor: {} | Time: {}\n",
        record.ordinal, record.identity.id, record.by, record.at
    ));
    match &record.entry {
        ReviewEntry::Annotation(a) => {
            text.push_str(&format!(
                "Annotation body: {}\n",
                a.body.as_deref().unwrap_or("No body recorded")
            ));
            if let Some(target) = &a.target {
                text.push_str(&format!("Target: {target}\n"));
            }
            if let Some(kind) = &a.kind {
                text.push_str(&format!("Kind: {kind}\n"));
            }
        }
        ReviewEntry::Resolution(r) => text.push_str(&format!(
            "Resolution: {:?}\nReason: {}\n",
            r.decision,
            super::gate_room::recorded_reason(&r.reason)
        )),
    }
    if !record.includes.is_empty() {
        text.push_str(&format!(
            "Recorded includes: {}\n",
            record.includes.join(", ")
        ));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app::{App, AppMode};
    use crossterm::event::{KeyEvent, KeyModifiers};
    use std::{fs, path::PathBuf};
    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &to.join(entry.file_name()));
            } else {
                fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
    }
    pub(crate) fn setup() -> (tempfile::TempDir, WorkflowSnapshot) {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/correction_history/workflow"),
            &root,
        );
        let definition =
            spacetop_core::parser::parse_workflow_readme(&root.join("README.md")).unwrap();
        let entity = spacetop_core::parser::parse_work_item(
            &root.join("sample/index.md"),
            &["implementation".into()],
            None,
        )
        .unwrap();
        (
            temp,
            WorkflowSnapshot {
                definition,
                items: vec![entity],
                parse_errors: Vec::new(),
            },
        )
    }
    fn press(app: &mut App, key: KeyCode) {
        app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
    }
    fn browser(app: &App) -> &HistoryBrowser {
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!("history browser expected")
        };
        &browser.history
    }
    #[test]
    fn gate_room_history_chain_back_order_and_current_are_identity_based() {
        let (_temp, snapshot) = setup();
        let mut app = App::from_snapshot(snapshot.definition.root.clone(), snapshot);
        press(&mut app, KeyCode::Char('B'));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(browser(&app).round.as_ref().unwrap().cycle, 1);
        press(&mut app, KeyCode::Down);
        assert_eq!(browser(&app).round.as_ref().unwrap().cycle, 2);
        press(&mut app, KeyCode::Down);
        assert_eq!(browser(&app).round.as_ref().unwrap().cycle, 10);
        assert!(browser(&app).content().contains("partial JSONL"));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Enter);
        let source = browser(&app).stack[0].clone();
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            browser(&app).stack.last().unwrap().id,
            "annotation:decline-duplicate-member"
        );
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            browser(&app).stack.last().unwrap().id,
            "resolution:ensign-3j-round-1"
        );
        press(&mut app, KeyCode::Esc);
        assert_eq!(
            browser(&app).stack.last().unwrap().id,
            "annotation:decline-duplicate-member"
        );
        press(&mut app, KeyCode::Esc);
        assert_eq!(browser(&app).stack.last(), Some(&source));
        press(&mut app, KeyCode::Esc);
        assert_eq!(
            browser(&app).selection,
            Some(HistorySelection::Record(source))
        );
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode(), AppMode::Overview(_)));
    }
    #[test]
    fn gate_room_history_removed_relationship_requires_explicit_reselection() {
        let (_temp, snapshot) = setup();
        let root = snapshot.definition.root.clone();
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        for key in [
            KeyCode::Char('B'),
            KeyCode::Char('h'),
            KeyCode::Tab,
            KeyCode::Enter,
            KeyCode::Down,
        ] {
            press(&mut app, key);
        }
        let selected = browser(&app).relationship.clone().unwrap();
        assert_eq!(selected.source.id, "annotation:decline-duplicate-member");
        let source = browser(&app).stack.clone();
        let log = root.join("sample/review/implementation/round-1/briefing.review.jsonl");
        let original = fs::read_to_string(&log).unwrap();
        fs::write(
            &log,
            original.lines().take(3).collect::<Vec<_>>().join("\n") + "\n",
        )
        .unwrap();
        app.reload_from_snapshot(snapshot.clone());
        assert!(browser(&app).relationship.is_none());
        assert!(browser(&app)
            .notice
            .as_ref()
            .unwrap()
            .contains("Selected relationship disappeared"));
        // Repeated refresh and Enter must not implicitly select the remaining reviewer edge.
        app.reload_from_snapshot(snapshot);
        press(&mut app, KeyCode::Enter);
        assert_eq!(browser(&app).stack, source);
        assert!(browser(&app).relationship.is_none());
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            browser(&app).stack.last().unwrap().id,
            "resolution:roborev-3j-round-1"
        );
    }

    #[test]
    fn gate_room_history_relationship_survives_log_reordering_by_identity() {
        let (_temp, snapshot) = setup();
        let root = snapshot.definition.root.clone();
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        for key in [
            KeyCode::Char('B'),
            KeyCode::Char('h'),
            KeyCode::Tab,
            KeyCode::Enter,
            KeyCode::Down,
        ] {
            press(&mut app, key);
        }
        let selected = browser(&app).relationship.clone().unwrap();
        let log = root.join("sample/review/implementation/round-1/briefing.review.jsonl");
        let original = fs::read_to_string(&log).unwrap();
        let mut lines = original.lines().collect::<Vec<_>>();
        lines.swap(2, 3);
        fs::write(&log, lines.join("\n") + "\n").unwrap();
        app.reload_from_snapshot(snapshot);
        assert_eq!(browser(&app).relationship.as_ref(), Some(&selected));
        assert!(selected.matches(&browser(&app).edges()[0]));
        assert!(browser(&app).notice.is_none());
        press(&mut app, KeyCode::Enter);
        assert_eq!(browser(&app).stack.last(), Some(&selected.source));
    }

    #[test]
    fn gate_room_history_refresh_invalidates_preview_and_preserves_or_removes_identity() {
        let (_temp, snapshot) = setup();
        let root = snapshot.definition.root.clone();
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        press(&mut app, KeyCode::Char('B'));
        press(&mut app, KeyCode::Char('h'));
        press(&mut app, KeyCode::Tab);
        for _ in 0..5 {
            press(&mut app, KeyCode::Down);
        }
        press(&mut app, KeyCode::Enter);
        assert!(browser(&app)
            .preview
            .as_ref()
            .unwrap()
            .contains("candidate 90aea55"));
        let selected = browser(&app).selection.clone();
        app.reload_from_snapshot(snapshot.clone());
        assert!(browser(&app).preview.is_none());
        assert_eq!(browser(&app).selection, selected);
        press(&mut app, KeyCode::Enter);
        assert!(browser(&app).preview.is_some());
        fs::remove_file(root.join("sample/candidate.patch")).unwrap();
        app.reload_from_snapshot(snapshot.clone());
        press(&mut app, KeyCode::Enter);
        assert!(browser(&app).preview.is_none());
        assert!(browser(&app).notice.as_ref().unwrap().contains("Missing"));
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Down);
        assert_eq!(browser(&app).round.as_ref().unwrap().cycle, 2);
        fs::remove_dir_all(root.join("sample/review/implementation/round-2")).unwrap();
        app.reload_from_snapshot(snapshot);
        assert!(browser(&app).round.is_none());
        assert!(browser(&app).selection.is_none());
        assert!(browser(&app)
            .notice
            .as_ref()
            .unwrap()
            .contains("disappeared"));
    }
    #[test]
    fn gate_room_history_revalidates_detail_before_open_and_never_infers_current() {
        let (_temp, mut snapshot) = setup();
        let root = snapshot.definition.root.clone();
        snapshot.items[0].review_round = ReviewPointer::Absent;
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        press(&mut app, KeyCode::Char('B'));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(browser(&app).round.as_ref().unwrap().stage, "validation");
        assert!(browser(&app).content().contains("No current pointer"));
        press(&mut app, KeyCode::Tab);
        let log = root.join("sample/review/validation/round-1/briefing.review.jsonl");
        fs::write(log, "").unwrap();
        press(&mut app, KeyCode::Enter);
        assert!(browser(&app).stack.is_empty());
        assert!(browser(&app)
            .notice
            .as_ref()
            .unwrap()
            .contains("not opened"));
        // A missing pointer target remains a row; it is not replaced by latest retained evidence.
        snapshot.items[0].review_round = ReviewPointer::Valid {
            pointer: RoundPointer {
                id: "round:3j:implementation:9".into(),
                stage: "implementation".into(),
                cycle: 9,
                briefing: RoundBinding {
                    id: "briefing:3j:implementation:round-9".into(),
                    digest: format!("sha256:{}", "a".repeat(64)),
                    room_ref: "./review/implementation/round-9".into(),
                },
            },
        };
        let mut app = App::from_snapshot(root, snapshot);
        press(&mut app, KeyCode::Char('B'));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(browser(&app).round.as_ref().unwrap().cycle, 9);
        assert_eq!(browser(&app).status(), "CURRENT UNVERIFIED");
    }
}
