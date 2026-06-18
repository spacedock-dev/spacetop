use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use crate::domain::{
    AgentKind, AgentSessionEvidence, AgentSessionLiveness, AttributionConfidence, Entity,
    EntitySessionAttribution, SessionScanReport,
};
use crate::entity_identity::entity_slug;

const RECENT_ACTIVITY_WINDOW: Duration = Duration::from_secs(30 * 60);
const OBSERVED_RUNNING_WINDOW: Duration = Duration::from_secs(2 * 60);
const MAX_SCAN_FILE_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct SessionScanRequest {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub entities: Vec<SessionScanEntity>,
    pub roots: SessionRoots,
    pub previous_session_files: HashMap<PathBuf, SessionFileSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionFileSnapshot {
    modified_unix: Option<i64>,
    len: u64,
    observed_running_until_unix: Option<i64>,
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

    fn all_roots(&self) -> Vec<(AgentKind, PathBuf)> {
        self.codex
            .iter()
            .cloned()
            .map(|path| (AgentKind::Codex, path))
            .chain(
                self.claude_code
                    .iter()
                    .cloned()
                    .map(|path| (AgentKind::ClaudeCode, path)),
            )
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScanError {
    pub message: String,
}

impl std::fmt::Display for SessionScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SessionScanError {}

pub trait ProcessProbe {
    fn is_running(&self, pid: u32) -> bool;

    fn command_lines(&self) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StdProcessProbe;

impl ProcessProbe for StdProcessProbe {
    fn is_running(&self, pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("pid=")
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    }

    fn command_lines(&self) -> Vec<String> {
        Command::new("ps")
            .arg("-axo")
            .arg("command=")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub fn scan_local_sessions(
    request: SessionScanRequest,
) -> Result<SessionScanReport, SessionScanError> {
    scan_local_sessions_with_snapshots(&request, &StdProcessProbe, SystemTime::now())
        .map(|result| result.report)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionActivityScan {
    pub report: SessionScanReport,
    pub session_files: HashMap<PathBuf, SessionFileSnapshot>,
}

pub fn scan_local_sessions_with_snapshots<P: ProcessProbe>(
    request: &SessionScanRequest,
    process_probe: &P,
    now: SystemTime,
) -> Result<SessionActivityScan, SessionScanError> {
    scan_local_sessions_inner(request, process_probe, now)
}

pub fn scan_local_sessions_with<P: ProcessProbe>(
    request: &SessionScanRequest,
    process_probe: &P,
    now: SystemTime,
) -> Result<SessionScanReport, SessionScanError> {
    scan_local_sessions_inner(request, process_probe, now).map(|result| result.report)
}

fn scan_local_sessions_inner<P: ProcessProbe>(
    request: &SessionScanRequest,
    process_probe: &P,
    now: SystemTime,
) -> Result<SessionActivityScan, SessionScanError> {
    let mut errors = Vec::new();
    let mut per_entity: HashMap<String, Vec<AgentSessionEvidence>> = HashMap::new();
    let mut session_files = HashMap::new();
    let mut run_state_classifier = RunStateClassifier {
        process_probe,
        command_lines: None,
        pid_cache: HashMap::new(),
        session_cache: HashMap::new(),
    };
    let root_pairs = request.roots.all_roots();

    for (agent, root) in &root_pairs {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|entry| !is_pruned_dir(entry.path()))
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    errors.push(format!("{} scan skipped entry: {err}", agent.label()));
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_session_file(entry.path()) {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(err) => {
                    errors.push(format!(
                        "{} scan could not read metadata for {}: {err}",
                        agent.label(),
                        entry.path().display()
                    ));
                    continue;
                }
            };
            if metadata.len() > MAX_SCAN_FILE_BYTES {
                continue;
            }
            let previous_snapshot = request.previous_session_files.get(entry.path());
            let mut snapshot = SessionFileSnapshot::from_metadata(&metadata);
            let changed_since_last_scan =
                previous_snapshot.is_some_and(|previous| previous.has_different_metadata(snapshot));
            if changed_since_last_scan {
                snapshot.observed_running_until_unix =
                    Some(system_time_unix(now + OBSERVED_RUNNING_WINDOW).unwrap_or(i64::MAX));
            } else {
                snapshot.observed_running_until_unix =
                    previous_snapshot.and_then(|previous| previous.observed_running_until_unix);
            }
            let content = match fs::read_to_string(entry.path()) {
                Ok(content) => content,
                Err(err) => {
                    errors.push(format!(
                        "{} scan could not read {}: {err}",
                        agent.label(),
                        entry.path().display()
                    ));
                    continue;
                }
            };
            session_files.insert(entry.path().to_path_buf(), snapshot);
            let activity_time = metadata.modified().ok();
            let pid = extract_pid(&content);
            let live_session_key = live_session_key(*agent, entry.path(), &content);
            let matches: Vec<_> = request
                .entities
                .iter()
                .filter_map(|entity| {
                    match_entity(entity, &request.workflow_dir, &request.repo_root, &content).map(
                        |(confidence, matched_worktree)| {
                            (entity.id.clone(), confidence, matched_worktree)
                        },
                    )
                })
                .collect();
            if matches.is_empty() {
                continue;
            }
            let observed_file_activity_is_running = snapshot
                .observed_running_until_unix
                .is_some_and(|until| system_time_unix(now).is_some_and(|now| now <= until));
            let liveness = run_state_classifier.classify(SessionLivenessInput {
                agent: *agent,
                live_session_key: live_session_key.as_deref(),
                pid,
                observed_file_activity_is_running,
                observed_running_until_unix: snapshot.observed_running_until_unix,
                activity_time,
                now,
            });
            for (entity_id, confidence, matched_worktree) in matches {
                let evidence = AgentSessionEvidence {
                    agent: *agent,
                    session_id: session_id(entry.path()),
                    display_name: display_name(*agent, entry.path(), &content),
                    confidence,
                    liveness: liveness.clone(),
                    latest_activity_unix: activity_time.and_then(system_time_unix),
                    matched_worktree,
                };
                per_entity.entry(entity_id).or_default().push(evidence);
            }
        }
    }

    let mut attributions: Vec<EntitySessionAttribution> = per_entity
        .into_iter()
        .map(|(entity_id, mut evidence)| {
            evidence.sort_by_key(|evidence| {
                (
                    std::cmp::Reverse(evidence.run_state()),
                    std::cmp::Reverse(evidence.confidence),
                    std::cmp::Reverse(evidence.latest_activity_unix),
                )
            });
            evidence.truncate(3);
            EntitySessionAttribution {
                entity_id,
                evidence,
            }
        })
        .collect();
    attributions.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));

    Ok(SessionActivityScan {
        report: SessionScanReport {
            workflow_dir: request.workflow_dir.clone(),
            repo_root: request.repo_root.clone(),
            scanned_roots: root_pairs.into_iter().map(|(_, root)| root).collect(),
            attributions,
            errors,
        },
        session_files,
    })
}

impl SessionFileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            modified_unix: metadata.modified().ok().and_then(system_time_unix),
            len: metadata.len(),
            observed_running_until_unix: None,
        }
    }

    fn has_different_metadata(self, other: Self) -> bool {
        self.modified_unix != other.modified_unix || self.len != other.len
    }
}

