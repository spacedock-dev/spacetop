mod claude;
mod codex;
mod projection;
mod reducer;
mod state;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{AgentRuntime, Entity, EntityActivityAttribution, SessionScanReport};
use projection::{contains_dispatch, ProjectedToolInput};

pub use reducer::{reduce_activity, ActivityEvent, ActivityEventKind};
pub use state::{SessionEvidenceStore, SessionFileCursor, SessionScanState};

const STAGES: &[&str] = &["shape", "plan", "implement", "verify", "done", "pr-merge"];
const UNSTABLE_GENERATION: &str = "session files changed during scan; retrying";

#[derive(Debug, Clone, PartialEq)]
pub struct SessionScanRequest {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub entities: Vec<SessionScanEntity>,
    pub roots: SessionRoots,
    pub previous_state: SessionScanState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScanEntity {
    pub id: String,
    pub path: PathBuf,
    pub worktree: Option<String>,
    pub worktree_source: Option<PathBuf>,
}

impl From<&Entity> for SessionScanEntity {
    fn from(entity: &Entity) -> Self {
        Self {
            id: entity.id.clone(),
            path: entity.path.clone(),
            worktree: entity.worktree.clone(),
            worktree_source: entity.worktree_source.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionRoots {
    pub codex: Vec<PathBuf>,
    pub claude_code: Vec<PathBuf>,
}

impl SessionRoots {
    pub fn from_home(home: &Path) -> Self {
        Self {
            codex: vec![home.join(".codex/sessions")],
            claude_code: vec![home.join(".claude/projects")],
        }
    }

    pub fn from_env() -> Self {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| Self::from_home(&home))
            .unwrap_or_default()
    }

    fn all_roots(&self) -> impl Iterator<Item = (AgentRuntime, &PathBuf)> {
        self.codex
            .iter()
            .map(|path| (AgentRuntime::Codex, path))
            .chain(
                self.claude_code
                    .iter()
                    .map(|path| (AgentRuntime::ClaudeCode, path)),
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScanError {
    pub message: String,
}

impl SessionScanError {
    pub fn retry_immediately(&self) -> bool {
        self.message == UNSTABLE_GENERATION
    }
}

impl std::fmt::Display for SessionScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SessionScanError {}

/// Kept as an injectable boundary for callers. Structured activity detection
/// deliberately does not use process presence as evidence.
pub trait ProcessProbe {
    fn is_running(&self, _pid: u32) -> bool {
        false
    }

    fn command_lines(&self) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StdProcessProbe;

impl ProcessProbe for StdProcessProbe {}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionActivityScan {
    pub report: SessionScanReport,
    pub state: SessionScanState,
}

pub fn scan_local_sessions(
    request: SessionScanRequest,
) -> Result<SessionScanReport, SessionScanError> {
    scan_local_sessions_with_state(&request, &StdProcessProbe, SystemTime::now())
        .map(|scan| scan.report)
}

pub fn scan_local_sessions_with_state<P: ProcessProbe>(
    request: &SessionScanRequest,
    _process_probe: &P,
    now: SystemTime,
) -> Result<SessionActivityScan, SessionScanError> {
    scan_local_sessions_inner(request, now)
}

pub fn scan_local_sessions_with<P: ProcessProbe>(
    request: &SessionScanRequest,
    _process_probe: &P,
    now: SystemTime,
) -> Result<SessionScanReport, SessionScanError> {
    scan_local_sessions_inner(request, now).map(|scan| scan.report)
}

fn scan_local_sessions_inner(
    request: &SessionScanRequest,
    now: SystemTime,
) -> Result<SessionActivityScan, SessionScanError> {
    let loaded =
        state::load_generation(&request.roots, &request.previous_state).map_err(|error| {
            SessionScanError {
                message: match error {
                    state::LoadGenerationError::Root(message) => message,
                    state::LoadGenerationError::Unstable => UNSTABLE_GENERATION.to_string(),
                },
            }
        })?;
    let fallback_time = system_time_unix(now).unwrap_or_default();
    let records = loaded.state.evidence.all_records();
    let mut per_entity: HashMap<String, Vec<ActivityEvent>> = request
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), Vec::new()))
        .collect();
    codex::collect(
        &records,
        &request.entities,
        &request.repo_root,
        fallback_time,
        &mut per_entity,
    );
    claude::collect(
        &records,
        &request.entities,
        &request.repo_root,
        fallback_time,
        &mut per_entity,
    );

    let mut attributions: Vec<_> = request
        .entities
        .iter()
        .map(|entity| EntityActivityAttribution {
            entity_id: entity.id.clone(),
            activity: reduce_activity(
                per_entity
                    .get(&entity.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            ),
        })
        .collect();
    attributions.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    let scanned_roots = request
        .roots
        .all_roots()
        .map(|(_, root)| root.clone())
        .collect();

    Ok(SessionActivityScan {
        report: SessionScanReport {
            workflow_dir: request.workflow_dir.clone(),
            repo_root: request.repo_root.clone(),
            scanned_roots,
            attributions,
            errors: loaded.errors,
        },
        state: loaded.state,
    })
}

fn cwd_matches_entity(cwd: &Path, repo_root: &Path, entity: &SessionScanEntity) -> bool {
    if cwd == repo_root
        || cwd.starts_with(repo_root) && !cwd.starts_with(repo_root.join(".worktrees"))
    {
        return true;
    }
    if let Some(worktree) = entity.worktree.as_ref() {
        let worktree_root = repo_root.join(worktree);
        if cwd == worktree_root || cwd.starts_with(&worktree_root) {
            return true;
        }
    }
    entity
        .worktree_source
        .as_ref()
        .is_some_and(|source| source.starts_with(cwd))
}

fn call_scopes_entity(
    tool_name: &str,
    input: &ProjectedToolInput,
    entity: &SessionScanEntity,
    slug: &str,
    parent_session_id: Option<&str>,
) -> bool {
    if tool_name.ends_with("spawn_agent") || tool_name == "Agent" {
        let expected_dash = format!("spacedock-ensign-{slug}-");
        let expected_underscore = format!("spacedock_ensign_{}_", slug.replace('-', "_"));
        return input.task_name.as_deref().is_some_and(|name| {
            (name.starts_with(&expected_dash) || name.starts_with(&expected_underscore))
                && STAGES.iter().any(|stage| name.ends_with(stage))
        }) && contains_dispatch(&input.dispatches, slug, parent_session_id);
    }

    if matches!(tool_name, "exec" | "exec_command" | "Bash") {
        return input.commands.iter().any(|command| {
            command_contains_exact_path(command, &entity.path)
                || contains_dispatch_markers(command, slug, parent_session_id)
        });
    }

    [&input.file_path, &input.path, &input.uri]
        .into_iter()
        .flatten()
        .any(|value| {
            value == &entity.path.to_string_lossy()
                || contains_dispatch_markers(value, slug, parent_session_id)
        })
}

fn contains_dispatch_markers(text: &str, slug: &str, parent_session_id: Option<&str>) -> bool {
    let markers = STAGES.iter().flat_map(|stage| {
        let canonical = format!("/tmp/spacedock-dispatch/spacedock-ensign-{slug}-{stage}.md");
        let prefixed = parent_session_id.map(|parent| {
            format!("/tmp/spacedock-dispatch/{parent}-spacedock-ensign-{slug}-{stage}.md")
        });
        std::iter::once(canonical).chain(prefixed)
    });
    markers.into_iter().any(|marker| text.contains(&marker))
}

fn command_contains_exact_path(command: &str, path: &Path) -> bool {
    let path = path.to_string_lossy();
    command
        .match_indices(path.as_ref())
        .any(|(start, matched)| {
            let before = command[..start].chars().next_back();
            let after = command[start + matched.len()..].chars().next();
            before.is_none_or(is_command_boundary) && after.is_none_or(is_command_boundary)
        })
}

fn is_command_boundary(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '\'' | '"' | '`' | '=' | ':' | ';' | '|' | '&' | '(' | ')' | '[' | ']' | '{' | '}'
        )
}

fn is_gate_question(input: &ProjectedToolInput) -> bool {
    input.questions.iter().any(|question| {
        let gate_named = [&question.id, &question.header]
            .into_iter()
            .flatten()
            .any(|value| value.to_ascii_lowercase().contains("gate"));
        let labels: Vec<_> = question
            .labels
            .iter()
            .map(|label| label.to_ascii_lowercase())
            .collect();
        let accepts = labels.iter().any(|label| {
            ["approve", "pass", "accept"]
                .iter()
                .any(|term| label.contains(term))
        });
        let rejects = labels.iter().any(|label| {
            ["reject", "bounce back"]
                .iter()
                .any(|term| label.contains(term))
        });
        gate_named && accepts && rejects
    })
}

fn parse_rfc3339_unix(value: &str) -> Option<i64> {
    let (date, rest) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;

    let timezone_index = rest
        .char_indices()
        .find_map(|(index, ch)| (ch == 'Z' || ch == '+' || ch == '-').then_some(index))?;
    let (clock, zone) = rest.split_at(timezone_index);
    let mut clock_parts = clock.split(':');
    let hour = clock_parts.next()?.parse::<i64>().ok()?;
    let minute = clock_parts.next()?.parse::<i64>().ok()?;
    let second = clock_parts.next()?.split('.').next()?.parse::<i64>().ok()?;
    let offset = if zone == "Z" {
        0
    } else {
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        let mut parts = zone[1..].split(':');
        let hours = parts.next()?.parse::<i64>().ok()?;
        let minutes = parts.next().unwrap_or("0").parse::<i64>().ok()?;
        sign * (hours * 3600 + minutes * 60)
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) as i64
}

fn system_time_unix(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
}

#[cfg(test)]
static SESSION_FILE_PARSE_STARTS: LazyLock<Mutex<HashMap<PathBuf, Vec<u64>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
fn record_session_file_parse(path: &Path, start: u64) {
    SESSION_FILE_PARSE_STARTS
        .lock()
        .expect("session parse count lock")
        .entry(path.to_path_buf())
        .or_default()
        .push(start);
}

#[cfg(test)]
fn session_file_parse_starts(path: &Path) -> Vec<u64> {
    SESSION_FILE_PARSE_STARTS
        .lock()
        .expect("session parse count lock")
        .get(path)
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;
    use crate::domain::{ActivityHandler, EntityActivity};

    fn event(
        at: i64,
        runtime: AgentRuntime,
        session: &str,
        kind: ActivityEventKind,
    ) -> ActivityEvent {
        ActivityEvent {
            runtime,
            session_id: session.to_string(),
            updated_unix: at,
            source: PathBuf::from("manual"),
            byte_offset: at as u64,
            evidence_kind_rank: 0,
            kind,
        }
    }

    #[test]
    fn reducer_is_deterministic_and_preserves_precedence_and_handoff() {
        let events = vec![
            event(
                1,
                AgentRuntime::Codex,
                "fo",
                ActivityEventKind::FirstOfficerStarted,
            ),
            event(
                2,
                AgentRuntime::Codex,
                "worker",
                ActivityEventKind::WorkerStarted,
            ),
            event(
                3,
                AgentRuntime::Codex,
                "worker",
                ActivityEventKind::WorkerStopped,
            ),
        ];
        assert_eq!(reduce_activity(&events).status_label(), "running · FO");
        let mut shuffled = vec![events[2].clone(), events[0].clone(), events[1].clone()];
        assert_eq!(reduce_activity(&shuffled), reduce_activity(&events));
        shuffled.push(event(
            4,
            AgentRuntime::ClaudeCode,
            "worker-2",
            ActivityEventKind::WorkerStarted,
        ));
        shuffled.push(event(
            5,
            AgentRuntime::Codex,
            "fo",
            ActivityEventKind::HumanGateOpened {
                call_id: "gate".to_string(),
            },
        ));
        assert_eq!(reduce_activity(&shuffled).status_label(), "human-gate");

        let mut same_time_start = event(
            10,
            AgentRuntime::Codex,
            "same-time",
            ActivityEventKind::WorkerStarted,
        );
        same_time_start.byte_offset = 1;
        let mut same_time_stop = event(
            10,
            AgentRuntime::Codex,
            "same-time",
            ActivityEventKind::WorkerStopped,
        );
        same_time_stop.byte_offset = 2;
        assert_eq!(
            reduce_activity(&[same_time_stop, same_time_start]).status_label(),
            "idle",
            "source byte order must settle equal-timestamp lifecycle records"
        );
    }

    fn fixture_root(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/session-activity")
            .join(name)
    }

    fn entity() -> SessionScanEntity {
        SessionScanEntity {
            id: "075".to_string(),
            path: PathBuf::from("/repo/docs/state/stable-agent-activity-detection.md"),
            worktree: Some(".worktrees/stable-agent-activity-detection".to_string()),
            worktree_source: None,
        }
    }

    fn fixture_request(runtime: AgentRuntime, fixture: &str) -> SessionScanRequest {
        let roots = match runtime {
            AgentRuntime::Codex => SessionRoots {
                codex: vec![fixture_root(fixture)],
                claude_code: Vec::new(),
            },
            AgentRuntime::ClaudeCode => SessionRoots {
                codex: Vec::new(),
                claude_code: vec![fixture_root(fixture)],
            },
        };
        SessionScanRequest {
            workflow_dir: PathBuf::from("/repo/docs"),
            repo_root: PathBuf::from("/repo"),
            entities: vec![entity()],
            roots,
            previous_state: SessionScanState::default(),
        }
    }

    fn legacy_fixture_request(runtime: AgentRuntime, fixture: &str) -> SessionScanRequest {
        let mut request = fixture_request(runtime, fixture);
        request.entities = vec![SessionScanEntity {
            id: "069".to_string(),
            path: PathBuf::from("/repo/docs/state/detect-entity-activity-state.md"),
            worktree: None,
            worktree_source: None,
        }];
        request
    }

    #[test]
    fn codex_v2_parent_start_fixture_runs_and_exact_mismatches_fail_closed() {
        let report = scan_local_sessions_with(
            &fixture_request(AgentRuntime::Codex, "codex-v2-worker-open"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("fixture");
        assert_eq!(
            report.attributions[0].activity,
            EntityActivity::Running {
                handler: ActivityHandler::Worker,
                runtime: AgentRuntime::Codex,
                session_id: "codex-child".to_string(),
                updated_unix: 2,
            }
        );

        for fixture in [
            "codex-v2-wrong-parent",
            "codex-v2-wrong-path",
            "codex-v2-wrong-cwd",
        ] {
            let report = scan_local_sessions_with(
                &fixture_request(AgentRuntime::Codex, fixture),
                &StdProcessProbe,
                UNIX_EPOCH,
            )
            .expect("negative fixture");
            assert_eq!(
                report.attributions[0].activity.status_label(),
                "idle",
                "{fixture} must fail closed"
            );
        }
    }

    #[test]
    fn legacy_worker_correlations_and_terminals_remain_supported() {
        let codex_open = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::Codex, "codex-worker-open"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("legacy Codex");
        assert_eq!(
            codex_open.attributions[0].activity.session_id(),
            Some("codex-worker-redacted")
        );
        let codex_complete = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::Codex, "codex-worker-complete"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("legacy Codex complete");
        assert_eq!(
            codex_complete.attributions[0].activity.status_label(),
            "idle"
        );
        let codex_unlinked = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::Codex, "codex-worker-unlinked"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("legacy Codex unlinked");
        assert_eq!(
            codex_unlinked.attributions[0].activity.status_label(),
            "idle"
        );

        let claude_open = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::ClaudeCode, "claude-worker-open"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("legacy Claude");
        assert_eq!(
            claude_open.attributions[0].activity.session_id(),
            Some("claude-worker-redacted")
        );
        let claude_idle = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::ClaudeCode, "claude-worker-idle"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("legacy Claude idle");
        assert_eq!(claude_idle.attributions[0].activity.status_label(), "idle");
    }

    #[test]
    fn claude_modern_fixture_correlates_prefixed_dispatch_meta_and_sidechain() {
        let report = scan_local_sessions_with(
            &fixture_request(AgentRuntime::ClaudeCode, "claude-modern-worker-open"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("fixture");
        assert_eq!(
            report.attributions[0].activity.session_id(),
            Some("claude-worker")
        );
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "running · worker"
        );
    }

    #[test]
    fn claude_modern_correlation_fails_closed_across_cwd_parent_and_ambiguous_calls() {
        for fixture in [
            "claude-modern-wrong-cwd",
            "claude-modern-cross-parent",
            "claude-modern-duplicate-call",
        ] {
            let report = scan_local_sessions_with(
                &fixture_request(AgentRuntime::ClaudeCode, fixture),
                &StdProcessProbe,
                UNIX_EPOCH,
            )
            .expect("negative fixture");
            assert_eq!(
                report.attributions[0].activity.status_label(),
                "idle",
                "{fixture} must fail closed"
            );
        }
    }

    #[test]
    fn existing_first_officer_and_gate_evidence_keeps_exact_scope() {
        for fixture in [
            "codex-fo-exec",
            "codex-fo-exec-nested",
            "codex-fo-exec-command",
        ] {
            let report = scan_local_sessions_with(
                &legacy_fixture_request(AgentRuntime::Codex, fixture),
                &StdProcessProbe,
                UNIX_EPOCH,
            )
            .expect("Codex FO fixture");
            assert_eq!(
                report.attributions[0].activity.status_label(),
                "running · FO"
            );
        }
        let text_only = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::Codex, "codex-fo-exec-text-only"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("text-only fixture");
        assert_eq!(text_only.attributions[0].activity.status_label(), "idle");

        let codex_gate = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::Codex, "codex-fo-gate"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("Codex gate");
        assert_eq!(
            codex_gate.attributions[0].activity.status_label(),
            "human-gate"
        );
        let claude_gate = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::ClaudeCode, "claude-fo-gate"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("Claude gate");
        assert_eq!(
            claude_gate.attributions[0].activity.status_label(),
            "human-gate"
        );
        let claude_complete = scan_local_sessions_with(
            &legacy_fixture_request(AgentRuntime::ClaudeCode, "claude-fo-complete"),
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("Claude complete");
        assert_eq!(
            claude_complete.attributions[0].activity.status_label(),
            "idle"
        );
    }

    #[test]
    fn codex_evidence_survives_unchanged_partial_truncate_rotate_and_delete() {
        let temp = tempfile::tempdir().expect("temp");
        let child = temp.path().join("child.jsonl");
        let parent = temp.path().join("parent.jsonl");
        fs::copy(fixture_root("codex-v2-worker-open/child.jsonl"), &child).expect("child");
        fs::copy(fixture_root("codex-v2-worker-open/parent.jsonl"), &parent).expect("parent");
        let base = SessionScanRequest {
            roots: SessionRoots {
                codex: vec![temp.path().to_path_buf()],
                claude_code: Vec::new(),
            },
            ..fixture_request(AgentRuntime::Codex, "codex-v2-worker-open")
        };
        let first =
            scan_local_sessions_with_state(&base, &StdProcessProbe, UNIX_EPOCH).expect("first");
        assert_eq!(
            first.report.attributions[0].activity.status_label(),
            "running · worker"
        );
        let unchanged = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: first.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("unchanged");
        assert_eq!(
            unchanged.report.attributions[0].activity.status_label(),
            "running · worker"
        );

        let mut append = fs::OpenOptions::new()
            .append(true)
            .open(&child)
            .expect("append");
        write!(append, "{{\"timestamp\":3").expect("partial");
        let partial = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: unchanged.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("partial");
        assert_eq!(
            partial.report.attributions[0].activity.status_label(),
            "running · worker"
        );

        fs::write(
            &child,
            "{\"timestamp\":1,\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-child\",\"cwd\":\"/repo/.worktrees/stable-agent-activity-detection\",\"source\":{\"subagent\":{\"thread_spawn\":{\"agent_path\":\"/root/spacedock_ensign_stable_agent_activity_detection_implement\",\"parent_thread_id\":\"codex-parent\"}}}}}\n",
        )
        .expect("truncate");
        let truncated = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: partial.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("truncated");
        assert_eq!(
            truncated.report.attributions[0].activity.status_label(),
            "running · worker",
            "truncation without a terminal fact must retain the proven start"
        );
        fs::remove_file(&child).expect("delete");
        fs::remove_file(&parent).expect("delete");
        let deleted = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: truncated.state,
                ..base
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("deleted");
        assert_eq!(
            deleted.report.attributions[0].activity.status_label(),
            "running · worker",
            "deletion alone must not synthesize a stop"
        );
    }

    #[test]
    fn exact_codex_terminal_closes_retained_worker() {
        let temp = tempfile::tempdir().expect("temp");
        let child = temp.path().join("child.jsonl");
        fs::copy(fixture_root("codex-v2-worker-open/child.jsonl"), &child).expect("child");
        fs::copy(
            fixture_root("codex-v2-worker-open/parent.jsonl"),
            temp.path().join("parent.jsonl"),
        )
        .expect("parent");
        let base = SessionScanRequest {
            roots: SessionRoots {
                codex: vec![temp.path().to_path_buf()],
                claude_code: Vec::new(),
            },
            ..fixture_request(AgentRuntime::Codex, "codex-v2-worker-open")
        };
        let first =
            scan_local_sessions_with_state(&base, &StdProcessProbe, UNIX_EPOCH).expect("first");
        let first_cursor = first.state.files[&child].cursor;
        let second = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: first.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("second");
        let third = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: second.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("third");
        let mut append = fs::OpenOptions::new()
            .append(true)
            .open(&child)
            .expect("append");
        writeln!(append, r#"{{"timestamp":4,"type":"event_msg","payload":{{"type":"task_complete","turn_id":"turn"}}}}"#).expect("terminal");
        let complete = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: third.state,
                ..base
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("complete");
        assert_eq!(
            complete.report.attributions[0].activity.status_label(),
            "idle"
        );
        println!("Codex replay: running, running, running, idle");
        assert_eq!(session_file_parse_starts(&child), vec![0, first_cursor]);
    }

    #[test]
    fn malformed_claude_meta_does_not_replace_valid_identity() {
        let temp = tempfile::tempdir().expect("temp");
        copy_fixture_tree(&fixture_root("claude-modern-worker-open"), temp.path());
        let base = SessionScanRequest {
            roots: SessionRoots {
                codex: Vec::new(),
                claude_code: vec![temp.path().to_path_buf()],
            },
            ..fixture_request(AgentRuntime::ClaudeCode, "claude-modern-worker-open")
        };
        let first =
            scan_local_sessions_with_state(&base, &StdProcessProbe, UNIX_EPOCH).expect("first");
        let meta = temp.path().join("claude-parent/subagents/worker.meta.json");
        fs::write(&meta, "{").expect("malformed");
        let malformed = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: first.state,
                ..base
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("malformed");
        assert_eq!(
            malformed.report.attributions[0].activity.status_label(),
            "running · worker"
        );
        assert!(!malformed.report.errors.is_empty());
    }

    #[test]
    fn claude_replay_is_stable_then_stops_reopens_and_stops() {
        let temp = tempfile::tempdir().expect("temp");
        copy_fixture_tree(&fixture_root("claude-modern-worker-open"), temp.path());
        let parent = temp.path().join("parent.jsonl");
        let child = temp.path().join("claude-parent/subagents/worker.jsonl");
        let base = SessionScanRequest {
            roots: SessionRoots {
                codex: Vec::new(),
                claude_code: vec![temp.path().to_path_buf()],
            },
            ..fixture_request(AgentRuntime::ClaudeCode, "claude-modern-worker-open")
        };
        let first =
            scan_local_sessions_with_state(&base, &StdProcessProbe, UNIX_EPOCH).expect("first");
        let second = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: first.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("second");
        let third = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: second.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("third");
        assert_eq!(
            [
                first.report.attributions[0].activity.status_label(),
                second.report.attributions[0].activity.status_label(),
                third.report.attributions[0].activity.status_label(),
            ],
            ["running · worker"; 3]
        );

        let mut parent_append = fs::OpenOptions::new()
            .append(true)
            .open(&parent)
            .expect("parent append");
        writeln!(
            parent_append,
            r#"{{"timestamp":3,"type":"user","sessionId":"claude-parent","cwd":"/repo","isSidechain":false,"message":{{"content":"<teammate-message>{{\"type\":\"idle_notification\",\"from\":\"spacedock-ensign-stable-agent-activity-detection-implement\",\"idleReason\":\"available\"}}</teammate-message>"}}}}"#
        )
        .expect("idle");
        let stopped = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: third.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("stopped");
        assert_eq!(
            stopped.report.attributions[0].activity.status_label(),
            "idle"
        );

        writeln!(
            parent_append,
            r#"{{"timestamp":4,"type":"user","sessionId":"claude-parent","cwd":"/repo","isSidechain":false,"message":{{"content":"<teammate-message>{{\"type\":\"teammate_message\",\"from\":\"spacedock-ensign-stable-agent-activity-detection-implement\"}}</teammate-message>"}}}}"#
        )
        .expect("follow-up boundary");
        let mut child_append = fs::OpenOptions::new()
            .append(true)
            .open(&child)
            .expect("child append");
        writeln!(
            child_append,
            r#"{{"timestamp":5,"type":"assistant","sessionId":"claude-child","cwd":"/repo/.worktrees/stable-agent-activity-detection","isSidechain":true,"agentId":"claude-worker","message":{{"content":[{{"type":"text","text":"follow-up"}}],"stop_reason":null}}}}"#
        )
        .expect("follow-up assistant");
        let reopened = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: stopped.state,
                ..base.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("reopened");
        assert_eq!(
            reopened.report.attributions[0].activity.status_label(),
            "running · worker"
        );

        writeln!(
            parent_append,
            r#"{{"timestamp":6,"type":"user","sessionId":"claude-parent","cwd":"/repo","isSidechain":false,"message":{{"content":"<teammate-message>{{\"type\":\"idle_notification\",\"from\":\"spacedock-ensign-stable-agent-activity-detection-implement\",\"idleReason\":\"available\"}}</teammate-message>"}}}}"#
        )
        .expect("second idle");
        let final_stop = scan_local_sessions_with_state(
            &SessionScanRequest {
                previous_state: reopened.state,
                ..base
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("final stop");
        assert_eq!(
            final_stop.report.attributions[0].activity.status_label(),
            "idle"
        );
        println!("Claude replay: running, running, idle, running, idle");
    }

    fn copy_fixture_tree(source: &Path, destination: &Path) {
        for entry in walkdir::WalkDir::new(source) {
            let entry = entry.expect("fixture entry");
            let relative = entry.path().strip_prefix(source).expect("relative");
            let target = destination.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target).expect("dir");
            } else {
                fs::copy(entry.path(), target).expect("file");
            }
        }
    }

    #[test]
    fn unreadable_root_is_a_scan_failure_instead_of_false_idle() {
        let temp = tempfile::tempdir().expect("temp");
        let not_a_directory = temp.path().join("session-root");
        fs::write(&not_a_directory, "not a directory").expect("fixture");
        let result = scan_local_sessions_with(
            &SessionScanRequest {
                roots: SessionRoots {
                    codex: vec![not_a_directory],
                    claude_code: Vec::new(),
                },
                ..fixture_request(AgentRuntime::Codex, "codex-v2-worker-open")
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        );
        assert!(result.is_err());
    }
}
