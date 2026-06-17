use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use crate::domain::{
    AgentKind, AgentSessionEvidence, AgentSessionState, AttributionConfidence, Entity,
    EntitySessionAttribution, SessionScanReport,
};
use crate::entity_identity::entity_slug;

const RECENT_ACTIVITY_WINDOW: Duration = Duration::from_secs(30 * 60);
const MAX_SCAN_FILE_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct SessionScanRequest {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub entities: Vec<Entity>,
    pub roots: SessionRoots,
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
}

pub fn scan_local_sessions(
    request: SessionScanRequest,
) -> Result<SessionScanReport, SessionScanError> {
    scan_local_sessions_with(&request, &StdProcessProbe, SystemTime::now())
}

pub fn scan_local_sessions_with<P: ProcessProbe>(
    request: &SessionScanRequest,
    process_probe: &P,
    now: SystemTime,
) -> Result<SessionScanReport, SessionScanError> {
    let mut errors = Vec::new();
    let mut per_entity: HashMap<String, Vec<AgentSessionEvidence>> = HashMap::new();
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
            let activity_time = metadata.modified().ok();
            let pid = extract_pid(&content);
            let run_state = classify_run_state(pid, activity_time, process_probe, now);
            for entity in &request.entities {
                if let Some((confidence, matched_worktree)) =
                    match_entity(entity, &request.workflow_dir, &request.repo_root, &content)
                {
                    let evidence = AgentSessionEvidence {
                        agent: *agent,
                        session_id: session_id(entry.path()),
                        display_name: display_name(*agent, entry.path(), &content),
                        confidence,
                        run_state,
                        latest_activity_unix: activity_time.and_then(system_time_unix),
                        matched_worktree,
                    };
                    per_entity
                        .entry(entity.id.clone())
                        .or_default()
                        .push(evidence);
                }
            }
        }
    }

    let mut attributions: Vec<EntitySessionAttribution> = per_entity
        .into_iter()
        .map(|(entity_id, mut evidence)| {
            evidence.sort_by_key(|evidence| {
                (
                    std::cmp::Reverse(evidence.run_state),
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

    Ok(SessionScanReport {
        workflow_dir: request.workflow_dir.clone(),
        repo_root: request.repo_root.clone(),
        scanned_roots: root_pairs.into_iter().map(|(_, root)| root).collect(),
        attributions,
        errors,
    })
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

fn classify_run_state<P: ProcessProbe>(
    pid: Option<u32>,
    activity_time: Option<SystemTime>,
    process_probe: &P,
    now: SystemTime,
) -> AgentSessionState {
    if pid.is_some_and(|pid| process_probe.is_running(pid)) {
        return AgentSessionState::Running;
    }
    if activity_time.is_some_and(|time| {
        now.duration_since(time)
            .map(|age| age <= RECENT_ACTIVITY_WINDOW)
            .unwrap_or(false)
    }) {
        return AgentSessionState::Recent;
    }
    AgentSessionState::Stale
}

fn match_entity(
    entity: &Entity,
    workflow_dir: &Path,
    repo_root: &Path,
    content: &str,
) -> Option<(AttributionConfidence, Option<PathBuf>)> {
    let worktree = entity
        .worktree
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let worktree_paths = worktree
        .map(|value| candidate_paths(repo_root, value))
        .unwrap_or_default();
    for path in &worktree_paths {
        if content.contains(&path.to_string_lossy().to_string()) {
            return Some((AttributionConfidence::High, Some(path.clone())));
        }
    }
    if entity
        .worktree_source
        .as_ref()
        .is_some_and(|path| content.contains(&path.to_string_lossy().to_string()))
    {
        return Some((AttributionConfidence::High, entity.worktree_source.clone()));
    }

    let slug = entity_slug(&entity.path);
    let has_entity_id = !entity.id.trim().is_empty() && content.contains(&entity.id);
    let has_slug = slug
        .as_deref()
        .is_some_and(|slug| !slug.is_empty() && content.contains(slug));
    let has_workflow = content.contains(&workflow_dir.to_string_lossy().to_string())
        || content.contains(&repo_root.to_string_lossy().to_string());

    match (has_workflow, has_entity_id || has_slug) {
        (true, true) => Some((AttributionConfidence::Medium, None)),
        (false, true) => Some((AttributionConfidence::Low, None)),
        _ => None,
    }
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
    for key in ["\"pid\"", "pid"] {
        if let Some(pid) = extract_number_after_key(content, key) {
            return Some(pid);
        }
    }
    None
}

fn extract_number_after_key(content: &str, key: &str) -> Option<u32> {
    let start = content.find(key)? + key.len();
    let after_key = &content[start..];
    let digits: String = after_key
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().ok()
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
    use std::fs;

    struct FixtureProbe {
        running: HashSet<u32>,
    }

    impl ProcessProbe for FixtureProbe {
        fn is_running(&self, pid: u32) -> bool {
            self.running.contains(&pid)
        }
    }

    fn entity(id: &str, worktree: Option<&str>) -> Entity {
        Entity {
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
        }
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
                r#"{{"pid":4242,"agent_nickname":"Mendel","workdir":"{}","note":"065"}}"#,
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
    fn stale_or_low_confidence_evidence_does_not_mark_active() {
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
        };
        let probe = FixtureProbe {
            running: HashSet::new(),
        };

        let report =
            scan_local_sessions_with(&request, &probe, SystemTime::now()).expect("scan succeeds");

        assert!(!report.attributions[0].has_active_marker());
        assert_eq!(
            report.attributions[0].evidence[0].confidence,
            AttributionConfidence::Low
        );
        assert_ne!(
            report.attributions[0].evidence[0].run_state,
            AgentSessionState::Running
        );
    }
}