fn is_pruned_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| matches!(name, ".git" | "node_modules" | "target"))
}

fn is_session_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("jsonl" | "json" | "log" | "md")
    )
}

struct RunStateClassifier<'a, P: ProcessProbe> {
    process_probe: &'a P,
    command_lines: Option<Vec<String>>,
    pid_cache: HashMap<u32, bool>,
    session_cache: HashMap<(AgentKind, String), bool>,
}

struct SessionLivenessInput<'a> {
    agent: AgentKind,
    live_session_key: Option<&'a str>,
    pid: Option<u32>,
    observed_file_activity_is_running: bool,
    observed_running_until_unix: Option<i64>,
    activity_time: Option<SystemTime>,
    now: SystemTime,
}

impl<'a, P: ProcessProbe> RunStateClassifier<'a, P> {
    fn classify(&mut self, input: SessionLivenessInput<'_>) -> AgentSessionLiveness {
        if let Some(pid) = input.pid.filter(|pid| {
            *self
                .pid_cache
                .entry(*pid)
                .or_insert_with(|| self.process_probe.is_running(*pid))
        }) {
            return AgentSessionLiveness::LivePid { pid };
        }
        if let Some(session_key) = input.live_session_key.filter(|key| {
            let command_lines = self
                .command_lines
                .get_or_insert_with(|| self.process_probe.command_lines());
            *self
                .session_cache
                .entry((input.agent, (*key).to_string()))
                .or_insert_with(|| has_live_session_command(input.agent, key, command_lines))
        }) {
            return AgentSessionLiveness::LiveResumeCommand {
                session_key: session_key.to_string(),
            };
        }
        if input.observed_file_activity_is_running {
            return AgentSessionLiveness::ObservedSessionWrite {
                until_unix: input.observed_running_until_unix.unwrap_or_default(),
            };
        }
        if input.activity_time.is_some_and(|time| {
            input
                .now
                .duration_since(time)
                .map(|age| age <= RECENT_ACTIVITY_WINDOW)
                .unwrap_or(false)
        }) {
            return AgentSessionLiveness::RecentMtime;
        }
        AgentSessionLiveness::Stale
    }
}

