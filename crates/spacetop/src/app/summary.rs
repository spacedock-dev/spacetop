use super::{App, AppMode, OverviewState};
use crossterm::event::{KeyCode, KeyEvent};
use spacetop_core::summary::{
    snapshot::{self, SummaryKey, SummarySnapshot},
    SummaryError, SummaryProvider,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    workflow: PathBuf,
    task: String,
    source: PathBuf,
}
impl Selection {
    fn from_state(state: &OverviewState) -> Option<Self> {
        let entity = state.selected_item()?;
        Some(Self {
            workflow: state.workflow_dir.clone(),
            task: entity.id,
            source: entity.worktree_source.unwrap_or(entity.path),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRequest {
    pub nonce: u64,
    pub snapshot: SummarySnapshot,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryCompletion {
    pub nonce: u64,
    pub key: SummaryKey,
    pub result: Result<String, SummaryError>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cached {
    selection: Selection,
    key: SummaryKey,
    text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct View {
    selection: Selection,
    snapshot: Result<SummarySnapshot, SummaryError>,
    notice: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Active {
    request: SummaryRequest,
    selection: Selection,
    started: Instant,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SummaryState {
    view: Option<View>,
    active: Option<Active>,
    pending: Option<SummaryRequest>,
    cache: VecDeque<Cached>,
    next_nonce: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    pub text: String,
    pub running: bool,
    pub stale: bool,
}

impl SummaryState {
    pub fn active_nonce(&self) -> Option<u64> {
        self.active.as_ref().map(|active| active.request.nonce)
    }
    pub fn take_request(&mut self) -> Option<SummaryRequest> {
        self.pending.take()
    }
    fn cancel(&mut self) {
        self.pending = None;
        if let Some(active) = self.active.take() {
            if let Some(view) = self
                .view
                .as_mut()
                .filter(|view| view.selection == active.selection)
            {
                view.notice = Some(SummaryError::Canceled.to_string());
            }
        }
    }
    fn start(&mut self, regenerate: bool) {
        let Some(view) = &mut self.view else {
            return;
        };
        if self.active.is_some() {
            return;
        }
        let snapshot = match &view.snapshot {
            Ok(snapshot) => snapshot.clone(),
            Err(error) => {
                view.notice = Some(error.to_string());
                return;
            }
        };
        if !regenerate && self.cache.iter().any(|entry| entry.key == snapshot.key) {
            return;
        }
        self.next_nonce += 1;
        let request = SummaryRequest {
            nonce: self.next_nonce,
            snapshot,
        };
        self.pending = Some(request.clone());
        self.active = Some(Active {
            request,
            selection: view.selection.clone(),
            started: Instant::now(),
        });
        view.notice = None;
    }
    pub fn complete(&mut self, completion: SummaryCompletion) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        if active.request.nonce != completion.nonce || active.request.snapshot.key != completion.key
        {
            return;
        }
        let selection = active.selection.clone();
        self.active = None;
        self.pending = None;
        match completion.result {
            Ok(text) => {
                self.cache.retain(|entry| entry.key != completion.key);
                self.cache.push_back(Cached {
                    selection,
                    key: completion.key,
                    text,
                });
                while self.cache.len() > 16
                    || self
                        .cache
                        .iter()
                        .map(|entry| entry.text.len())
                        .sum::<usize>()
                        > 1024 * 1024
                {
                    self.cache.pop_front();
                }
            }
            Err(error) => {
                if let Some(view) = self
                    .view
                    .as_mut()
                    .filter(|view| view.selection == selection)
                {
                    view.notice = Some(error.to_string());
                }
            }
        }
    }
    pub fn presentation(
        &self,
        state: &OverviewState,
        provider: SummaryProvider,
        now: Instant,
    ) -> Option<Presentation> {
        let selection = Selection::from_state(state)?;
        let view = self
            .view
            .as_ref()
            .filter(|view| view.selection == selection)?;
        let status = state
            .selected_item()
            .map(|item| item.status)
            .unwrap_or_default();
        let mut text = format!("AI Summary · {} / {}\nRecorded status: {}\nS/O: Original · Enter: generate/reuse · R: regenerate · Esc: cancel/back\n\n", provider.executable(), provider.model(), status);
        let active = self.active.as_ref().filter(|active| {
            active.selection == selection
                && view
                    .snapshot
                    .as_ref()
                    .is_ok_and(|snapshot| snapshot.key == active.request.snapshot.key)
        });
        if let Some(active) = active {
            let frame =
                (now.saturating_duration_since(active.started).as_millis() / 120) as usize % 4;
            text.push_str(&format!(
                "{} Generating summary… Esc cancels.\n\n",
                ["|", "/", "-", "\\"][frame]
            ));
        }
        if self.active.is_some() && active.is_none() {
            text.push_str("Another captured snapshot is still running. Esc cancels it before a new request.\n\n");
        }
        if let Some(notice) = &view.notice {
            text.push_str(notice);
            text.push_str("\n\n");
        }
        let key = view.snapshot.as_ref().ok().map(|snapshot| &snapshot.key);
        let cached = self
            .cache
            .iter()
            .rev()
            .find(|entry| Some(&entry.key) == key)
            .or_else(|| {
                self.cache
                    .iter()
                    .rev()
                    .find(|entry| entry.selection == selection)
            });
        let stale = cached.is_some_and(|entry| Some(&entry.key) != key);
        if let Some(cached) = cached {
            if stale {
                text.push_str("STALE — source changed. R explicitly regenerates.\n\n");
            }
            text.push_str("Generated prose is advisory. Recorded facts above and Original remain authoritative.\n\n");
            text.push_str(&cached.text);
        } else {
            text.push_str("Generate only if you agree: the selected installed CLI receives this captured task and may send it to its configured provider, with usage charges. Spacetop trusts that CLI's login, config, helpers and network behavior. Native controls are not a full sandbox. Cancel/timeout terminates and reaps the tracked process/group; escaped or untracked descendants may survive. Spacetop does not read credentials, follow evidence links or write workflow/project files.\n\nEnter sends this task using the provider/model above. No fallback. Nothing is sent by browsing.\n");
        }
        if let Err(error) = &view.snapshot {
            text.push_str(&format!("\n{error}"));
        }
        Some(Presentation {
            text,
            running: active.is_some(),
            stale,
        })
    }
}

impl App {
    pub fn refresh_summary_snapshot(&mut self) {
        if self.summary.view.is_none()
            || !self
                .as_session()
                .is_some_and(|session| session.active_slot_loaded())
        {
            return;
        }
        let Some(state) = self.as_overview() else {
            return;
        };
        let Some(selection) = Selection::from_state(state) else {
            return;
        };
        if !self
            .summary
            .view
            .as_ref()
            .is_some_and(|view| view.selection == selection)
        {
            return;
        }
        let snapshot = state
            .selected_item()
            .ok_or(SummaryError::Changed)
            .and_then(|entity| {
                snapshot::capture(&state.workflow_dir, &entity, self.config.summary.provider)
            });
        if let Some(view) = &mut self.summary.view {
            view.snapshot = snapshot;
        }
    }
    pub(super) fn handle_summary_key(&mut self, key: KeyEvent) -> bool {
        if !matches!(self.mode, AppMode::Overview(_))
            || !self
                .as_session()
                .is_some_and(|session| session.active_slot_loaded())
        {
            return false;
        }
        if key.code == KeyCode::Esc && self.summary.active.is_some() {
            self.summary.cancel();
            return true;
        }
        let Some(state) = self.as_overview() else {
            return false;
        };
        if !state.preview_open() {
            return false;
        }
        let Some(selection) = Selection::from_state(state) else {
            return false;
        };
        let showing = self
            .summary
            .view
            .as_ref()
            .is_some_and(|view| view.selection == selection);
        match key.code {
            KeyCode::Esc | KeyCode::Char('O') if showing => {
                self.summary.view = None;
                true
            }
            KeyCode::Char('S') => {
                if showing {
                    self.summary.view = None;
                } else {
                    let snapshot =
                        state
                            .selected_item()
                            .ok_or(SummaryError::Changed)
                            .and_then(|entity| {
                                snapshot::capture(
                                    &state.workflow_dir,
                                    &entity,
                                    self.config.summary.provider,
                                )
                            });
                    self.summary.view = Some(View {
                        selection,
                        snapshot,
                        notice: None,
                    });
                }
                if let Some(session) = self.mode.as_session_mut() {
                    session.active_state_mut().scroll_preview_to_top();
                }
                true
            }
            KeyCode::Enter | KeyCode::Char('R') if showing => {
                self.refresh_summary_snapshot();
                self.summary.start(key.code == KeyCode::Char('R'));
                true
            }
            _ => false,
        }
    }
    pub fn complete_summary(&mut self, completion: SummaryCompletion) {
        self.refresh_summary_snapshot();
        self.summary.complete(completion);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    pub(crate) fn fixture() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "---\ncommissioned-by: spacedock@test\nstages:\n  states:\n    - name: implement\n---\n").unwrap();
        for id in ["1", "2"] {
            std::fs::write(dir.path().join(format!("{id}.md")), format!("---\nid: '{id}'\ntitle: Task {id}\nstatus: implement\n---\n\nReported work. Rejected review; no human check.\n")).unwrap();
        }
        let mut app = App::load(dir.path().to_owned()).unwrap();
        key(&mut app, KeyCode::Enter);
        (app, dir)
    }
    pub(crate) fn key(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    fn presentation(app: &App, now: Instant) -> Option<Presentation> {
        app.summary
            .presentation(app.as_overview().unwrap(), app.config.summary.provider, now)
    }
    fn finish(request: &SummaryRequest, text: &str) -> SummaryCompletion {
        SummaryCompletion {
            nonce: request.nonce,
            key: request.snapshot.key.clone(),
            result: Ok(text.into()),
        }
    }

    #[test]
    fn browsing_and_disclosure_do_not_generate() {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Up);
        assert!(app.summary.take_request().is_none());
        key(&mut app, KeyCode::Char('S'));
        let view = presentation(&app, Instant::now()).unwrap();
        for disclosure in [
            "claude-haiku-5-5",
            "usage charges",
            "escaped",
            "does not read credentials",
        ] {
            assert!(view.text.contains(disclosure));
        }
        assert!(!view.running);
        assert!(app.summary.take_request().is_none());
        key(&mut app, KeyCode::Enter);
        assert!(app.summary.take_request().is_some());
    }
    #[test]
    fn progress_and_results_never_attach_to_other_selection() {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        let request = app.summary.take_request().unwrap();
        let now = app.summary.active.as_ref().unwrap().started;
        assert_ne!(
            presentation(&app, now).unwrap().text,
            presentation(&app, now + std::time::Duration::from_millis(120))
                .unwrap()
                .text
        );
        key(&mut app, KeyCode::Down);
        assert!(presentation(&app, now).is_none());
        assert!(app.summary.take_request().is_none());
        app.complete_summary(finish(&request, "Task one only"));
        assert!(presentation(&app, now).is_none());
        key(&mut app, KeyCode::Up);
        let view = presentation(&app, now).unwrap();
        assert!(!view.running);
        assert!(view.text.contains("Task one only"));
        key(&mut app, KeyCode::Char('O'));
        assert!(presentation(&app, now).is_none());
        assert!(app.as_overview().unwrap().preview_open());
    }
    #[test]
    fn cancel_retry_nonce_and_cache_are_explicit() {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        let first = app.summary.take_request().unwrap();
        key(&mut app, KeyCode::Esc);
        assert!(!presentation(&app, Instant::now()).unwrap().running);
        key(&mut app, KeyCode::Enter);
        let second = app.summary.take_request().unwrap();
        assert_ne!(first.nonce, second.nonce);
        app.complete_summary(finish(&first, "late old result"));
        assert_eq!(app.summary.active_nonce(), Some(second.nonce));
        app.complete_summary(finish(&second, "current result"));
        assert!(!presentation(&app, Instant::now())
            .unwrap()
            .text
            .contains("late old result"));
        key(&mut app, KeyCode::Enter);
        assert!(app.summary.take_request().is_none());
        key(&mut app, KeyCode::Char('R'));
        assert!(app.summary.take_request().is_some());
    }
    #[test]
    fn changed_source_stays_stale_until_explicit_regeneration() {
        let (mut app, dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        let request = app.summary.take_request().unwrap();
        let path = dir.path().join("1.md");
        let old = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, old.replace("Reported work", "Changed work")).unwrap();
        app.reload().unwrap();
        assert!(!presentation(&app, Instant::now()).unwrap().running);
        app.complete_summary(finish(&request, "Prior snapshot"));
        let view = presentation(&app, Instant::now()).unwrap();
        assert!(view.stale);
        assert!(view.text.contains("STALE"));
        assert!(app.summary.take_request().is_none());
        key(&mut app, KeyCode::Char('R'));
        assert_ne!(
            app.summary.take_request().unwrap().snapshot.key,
            request.snapshot.key
        );
    }
    #[test]
    fn failures_stop_spinner_and_leave_original_usable() {
        for error in [
            SummaryError::Unavailable,
            SummaryError::Provider,
            SummaryError::Timeout,
            SummaryError::Output,
            SummaryError::OutputLimit,
        ] {
            let (mut app, _dir) = fixture();
            key(&mut app, KeyCode::Char('S'));
            key(&mut app, KeyCode::Enter);
            let request = app.summary.take_request().unwrap();
            app.complete_summary(SummaryCompletion {
                nonce: request.nonce,
                key: request.snapshot.key,
                result: Err(error.clone()),
            });
            let view = presentation(&app, Instant::now()).unwrap();
            assert!(!view.running);
            assert!(view.text.contains(&error.to_string()));
            key(&mut app, KeyCode::Char('O'));
            assert!(app
                .as_overview()
                .unwrap()
                .selected_item()
                .unwrap()
                .body
                .contains("Rejected review"));
        }
    }
    #[test]
    fn cache_is_bounded_and_workflow_identity_is_separate() {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        for index in 0..20 {
            app.summary.start(true);
            let mut request = app.summary.take_request().unwrap();
            request.snapshot.key.input_digest = index.to_string();
            app.summary.active.as_mut().unwrap().request = request.clone();
            app.summary.complete(finish(&request, &"x".repeat(65536)));
        }
        assert_eq!(app.summary.cache.len(), 16);
        assert!(
            app.summary
                .cache
                .iter()
                .map(|entry| entry.text.len())
                .sum::<usize>()
                <= 1024 * 1024
        );
        let (other, _otherdir) = fixture();
        assert!(app
            .summary
            .presentation(
                other.as_overview().unwrap(),
                SummaryProvider::Claude,
                Instant::now()
            )
            .is_none());
    }
    #[test]
    fn summary_survives_an_unloaded_workflow_switch_without_foreign_progress() {
        use spacetop_core::discovery::DiscoveredWorkflow;
        let (mut app, dir) = fixture();
        let (_, other_dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        let request = app.summary.take_request().unwrap();
        key(&mut app, KeyCode::Char('q'));
        let state = app.as_overview().unwrap().clone();
        app.mode = AppMode::Overview(super::super::OverviewSession::from_discovery(
            dir.path().to_owned(),
            vec![
                DiscoveredWorkflow {
                    root: dir.path().to_owned(),
                    title: None,
                },
                DiscoveredWorkflow {
                    root: other_dir.path().to_owned(),
                    title: None,
                },
            ],
            0,
            state,
        ));
        key(&mut app, KeyCode::Right);
        assert!(!app.as_session().unwrap().active_slot_loaded());
        app.materialize_active();
        assert!(presentation(&app, Instant::now()).is_none());
        app.complete_summary(finish(&request, "Original workflow result"));
        assert!(presentation(&app, Instant::now()).is_none());
    }
}