fn match_entity(
    entity: &SessionScanEntity,
    workflow_dir: &Path,
    repo_root: &Path,
    content: &str,
) -> Option<(AttributionConfidence, Option<PathBuf>)> {
    if has_conflicting_dispatch_assignment(entity, content) {
        return None;
    }

    let worktree = entity
        .worktree
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let worktree_paths = worktree
        .map(|value| candidate_paths(repo_root, value))
        .unwrap_or_default();
    for path in &worktree_paths {
        if content_mentions_path(content, path) {
            return Some((AttributionConfidence::High, Some(path.clone())));
        }
    }
    if entity
        .worktree_source
        .as_ref()
        .is_some_and(|path| content_mentions_path(content, path))
    {
        return Some((AttributionConfidence::High, entity.worktree_source.clone()));
    }

    explicit_entity_reference_paths(entity, workflow_dir, repo_root)
        .into_iter()
        .any(|path| content_mentions_path(content, &path))
        .then_some((AttributionConfidence::Medium, None))
}

fn content_mentions_path(content: &str, path: &Path) -> bool {
    let path_text = path.to_string_lossy();
    content.contains(path_text.as_ref())
}

fn has_conflicting_dispatch_assignment(entity: &SessionScanEntity, content: &str) -> bool {
    let Some(entity_slug) = entity_slug(&entity.path) else {
        return false;
    };
    dispatch_assignment_slugs(content).any(|assigned_slug| assigned_slug != entity_slug)
}

fn dispatch_assignment_slugs(content: &str) -> impl Iterator<Item = String> + '_ {
    const PREFIX: &str = "/tmp/spacedock-dispatch/spacedock-ensign-";

    content.match_indices(PREFIX).filter_map(|(start, _)| {
        let after_prefix = &content[start + PREFIX.len()..];
        let end = after_prefix.find(".md")?;
        let stem = &after_prefix[..end];
        dispatch_slug_from_stem(stem).map(str::to_string)
    })
}

fn dispatch_slug_from_stem(stem: &str) -> Option<&str> {
    const STAGES: &[&str] = &["shape", "plan", "implement", "verify", "done", "pr-merge"];

    STAGES
        .iter()
        .find_map(|stage| stem.strip_suffix(&format!("-{stage}")))
        .filter(|slug| !slug.is_empty())
}

#[cfg(test)]
fn contains_entity_id(content: &str, id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && content.match_indices(id).any(|(start, _)| {
            let before = content[..start].chars().next_back();
            let after = content[start + id.len()..].chars().next();
            !before.is_some_and(|ch| ch.is_ascii_alphanumeric())
                && !after.is_some_and(|ch| ch.is_ascii_alphanumeric())
        })
}

fn explicit_entity_reference_paths(
    entity: &SessionScanEntity,
    workflow_dir: &Path,
    repo_root: &Path,
) -> Vec<PathBuf> {
    let path = &entity.path;
    let absolute = if path.is_absolute() {
        path.clone()
    } else {
        repo_root.join(path)
    };
    let repo_relative = absolute.strip_prefix(repo_root).ok().map(Path::to_path_buf);
    let workflow_relative = absolute
        .strip_prefix(workflow_dir)
        .ok()
        .map(Path::to_path_buf);
    let file_name = path
        .file_name()
        .filter(|name| *name != "index.md")
        .map(PathBuf::from);

    let mut seen = HashSet::new();
    [
        Some(path.clone()),
        Some(absolute),
        repo_relative,
        workflow_relative,
        file_name,
    ]
    .into_iter()
    .flatten()
    .filter(|path| !path.as_os_str().is_empty())
    .filter(|path| seen.insert(path.clone()))
    .collect()
}

fn candidate_paths(repo_root: &Path, raw: &str) -> Vec<PathBuf> {
    let path = PathBuf::from(raw);
    let absolute = if path.is_absolute() {
        path.clone()
    } else {
        repo_root.join(&path)
    };
    let mut seen = HashSet::new();
    [path, absolute]
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn extract_pid(content: &str) -> Option<u32> {
    let key = "\"pid\"";
    let start = content.find(key)? + key.len();
    let after_key = &content[start..];
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let digits: String = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn live_session_key(agent: AgentKind, path: &Path, content: &str) -> Option<String> {
    match agent {
        AgentKind::Codex => uuid_in_value(&session_id(path)).map(str::to_string),
        AgentKind::ClaudeCode => extract_json_string(content, "sessionId")
            .and_then(|value| is_uuid_like(&value).then_some(value))
            .or_else(|| is_uuid_like(&session_id(path)).then(|| session_id(path))),
    }
}

fn has_live_session_command(agent: AgentKind, session_key: &str, command_lines: &[String]) -> bool {
    command_lines
        .iter()
        .any(|line| command_matches_live_session(agent, session_key, line))
}

fn command_matches_live_session(agent: AgentKind, session_key: &str, command: &str) -> bool {
    let tokens = command_tokens(command);
    let Some(binary) = tokens.first() else {
        return false;
    };
    match agent {
        AgentKind::Codex => {
            command_basename(binary) == "codex"
                && (has_arg_pair(&tokens, "resume", session_key)
                    || has_arg_pair(&tokens, "--resume", session_key)
                    || has_arg_value(&tokens, "--resume", session_key))
        }
        AgentKind::ClaudeCode => {
            command_basename(binary) == "claude"
                && (has_arg_pair(&tokens, "--resume", session_key)
                    || has_arg_value(&tokens, "--resume", session_key))
        }
    }
}

fn command_tokens(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|ch| matches!(ch, '"' | '\''))
                .to_string()
        })
        .collect()
}

fn command_basename(command: &str) -> &str {
    Path::new(command)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(command)
}

fn has_arg_pair(tokens: &[String], flag: &str, value: &str) -> bool {
    tokens
        .windows(2)
        .any(|pair| pair[0] == flag && pair[1] == value)
}

fn has_arg_value(tokens: &[String], flag: &str, value: &str) -> bool {
    let prefix = format!("{flag}=");
    tokens
        .iter()
        .any(|token| token.strip_prefix(&prefix) == Some(value))
}

fn uuid_in_value(value: &str) -> Option<&str> {
    if is_uuid_like(value) {
        return Some(value);
    }
    value
        .as_bytes()
        .windows(36)
        .position(is_uuid_like_bytes)
        .map(|start| &value[start..start + 36])
}

fn is_uuid_like(value: &str) -> bool {
    value.len() == 36 && is_uuid_like_bytes(value.as_bytes())
}

fn is_uuid_like_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn display_name(agent: AgentKind, path: &Path, content: &str) -> Option<String> {
    match agent {
        AgentKind::Codex => extract_json_string(content, "agent_nickname")
            .or_else(|| extract_json_string(content, "model")),
        AgentKind::ClaudeCode => path
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .map(str::to_string),
    }
}

fn extract_json_string(content: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = content.find(&needle)? + needle.len();
    let after_key = &content[start..];
    let quote_start = after_key.find('"')? + 1;
    let value = &after_key[quote_start..];
    let quote_end = value.find('"')?;
    let value = value[..quote_end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn session_id(path: &Path) -> String {
    path.file_stem()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn system_time_unix(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AgentSessionState;
    use std::fs;

    struct FixtureProbe {
        running: HashSet<u32>,
    }

    impl ProcessProbe for FixtureProbe {
        fn is_running(&self, pid: u32) -> bool {
            self.running.contains(&pid)
        }
    }

    struct CountingProbe {
        running: HashSet<u32>,
        calls: std::cell::Cell<usize>,
    }

    impl ProcessProbe for CountingProbe {
        fn is_running(&self, pid: u32) -> bool {
            self.calls.set(self.calls.get() + 1);
            self.running.contains(&pid)
        }
    }

    struct CommandProbe {
        running: HashSet<u32>,
        command_lines: Vec<String>,
    }

    impl ProcessProbe for CommandProbe {
        fn is_running(&self, pid: u32) -> bool {
            self.running.contains(&pid)
        }

        fn command_lines(&self) -> Vec<String> {
            self.command_lines.clone()
        }
    }

    struct CountingCommandProbe {
        command_line_calls: std::cell::Cell<usize>,
    }

    impl ProcessProbe for CountingCommandProbe {
        fn is_running(&self, _pid: u32) -> bool {
            false
        }

        fn command_lines(&self) -> Vec<String> {
            self.command_line_calls
                .set(self.command_line_calls.get() + 1);
            Vec::new()
        }
    }

    fn entity(id: &str, worktree: Option<&str>) -> SessionScanEntity {
        SessionScanEntity::from(&Entity {
            path: PathBuf::from(format!("{id}-task.md")),
            id: id.to_string(),
            title: format!("task {id}"),
            status: "implement".to_string(),
            source: None,
            started: None,
            completed: None,
            verdict: None,
            score: None,
            worktree: worktree.map(str::to_string),
            issue: None,
            pr: None,
            body: "body".to_string(),
            worktree_source: None,
            main_body: None,
        })
    }

    fn entity_with_path(id: &str, path: impl Into<PathBuf>) -> SessionScanEntity {
        let mut entity = entity(id, None);
        entity.path = path.into();
        entity
    }

    fn write_session(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, body).expect("write session");
    }

    #[test]
    fn running_codex_worktree_match_is_high_confidence_active() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let worktree = repo.join(".worktrees/task-065");
        write_session(
            &root.join("2026/06/session-1.jsonl"),
            &format!(
                r#"{{"pid":4242,"agent_nickname":"Mendel","workdir":"{}"}}"#,
                worktree.display()
            ),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::from([4242]),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        let attribution = &report.attributions[0];
        assert_eq!(attribution.entity_id, "065");
        assert!(attribution.has_active_marker());
        assert_eq!(attribution.evidence[0].agent, AgentKind::Codex);
        assert_eq!(
            attribution.evidence[0].display_name.as_deref(),
            Some("Mendel")
        );
    }

    #[test]
    fn running_claude_code_worktree_match_is_high_confidence_active() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("claude/projects/project-a");
        let worktree = repo.join(".worktrees/task-065");
        write_session(
            &root.join("session.jsonl"),
            &format!(r#"{{"pid":5150,"cwd":"{}"}}"#, worktree.display()),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: Vec::new(),
                claude_code: vec![root],
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::from([5150]),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions[0].has_active_marker());
        assert_eq!(
            report.attributions[0].evidence[0].agent,
            AgentKind::ClaudeCode
        );
    }

    #[test]
    fn running_medium_confidence_match_is_active() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("repo-session.jsonl"),
            &format!(
                r#"{{"pid":4242,"agent_nickname":"Mendel","workdir":"{}","note":"065-task.md"}}"#,
                repo.display()
            ),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::from([4242]),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert_eq!(
            report.attributions[0].evidence[0].confidence,
            AttributionConfidence::Medium
        );
        assert!(report.attributions[0].has_active_marker());
    }

    #[test]
    fn pidless_codex_resume_command_marks_session_running() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let session_uuid = "019ed968-6e77-7d71-9386-aae754c6c8be";
        let session_stem = format!("rollout-2026-06-18T14-26-00-{session_uuid}");
        write_session(
            &root.join(format!("{session_stem}.jsonl")),
            &format!(r#"{{"workdir":"{}","note":"065-task.md"}}"#, repo.display()),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: vec![format!("/opt/homebrew/bin/codex resume {session_uuid}")],
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert_eq!(
            report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Running
        );
        assert!(report.attributions[0].has_active_marker());
    }

    #[test]
    fn pidless_claude_resume_command_marks_session_running() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("claude/projects/project-a");
        let session_uuid = "f0d61fc8-8903-43c0-b916-4b02c45bf441";
        let worktree = repo.join(".worktrees/task-065");
        write_session(
            &root.join("session.jsonl"),
            &format!(
                r#"{{"sessionId":"{session_uuid}","cwd":"{}"}}"#,
                worktree.display()
            ),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: Vec::new(),
                claude_code: vec![root],
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: vec![format!("claude --resume {session_uuid}")],
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert_eq!(
            report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Running
        );
        assert!(report.attributions[0].has_active_marker());
    }

    #[test]
    fn pidless_match_without_live_session_command_falls_back_to_stale() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let worktree = repo.join(".worktrees/task-065");
        write_session(
            &root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl"),
            &format!(r#"{{"workdir":"{}"}}"#, worktree.display()),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: Vec::new(),
        };

        let report = scan_local_sessions_with(
            &request,
            &probe,
            SystemTime::now() + Duration::from_secs(RECENT_ACTIVITY_WINDOW.as_secs() + 60),
        )
        .expect("scan succeeds");

        assert_eq!(
            report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Stale
        );
        assert!(!report.attributions[0].has_active_marker());
    }

    #[test]
    fn unrelated_sessions_do_not_trigger_resume_command_scan() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl"),
            r#"{"workdir":"/tmp/other-repo","note":"no task match"}"#,
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CountingCommandProbe {
            command_line_calls: std::cell::Cell::new(0),
        };

        let scan =
            scan_local_sessions_with_snapshots(&request, &probe, SystemTime::now()).expect("scan");

        assert!(scan.report.attributions.is_empty());
        assert_eq!(probe.command_line_calls.get(), 0);
    }

    #[test]
    fn mtime_only_recent_match_does_not_mark_active() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let worktree = repo.join(".worktrees/task-065");
        write_session(
            &root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl"),
            &format!(r#"{{"workdir":"{}"}}"#, worktree.display()),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: Vec::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert_eq!(
            report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Recent
        );
        assert!(!report.attributions[0].has_active_marker());
    }

    #[test]
    fn observed_session_file_change_marks_matched_session_running_temporarily() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let session =
            root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl");
        write_session(
            &session,
            &format!(r#"{{"workdir":"{}","note":"065-task.md"}}"#, repo.display()),
        );
        let now = SystemTime::now();
        let mut request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: Vec::new(),
        };

        let first =
            scan_local_sessions_with_snapshots(&request, &probe, now).expect("scan succeeds");
        assert_eq!(
            first.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Recent
        );
        assert!(!first.report.attributions[0].has_active_marker());

        fs::write(
            &session,
            format!(
                r#"{{"workdir":"{}","note":"065-task.md","event":"next"}}"#,
                request.repo_root.display()
            ),
        )
        .expect("update session");
        request.previous_session_files = first.session_files;
        let second =
            scan_local_sessions_with_snapshots(&request, &probe, now + Duration::from_secs(2))
                .expect("scan succeeds");

        assert_eq!(
            second.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Running
        );
        assert!(second.report.attributions[0].has_active_marker());

        request.previous_session_files = second.session_files;
        let third = scan_local_sessions_with_snapshots(
            &request,
            &probe,
            now + Duration::from_secs(2) + OBSERVED_RUNNING_WINDOW - Duration::from_secs(1),
        )
        .expect("scan succeeds");
        assert_eq!(
            third.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Running
        );

        request.previous_session_files = third.session_files;
        let fourth = scan_local_sessions_with_snapshots(
            &request,
            &probe,
            now + Duration::from_secs(2) + OBSERVED_RUNNING_WINDOW + Duration::from_secs(1),
        )
        .expect("scan succeeds");
        assert_eq!(
            fourth.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Recent
        );
    }

    #[test]
    fn previously_unmatched_session_write_marks_running_until_grace_window_expires() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let session =
            root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl");
        write_session(
            &session,
            r#"{"workdir":"/tmp/other-repo","note":"not this workflow"}"#,
        );
        let now = SystemTime::now();
        let mut request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: Vec::new(),
        };

        let first =
            scan_local_sessions_with_snapshots(&request, &probe, now).expect("scan succeeds");
        assert!(first.report.attributions.is_empty());

        fs::write(
            &session,
            format!(
                r#"{{"workdir":"{}","note":"065-task.md"}}"#,
                request.repo_root.display()
            ),
        )
        .expect("update session");
        request.previous_session_files = first.session_files;
        let second =
            scan_local_sessions_with_snapshots(&request, &probe, now + Duration::from_secs(2))
                .expect("scan succeeds");
        assert_eq!(
            second.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Running
        );

        request.previous_session_files = second.session_files;
        let third = scan_local_sessions_with_snapshots(
            &request,
            &probe,
            now + Duration::from_secs(2) + OBSERVED_RUNNING_WINDOW - Duration::from_secs(1),
        )
        .expect("scan succeeds");
        assert_eq!(
            third.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Running
        );

        request.previous_session_files = third.session_files;
        let fourth = scan_local_sessions_with_snapshots(
            &request,
            &probe,
            now + Duration::from_secs(2) + OBSERVED_RUNNING_WINDOW + Duration::from_secs(1),
        )
        .expect("scan succeeds");
        assert_eq!(
            fourth.report.attributions[0].evidence[0].run_state(),
            AgentSessionState::Recent
        );
    }

    #[test]
    fn id_only_evidence_does_not_match_entity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(&root.join("weak.jsonl"), r#"{"pid":9999,"note":"065"}"#);
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions.is_empty());
    }

    #[test]
    fn live_unrelated_workspace_session_with_entity_id_does_not_match_entity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("spacetop");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl"),
            r#"{"pid":4242,"agent_nickname":"Mendel","workdir":"/Users/kent/Dev/InfuseAI/GitHub/dataagentbench","note":"created task 068"}"#,
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("068", None)],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::from([4242]),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions.is_empty());
    }

    #[test]
    fn same_repo_session_with_incidental_entity_id_does_not_match_entity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("spacetop");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("hegel-070.jsonl"),
            &format!(
                r#"{{"agent_nickname":"Hegel","workdir":"{}","body":"verifying task 070; task 068 was mentioned in the task body under {}"}}"#,
                repo.display(),
                workflow.display()
            ),
        );
        let request = SessionScanRequest {
            workflow_dir: workflow.clone(),
            repo_root: repo.clone(),
            entities: vec![entity_with_path(
                "068",
                workflow.join("refine-session-preview-wording.md"),
            )],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions.is_empty());
    }

    #[test]
    fn explicit_task_file_reference_matches_non_worktree_entity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("spacetop");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("task-068.jsonl"),
            r#"{"agent_nickname":"Hegel","body":"review docs/spacetop-dev/refine-session-preview-wording.md"}"#,
        );
        let request = SessionScanRequest {
            workflow_dir: workflow.clone(),
            repo_root: repo.clone(),
            entities: vec![entity_with_path(
                "068",
                workflow.join("refine-session-preview-wording.md"),
            )],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert_eq!(report.attributions[0].entity_id, "068");
        assert_eq!(
            report.attributions[0].evidence[0].confidence,
            AttributionConfidence::Medium
        );
    }

    #[test]
    fn conflicting_dispatch_assignment_blocks_task_file_test_data_match() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("spacetop");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("heisenberg-070-verify.jsonl"),
            r#"{"agent_nickname":"Heisenberg","body":"Read /tmp/spacedock-dispatch/spacedock-ensign-fix-unrelated-session-running-attribution-verify.md. Regression mentions docs/spacetop-dev/refine-session-preview-wording.md as test data."}"#,
        );
        let request = SessionScanRequest {
            workflow_dir: workflow.clone(),
            repo_root: repo.clone(),
            entities: vec![entity_with_path(
                "068",
                workflow.join("refine-session-preview-wording.md"),
            )],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions.is_empty());
    }

    #[test]
    fn folder_entity_index_filename_alone_does_not_match_entity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("spacetop");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        write_session(
            &root.join("unrelated-index.jsonl"),
            r#"{"agent_nickname":"Hegel","body":"inspect index.md in another folder"}"#,
        );
        let request = SessionScanRequest {
            workflow_dir: workflow.clone(),
            repo_root: repo.clone(),
            entities: vec![entity_with_path(
                "071",
                workflow.join("folder-form-task/index.md"),
            )],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = FixtureProbe {
            running: HashSet::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions.is_empty());
    }

    #[test]
    fn extract_pid_ignores_unquoted_substrings() {
        assert_eq!(extract_pid(r#"{"pid":4242}"#), Some(4242));
        assert_eq!(extract_pid(r#"rapid123 mentions 065"#), None);
        assert_eq!(extract_pid(r#"pid: 4242"#), None);
    }

    #[test]
    fn entity_id_match_rejects_uuid_substrings() {
        assert!(!contains_entity_id("uuid dfbf9616-b067-4cd4-8a65", "067"));
        assert!(!contains_entity_id("uuid 09d0dbf9-0672-49ed", "067"));
        assert!(contains_entity_id("task 067 is ready", "067"));
        assert!(contains_entity_id("task-067.md", "067"));
    }

    #[test]
    fn live_session_with_id_only_inside_uuid_does_not_match_entity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp
            .path()
            .join("claude/projects/-Users-kent-Dev-InfuseAI-GitHub-recce");
        let session_id = "2a301c1d-2d0c-4fe0-81d6-55b61507cdc0";
        write_session(
            &root.join(format!("{session_id}.jsonl")),
            r#"{"uuid":"dfbf9616-b067-4cd4-8a65-59b90007284f","cwd":"/Users/kent/Dev/InfuseAI/GitHub/recce"}"#,
        );
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("067", Some(".worktrees/task-067"))],
            roots: SessionRoots {
                codex: Vec::new(),
                claude_code: vec![root],
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CommandProbe {
            running: HashSet::new(),
            command_lines: vec![format!("claude --resume {session_id}")],
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(report.attributions.is_empty());
    }

    #[test]
    fn live_session_key_extracts_agent_specific_uuid() {
        assert_eq!(
            live_session_key(
                AgentKind::Codex,
                Path::new("rollout-2026-06-18T14-26-00-019ed968-6e77-7d71-9386-aae754c6c8be.jsonl"),
                "{}"
            )
            .as_deref(),
            Some("019ed968-6e77-7d71-9386-aae754c6c8be")
        );
        assert_eq!(
            live_session_key(
                AgentKind::ClaudeCode,
                Path::new("session.jsonl"),
                r#"{"sessionId":"f0d61fc8-8903-43c0-b916-4b02c45bf441"}"#
            )
            .as_deref(),
            Some("f0d61fc8-8903-43c0-b916-4b02c45bf441")
        );
        assert_eq!(
            live_session_key(
                AgentKind::ClaudeCode,
                Path::new("f0d61fc8-8903-43c0-b916-4b02c45bf441.jsonl"),
                "{}"
            )
            .as_deref(),
            Some("f0d61fc8-8903-43c0-b916-4b02c45bf441")
        );
        assert_eq!(
            live_session_key(AgentKind::Codex, Path::new("session.jsonl"), "{}"),
            None
        );
    }

    #[test]
    fn live_session_command_matching_requires_exact_resume_argv() {
        let key = "019ed968-6e77-7d71-9386-aae754c6c8be";
        assert!(command_matches_live_session(
            AgentKind::Codex,
            key,
            &format!("codex resume {key}")
        ));
        assert!(command_matches_live_session(
            AgentKind::Codex,
            key,
            &format!("codex --resume={key}")
        ));
        assert!(command_matches_live_session(
            AgentKind::ClaudeCode,
            key,
            &format!("/usr/local/bin/claude --resume {key}")
        ));

        for command in [
            "codex",
            "claude",
            "Codex.app/Contents/MacOS/helper",
            "claude-helper --resume 019ed968-6e77-7d71-9386-aae754c6c8be",
            "codex-helper resume 019ed968-6e77-7d71-9386-aae754c6c8be",
            "codex --workdir /repo/.worktrees/task-065",
            "claude --cwd /repo/.worktrees/task-065",
            "sh -lc 'codex resume 019ed968-6e77-7d71-9386-aae754c6c8be'",
            "codex resume 019ed968-6e77-7d71-9386-aae754c6c8be-extra",
            "claude --resume 019ed968-6e77-7d71-9386-aae754c6c8be-extra",
        ] {
            assert!(
                !command_matches_live_session(AgentKind::Codex, key, command),
                "Codex false positive: {command}"
            );
            assert!(
                !command_matches_live_session(AgentKind::ClaudeCode, key, command),
                "Claude false positive: {command}"
            );
        }
    }

    #[test]
    fn repeated_pid_probe_is_cached_per_scan() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        let workflow = repo.join("docs/spacetop-dev");
        let root = tmp.path().join("codex");
        let worktree = repo.join(".worktrees/task-065");
        for idx in 0..2 {
            write_session(
                &root.join(format!("session-{idx}.jsonl")),
                &format!(r#"{{"pid":4242,"workdir":"{}"}}"#, worktree.display()),
            );
        }
        let request = SessionScanRequest {
            workflow_dir: workflow,
            repo_root: repo,
            entities: vec![entity("065", Some(".worktrees/task-065"))],
            roots: SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let probe = CountingProbe {
            running: HashSet::from([4242]),
            calls: std::cell::Cell::new(0),
        };

        scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert_eq!(probe.calls.get(), 1, "pid should be probed once per scan");
    }
}
