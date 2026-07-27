use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use walkdir::WalkDir;

use crate::domain::{
    ActivityHandler, AgentRuntime, Entity, EntityActivity, EntityActivityAttribution,
    SessionScanReport,
};
use crate::entity_identity::entity_slug;

const MAX_SCAN_FILE_BYTES: u64 = 4_000_000;
const DISPATCH_PREFIX: &str = "/tmp/spacedock-dispatch/spacedock-ensign-";
const STAGES: &[&str] = &["shape", "plan", "implement", "verify", "done", "pr-merge"];

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
    pub session_files: HashMap<PathBuf, SessionFileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEvent {
    pub runtime: AgentRuntime,
    pub session_id: String,
    pub updated_unix: i64,
    pub kind: ActivityEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityEventKind {
    WorkerStarted,
    WorkerStopped,
    FirstOfficerStarted,
    FirstOfficerStopped,
    HumanGateOpened { call_id: String },
    HumanGateResolved { call_id: String },
}

pub fn reduce_activity(events: &[ActivityEvent]) -> EntityActivity {
    let mut ordered: Vec<(usize, &ActivityEvent)> = events.iter().enumerate().collect();
    ordered.sort_by_key(|(index, event)| (event.updated_unix, *index));

    let mut workers = HashMap::new();
    let mut first_officers = HashMap::new();
    let mut gates = HashMap::new();
    let mut latest = None;

    for (_, event) in ordered {
        latest = Some(latest.unwrap_or(i64::MIN).max(event.updated_unix));
        let session_key = (event.runtime, event.session_id.clone());
        match &event.kind {
            ActivityEventKind::WorkerStarted => {
                workers.insert(session_key, event.updated_unix);
            }
            ActivityEventKind::WorkerStopped => {
                workers.remove(&session_key);
            }
            ActivityEventKind::FirstOfficerStarted => {
                first_officers.insert(session_key, event.updated_unix);
            }
            ActivityEventKind::FirstOfficerStopped => {
                first_officers.remove(&session_key);
            }
            ActivityEventKind::HumanGateOpened { call_id } => {
                gates.insert(
                    (event.runtime, event.session_id.clone(), call_id.clone()),
                    event.updated_unix,
                );
            }
            ActivityEventKind::HumanGateResolved { call_id } => {
                gates.remove(&(event.runtime, event.session_id.clone(), call_id.clone()));
            }
        }
    }

    if let Some(((runtime, session_id, _), updated_unix)) = gates
        .into_iter()
        .max_by_key(|((runtime, session, _), at)| (*at, *runtime, session.clone()))
    {
        return EntityActivity::HumanGate {
            runtime,
            session_id,
            updated_unix,
        };
    }
    if let Some(((runtime, session_id), updated_unix)) = workers
        .into_iter()
        .max_by_key(|((runtime, session), at)| (*at, *runtime, session.clone()))
    {
        return EntityActivity::Running {
            handler: ActivityHandler::Worker,
            runtime,
            session_id,
            updated_unix,
        };
    }
    if let Some(((runtime, session_id), updated_unix)) = first_officers
        .into_iter()
        .max_by_key(|((runtime, session), at)| (*at, *runtime, session.clone()))
    {
        return EntityActivity::Running {
            handler: ActivityHandler::FirstOfficer,
            runtime,
            session_id,
            updated_unix,
        };
    }
    EntityActivity::Idle {
        updated_unix: latest,
    }
}

pub fn scan_local_sessions(
    request: SessionScanRequest,
) -> Result<SessionScanReport, SessionScanError> {
    scan_local_sessions_with_snapshots(&request, &StdProcessProbe, SystemTime::now())
        .map(|scan| scan.report)
}

pub fn scan_local_sessions_with_snapshots<P: ProcessProbe>(
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
    let mut errors = Vec::new();
    let mut session_files = HashMap::new();
    let mut parsed_by_runtime: HashMap<AgentRuntime, Vec<ParsedFile>> = HashMap::new();
    let scanned_roots: Vec<PathBuf> = request
        .roots
        .all_roots()
        .map(|(_, root)| root.clone())
        .collect();

    for (runtime, root) in request.roots.all_roots() {
        if !root.exists() {
            continue;
        }
        if let Err(err) = fs::read_dir(root) {
            return Err(SessionScanError {
                message: format!(
                    "{} session root {} is unreadable: {err}",
                    runtime.label(),
                    root.display()
                ),
            });
        }
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|entry| !is_pruned_dir(entry.path()))
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    errors.push(format!("{} scan skipped entry: {err}", runtime.label()));
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
                        runtime.label(),
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
                        runtime.label(),
                        entry.path().display()
                    ));
                    continue;
                }
            };
            session_files.insert(
                entry.path().to_path_buf(),
                SessionFileSnapshot {
                    modified_unix: metadata.modified().ok().and_then(system_time_unix),
                    len: metadata.len(),
                },
            );
            let records = parse_records(entry.path(), &content, &mut errors);
            parsed_by_runtime
                .entry(runtime)
                .or_default()
                .push(ParsedFile {
                    path: entry.path().to_path_buf(),
                    records,
                });
        }
    }

    let fallback_time = system_time_unix(now).unwrap_or_default();
    let mut per_entity: HashMap<String, Vec<ActivityEvent>> = request
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), Vec::new()))
        .collect();
    if let Some(files) = parsed_by_runtime.get(&AgentRuntime::Codex) {
        collect_codex_events(files, &request.entities, fallback_time, &mut per_entity);
    }
    if let Some(files) = parsed_by_runtime.get(&AgentRuntime::ClaudeCode) {
        collect_claude_events(files, &request.entities, fallback_time, &mut per_entity);
    }

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

    Ok(SessionActivityScan {
        report: SessionScanReport {
            workflow_dir: request.workflow_dir.clone(),
            repo_root: request.repo_root.clone(),
            scanned_roots,
            attributions,
            errors,
        },
        session_files,
    })
}

#[derive(Debug)]
struct ParsedFile {
    path: PathBuf,
    records: Vec<Value>,
}

fn parse_records(path: &Path, content: &str, errors: &mut Vec<String>) -> Vec<Value> {
    if path.extension().and_then(OsStr::to_str) == Some("json") {
        return match serde_json::from_str(content) {
            Ok(value) => vec![value],
            Err(err) => {
                errors.push(format!(
                    "malformed session record {}: {err}",
                    path.display()
                ));
                Vec::new()
            }
        };
    }
    content
        .lines()
        .enumerate()
        .filter_map(|(line_number, line)| {
            if line.trim().is_empty() {
                return None;
            }
            match serde_json::from_str(line) {
                Ok(value) => Some(value),
                Err(err) => {
                    errors.push(format!(
                        "malformed session record {}:{}: {err}",
                        path.display(),
                        line_number + 1
                    ));
                    None
                }
            }
        })
        .collect()
}

fn collect_codex_events(
    files: &[ParsedFile],
    entities: &[SessionScanEntity],
    fallback_time: i64,
    per_entity: &mut HashMap<String, Vec<ActivityEvent>>,
) {
    for entity in entities {
        let Some(slug) = entity_slug(&entity.path) else {
            continue;
        };
        let mut matched_children = Vec::new();
        for file in files {
            let session_id = file
                .records
                .iter()
                .find_map(|record| {
                    (record_type(record) == Some("session_meta"))
                        .then(|| string_at(record, &["payload", "id"]))
                        .flatten()
                })
                .unwrap_or_else(|| file_id(&file.path));
            let agent_path = file.records.iter().find_map(|record| {
                string_at(
                    record,
                    &[
                        "payload",
                        "source",
                        "subagent",
                        "thread_spawn",
                        "agent_path",
                    ],
                )
            });
            let child_matches = agent_path
                .as_deref()
                .is_some_and(|path| canonical_codex_name(path, &slug))
                && file.records.iter().any(|record| {
                    codex_assignment_text(record)
                        .is_some_and(|text| contains_dispatch(&text, &slug))
                });
            if child_matches {
                matched_children.push((session_id.clone(), agent_path.unwrap_or_default()));
                collect_codex_worker(
                    &file.records,
                    per_entity.entry(entity.id.clone()).or_default(),
                    &session_id,
                    fallback_time,
                );
            } else {
                collect_codex_first_officer(
                    &file.records,
                    per_entity.entry(entity.id.clone()).or_default(),
                    &session_id,
                    entity,
                    &slug,
                    fallback_time,
                );
            }
        }
        for file in files {
            for record in &file.records {
                if event_type(record) != Some("sub_agent_activity")
                    || string_at(record, &["payload", "kind"]).as_deref() != Some("interrupted")
                {
                    continue;
                }
                let thread_id = string_at(record, &["payload", "agent_thread_id"]);
                let agent_path = string_at(record, &["payload", "agent_path"]);
                if let Some((session_id, _)) = matched_children.iter().find(|(session, path)| {
                    thread_id.as_deref() == Some(session.as_str())
                        && agent_path.as_deref() == Some(path.as_str())
                }) {
                    push_event(
                        per_entity.entry(entity.id.clone()).or_default(),
                        AgentRuntime::Codex,
                        session_id,
                        record_timestamp(record, fallback_time),
                        ActivityEventKind::WorkerStopped,
                    );
                }
            }
        }
    }
}

fn collect_codex_worker(
    records: &[Value],
    events: &mut Vec<ActivityEvent>,
    session_id: &str,
    fallback_time: i64,
) {
    let mut open_turn = None;
    for record in records {
        match event_type(record) {
            Some("task_started") => {
                open_turn = string_at(record, &["payload", "turn_id"]);
                if open_turn.is_none() {
                    continue;
                }
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    record_timestamp(record, fallback_time),
                    ActivityEventKind::WorkerStarted,
                );
            }
            Some("task_complete")
                if open_turn.as_deref()
                    == string_at(record, &["payload", "turn_id"]).as_deref() =>
            {
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    record_timestamp(record, fallback_time),
                    ActivityEventKind::WorkerStopped,
                );
                open_turn = None;
            }
            _ => {}
        }
    }
}

fn collect_codex_first_officer(
    records: &[Value],
    events: &mut Vec<ActivityEvent>,
    session_id: &str,
    entity: &SessionScanEntity,
    slug: &str,
    fallback_time: i64,
) {
    let mut open_turn = None;
    let mut scoped_turns = HashSet::new();
    for record in records {
        if event_type(record) == Some("task_started") {
            open_turn = string_at(record, &["payload", "turn_id"]);
            continue;
        }
        if let Some(call) = codex_call(record) {
            let Some(turn) = open_turn.clone() else {
                continue;
            };
            if call_scopes_entity(&call.name, &call.arguments, entity, slug) {
                scoped_turns.insert(turn.clone());
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    record_timestamp(record, fallback_time),
                    ActivityEventKind::FirstOfficerStarted,
                );
            }
            if scoped_turns.contains(&turn)
                && call.name == "request_user_input"
                && is_gate_question(&call.arguments)
            {
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    record_timestamp(record, fallback_time),
                    ActivityEventKind::HumanGateOpened {
                        call_id: call.call_id,
                    },
                );
            }
        }
        if let Some(call_id) = codex_call_output_id(record) {
            push_event(
                events,
                AgentRuntime::Codex,
                session_id,
                record_timestamp(record, fallback_time),
                ActivityEventKind::HumanGateResolved { call_id },
            );
        }
        if event_type(record) == Some("task_complete") {
            let completed = string_at(record, &["payload", "turn_id"]).unwrap_or_default();
            if scoped_turns.remove(&completed) {
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    record_timestamp(record, fallback_time),
                    ActivityEventKind::FirstOfficerStopped,
                );
            }
            if open_turn.as_deref() == Some(completed.as_str()) {
                open_turn = None;
            }
        }
    }
}

fn collect_claude_events(
    files: &[ParsedFile],
    entities: &[SessionScanEntity],
    fallback_time: i64,
    per_entity: &mut HashMap<String, Vec<ActivityEvent>>,
) {
    let mut teammate_meta: HashMap<String, (String, String)> = HashMap::new();
    for file in files {
        for record in &file.records {
            if string_at(record, &["taskKind"]).as_deref() == Some("in_process_teammate") {
                if let (Some(agent_id), Some(name)) = (
                    string_at(record, &["agentId"]),
                    string_at(record, &["name"]),
                ) {
                    teammate_meta.insert(name, (agent_id, file_id(&file.path)));
                }
            }
        }
    }

    for entity in entities {
        let Some(slug) = entity_slug(&entity.path) else {
            continue;
        };
        let mut dispatched_names = HashSet::new();
        for file in files {
            if file.records.iter().any(is_claude_sidechain) {
                continue;
            }
            let session_id = claude_session_id(file);
            collect_claude_first_officer(
                &file.records,
                per_entity.entry(entity.id.clone()).or_default(),
                &session_id,
                entity,
                &slug,
                fallback_time,
                &mut dispatched_names,
            );
        }

        for name in dispatched_names {
            let Some((expected_agent_id, _)) = teammate_meta.get(&name) else {
                continue;
            };
            for file in files {
                let agent_id = file.records.iter().find_map(|record| {
                    is_claude_sidechain(record)
                        .then(|| string_at(record, &["agentId"]))
                        .flatten()
                });
                if agent_id.as_deref() != Some(expected_agent_id.as_str()) {
                    continue;
                }
                if let Some(start) = file.records.iter().find(|record| {
                    is_claude_sidechain(record)
                        && string_at(record, &["type"]).as_deref() == Some("assistant")
                }) {
                    push_event(
                        per_entity.entry(entity.id.clone()).or_default(),
                        AgentRuntime::ClaudeCode,
                        expected_agent_id,
                        record_timestamp(start, fallback_time),
                        ActivityEventKind::WorkerStarted,
                    );
                }
            }
            for file in files {
                if file.records.iter().any(is_claude_sidechain) {
                    continue;
                }
                if let Some(stop) = file.records.iter().find(|record| {
                    teammate_idle_notification(record).is_some_and(|from| from == name)
                }) {
                    push_event(
                        per_entity.entry(entity.id.clone()).or_default(),
                        AgentRuntime::ClaudeCode,
                        expected_agent_id,
                        record_timestamp(stop, fallback_time),
                        ActivityEventKind::WorkerStopped,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_claude_first_officer(
    records: &[Value],
    events: &mut Vec<ActivityEvent>,
    session_id: &str,
    entity: &SessionScanEntity,
    slug: &str,
    fallback_time: i64,
    dispatched_names: &mut HashSet<String>,
) {
    let mut scoped = false;
    let mut handoff_pending = false;
    for record in records {
        let is_assistant = string_at(record, &["type"]).as_deref() == Some("assistant");
        if handoff_pending && is_assistant {
            scoped = true;
            handoff_pending = false;
            push_event(
                events,
                AgentRuntime::ClaudeCode,
                session_id,
                record_timestamp(record, fallback_time),
                ActivityEventKind::FirstOfficerStarted,
            );
        }
        if let Some(blocks) = record
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)
        {
            for block in blocks {
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    let call_id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if call_scopes_entity(name, &input, entity, slug) {
                        scoped = true;
                        push_event(
                            events,
                            AgentRuntime::ClaudeCode,
                            session_id,
                            record_timestamp(record, fallback_time),
                            ActivityEventKind::FirstOfficerStarted,
                        );
                    }
                    if name == "Agent" && call_scopes_entity(name, &input, entity, slug) {
                        if let Some(worker_name) = input.get("name").and_then(Value::as_str) {
                            dispatched_names.insert(worker_name.to_string());
                        }
                    }
                    if scoped && name == "AskUserQuestion" && is_gate_question(&input) {
                        push_event(
                            events,
                            AgentRuntime::ClaudeCode,
                            session_id,
                            record_timestamp(record, fallback_time),
                            ActivityEventKind::HumanGateOpened { call_id },
                        );
                    }
                }
            }
        }
        if let Some(blocks) = record
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)
        {
            for block in blocks {
                if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                    if let Some(call_id) = block.get("tool_use_id").and_then(Value::as_str) {
                        push_event(
                            events,
                            AgentRuntime::ClaudeCode,
                            session_id,
                            record_timestamp(record, fallback_time),
                            ActivityEventKind::HumanGateResolved {
                                call_id: call_id.to_string(),
                            },
                        );
                    }
                }
            }
        }
        if scoped && string_at(record, &["message", "stop_reason"]).as_deref() == Some("end_turn") {
            push_event(
                events,
                AgentRuntime::ClaudeCode,
                session_id,
                record_timestamp(record, fallback_time),
                ActivityEventKind::FirstOfficerStopped,
            );
            scoped = false;
        }
        if teammate_idle_notification(record).is_some_and(|from| dispatched_names.contains(&from)) {
            // The linked envelope scopes the handoff, but the next observable
            // assistant record is what makes FO work visible.
            scoped = false;
            handoff_pending = true;
        }
    }
}

struct ParsedCall {
    name: String,
    call_id: String,
    arguments: Value,
}

fn codex_call(record: &Value) -> Option<ParsedCall> {
    let payload = record.get("payload")?;
    let payload_type = payload.get("type").and_then(Value::as_str)?;
    if !matches!(payload_type, "function_call" | "custom_tool_call") {
        return None;
    }
    let name = payload.get("name").and_then(Value::as_str)?.to_string();
    let call_id = payload
        .get("call_id")
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let raw = payload
        .get("arguments")
        .or_else(|| payload.get("input"))
        .cloned()
        .unwrap_or(Value::Null);
    let arguments = raw
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(raw);
    Some(ParsedCall {
        name,
        call_id,
        arguments,
    })
}

fn codex_call_output_id(record: &Value) -> Option<String> {
    let payload = record.get("payload")?;
    (payload.get("type").and_then(Value::as_str) == Some("function_call_output"))
        .then(|| {
            payload
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .flatten()
}

fn call_scopes_entity(
    tool_name: &str,
    arguments: &Value,
    entity: &SessionScanEntity,
    slug: &str,
) -> bool {
    if tool_name.ends_with("spawn_agent") || tool_name == "Agent" {
        let expected_name = format!("spacedock-ensign-{slug}-");
        let expected_codex_name = format!("spacedock_ensign_{}_", slug.replace('-', "_"));
        let name = arguments
            .get("task_name")
            .or_else(|| arguments.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let prompt = arguments
            .get("message")
            .or_else(|| arguments.get("prompt"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        return (name.starts_with(&expected_name) || name.starts_with(&expected_codex_name))
            && contains_dispatch(prompt, slug);
    }

    let known_value = match tool_name {
        "Read" | "Edit" | "Write" => arguments.get("file_path").or_else(|| arguments.get("path")),
        "Bash" => arguments.get("command"),
        _ => arguments
            .get("path")
            .or_else(|| arguments.get("cmd"))
            .or_else(|| arguments.get("uri")),
    };
    known_value.and_then(Value::as_str).is_some_and(|value| {
        value == entity.path.to_string_lossy() || contains_dispatch(value, slug)
    })
}

fn is_gate_question(input: &Value) -> bool {
    let Some(questions) = input.get("questions").and_then(Value::as_array) else {
        return false;
    };
    questions.iter().any(|question| {
        let gate_named = ["id", "header"]
            .iter()
            .filter_map(|field| question.get(*field).and_then(Value::as_str))
            .any(|value| value.to_ascii_lowercase().contains("gate"));
        let labels: Vec<String> = question
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| option.get("label").and_then(Value::as_str))
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

fn codex_assignment_text(record: &Value) -> Option<String> {
    let payload = record.get("payload")?;
    if payload.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }
    payload
        .get("content")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|item| {
            item.get("text")
                .or_else(|| item.get("input_text"))
                .and_then(Value::as_str)
        })
        .collect::<Vec<_>>()
        .join("\n")
        .into()
}

fn canonical_codex_name(path: &str, slug: &str) -> bool {
    STAGES.iter().any(|stage| {
        path.rsplit('/').next()
            == Some(format!("spacedock_ensign_{}_{}", slug.replace('-', "_"), stage).as_str())
            || path.rsplit('/').next() == Some(format!("spacedock_ensign_{slug}_{stage}").as_str())
    })
}

fn contains_dispatch(text: &str, slug: &str) -> bool {
    STAGES
        .iter()
        .any(|stage| text.contains(&format!("{DISPATCH_PREFIX}{slug}-{stage}.md")))
}

fn is_claude_sidechain(record: &Value) -> bool {
    record
        .get("isSidechain")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn claude_session_id(file: &ParsedFile) -> String {
    file.records
        .iter()
        .find_map(|record| string_at(record, &["sessionId"]))
        .unwrap_or_else(|| file_id(&file.path))
}

fn teammate_idle_notification(record: &Value) -> Option<String> {
    if string_at(record, &["type"]).as_deref() != Some("user") {
        return None;
    }
    let content = record
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)?;
    let start = content.find("<teammate-message>")? + "<teammate-message>".len();
    let end = content[start..].find("</teammate-message>")? + start;
    let envelope: Value = serde_json::from_str(content[start..end].trim()).ok()?;
    (envelope.get("type").and_then(Value::as_str) == Some("idle_notification")
        && envelope.get("idleReason").and_then(Value::as_str) == Some("available"))
    .then(|| {
        envelope
            .get("from")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
    .flatten()
}

fn push_event(
    events: &mut Vec<ActivityEvent>,
    runtime: AgentRuntime,
    session_id: &str,
    updated_unix: i64,
    kind: ActivityEventKind,
) {
    events.push(ActivityEvent {
        runtime,
        session_id: session_id.to_string(),
        updated_unix,
        kind,
    });
}

fn record_type(record: &Value) -> Option<&str> {
    record.get("type").and_then(Value::as_str)
}

fn event_type(record: &Value) -> Option<&str> {
    (record_type(record) == Some("event_msg"))
        .then(|| {
            record
                .get("payload")
                .and_then(|payload| payload.get("type"))
                .and_then(Value::as_str)
        })
        .flatten()
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn record_timestamp(record: &Value, fallback: i64) -> i64 {
    record
        .get("timestamp")
        .and_then(|timestamp| {
            timestamp
                .as_i64()
                .or_else(|| timestamp.as_str().and_then(parse_rfc3339_unix))
        })
        .unwrap_or(fallback)
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

fn file_id(path: &Path) -> String {
    path.file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("unknown-session")
        .to_string()
}

fn is_pruned_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| matches!(name, ".git" | "node_modules" | "target"))
}

fn is_session_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("jsonl" | "json")
    )
}

fn system_time_unix(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            kind,
        }
    }

    #[test]
    fn reducer_covers_handoff_next_worker_gate_and_precedence() {
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
                "worker-1",
                ActivityEventKind::WorkerStarted,
            ),
            event(
                3,
                AgentRuntime::Codex,
                "worker-1",
                ActivityEventKind::WorkerStopped,
            ),
        ];
        assert_eq!(
            reduce_activity(&events).status_label(),
            "running · FO",
            "worker completion must reveal the still-open FO handoff"
        );

        let mut next_worker = events.clone();
        next_worker.push(event(
            4,
            AgentRuntime::ClaudeCode,
            "worker-2",
            ActivityEventKind::WorkerStarted,
        ));
        assert_eq!(
            reduce_activity(&next_worker).status_label(),
            "running · worker"
        );

        next_worker.push(event(
            5,
            AgentRuntime::Codex,
            "fo",
            ActivityEventKind::HumanGateOpened {
                call_id: "gate-1".to_string(),
            },
        ));
        assert_eq!(reduce_activity(&next_worker).status_label(), "human-gate");
        next_worker.push(event(
            6,
            AgentRuntime::Codex,
            "fo",
            ActivityEventKind::HumanGateResolved {
                call_id: "gate-1".to_string(),
            },
        ));
        assert_eq!(
            reduce_activity(&next_worker).status_label(),
            "running · worker"
        );
    }

    #[test]
    fn reducer_returns_idle_with_terminal_timestamp() {
        let activity = reduce_activity(&[
            event(
                10,
                AgentRuntime::Codex,
                "worker",
                ActivityEventKind::WorkerStarted,
            ),
            event(
                12,
                AgentRuntime::Codex,
                "worker",
                ActivityEventKind::WorkerStopped,
            ),
        ]);
        assert_eq!(
            activity,
            EntityActivity::Idle {
                updated_unix: Some(12)
            }
        );
    }

    #[test]
    fn rfc3339_parser_handles_utc_and_offsets() {
        assert_eq!(parse_rfc3339_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_unix("1970-01-01T08:00:00+08:00"), Some(0));
    }

    fn fixture_root(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/session-activity")
            .join(name)
    }

    fn scan_fixture(runtime: AgentRuntime, fixture: &str) -> SessionScanReport {
        let entity = SessionScanEntity {
            id: "069".to_string(),
            path: PathBuf::from("/repo/docs/state/detect-entity-activity-state.md"),
            worktree: None,
            worktree_source: None,
        };
        let root = fixture_root(fixture);
        let roots = match runtime {
            AgentRuntime::Codex => SessionRoots {
                codex: vec![root],
                claude_code: Vec::new(),
            },
            AgentRuntime::ClaudeCode => SessionRoots {
                codex: Vec::new(),
                claude_code: vec![root],
            },
        };
        scan_local_sessions_with(
            &SessionScanRequest {
                workflow_dir: PathBuf::from("/repo/docs"),
                repo_root: PathBuf::from("/repo"),
                entities: vec![entity],
                roots,
                previous_session_files: HashMap::new(),
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("fixture scan")
    }

    #[test]
    fn codex_worker_fixture_requires_canonical_child_and_exact_assignment() {
        let report = scan_fixture(AgentRuntime::Codex, "codex-worker-open");
        assert_eq!(report.errors, Vec::<String>::new());
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "running · worker"
        );
        assert_eq!(
            report.attributions[0].activity.session_id(),
            Some("codex-worker-redacted")
        );
    }

    #[test]
    fn codex_task_complete_closes_only_the_open_worker_turn() {
        let report = scan_fixture(AgentRuntime::Codex, "codex-worker-complete");
        assert_eq!(report.attributions[0].activity.status_label(), "idle");
        assert!(report.attributions[0].activity.updated_unix().is_some());
    }

    #[test]
    fn codex_gate_fixture_requires_scoped_fo_turn_and_balanced_options() {
        let report = scan_fixture(AgentRuntime::Codex, "codex-fo-gate");
        assert_eq!(report.attributions[0].activity.status_label(), "human-gate");
    }

    #[test]
    fn claude_worker_fixture_correlates_agent_call_meta_and_sidechain() {
        let report = scan_fixture(AgentRuntime::ClaudeCode, "claude-worker-open");
        assert_eq!(report.errors, Vec::<String>::new());
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "running · worker"
        );
        assert_eq!(
            report.attributions[0].activity.session_id(),
            Some("claude-worker-redacted")
        );
    }

    #[test]
    fn claude_idle_notification_closes_the_correlated_worker() {
        let report = scan_fixture(AgentRuntime::ClaudeCode, "claude-worker-idle");
        assert_eq!(report.attributions[0].activity.status_label(), "idle");
        assert!(report.attributions[0].activity.updated_unix().is_some());
    }

    #[test]
    fn claude_gate_and_end_turn_records_drive_exact_fo_transitions() {
        let gate = scan_fixture(AgentRuntime::ClaudeCode, "claude-fo-gate");
        assert_eq!(gate.attributions[0].activity.status_label(), "human-gate");

        let complete = scan_fixture(AgentRuntime::ClaudeCode, "claude-fo-complete");
        assert_eq!(complete.attributions[0].activity.status_label(), "idle");
        assert!(complete.attributions[0].activity.updated_unix().is_some());
    }

    #[test]
    fn ordinary_path_mentions_do_not_create_activity() {
        let temp = tempfile::tempdir().expect("temp");
        fs::write(
            temp.path().join("mention.jsonl"),
            r#"{"timestamp":1,"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"/repo/docs/state/detect-entity-activity-state.md approve"}]}}"#,
        )
        .expect("fixture");
        let entity = SessionScanEntity {
            id: "069".to_string(),
            path: PathBuf::from("/repo/docs/state/detect-entity-activity-state.md"),
            worktree: None,
            worktree_source: None,
        };
        let report = scan_local_sessions_with(
            &SessionScanRequest {
                workflow_dir: PathBuf::from("/repo/docs"),
                repo_root: PathBuf::from("/repo"),
                entities: vec![entity],
                roots: SessionRoots {
                    codex: vec![temp.path().to_path_buf()],
                    claude_code: Vec::new(),
                },
                previous_session_files: HashMap::new(),
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("scan");
        assert_eq!(report.attributions[0].activity, EntityActivity::default());
    }

    #[test]
    fn malformed_lines_are_reported_without_discarding_valid_transitions() {
        let temp = tempfile::tempdir().expect("temp");
        let fixture = fs::read_to_string(fixture_root("codex-worker-open/rollout.jsonl"))
            .expect("source fixture");
        fs::write(
            temp.path().join("rollout.jsonl"),
            format!("{fixture}\nnot-json\n"),
        )
        .expect("fixture");
        let entity = SessionScanEntity {
            id: "069".to_string(),
            path: PathBuf::from("/repo/docs/state/detect-entity-activity-state.md"),
            worktree: None,
            worktree_source: None,
        };
        let report = scan_local_sessions_with(
            &SessionScanRequest {
                workflow_dir: PathBuf::from("/repo/docs"),
                repo_root: PathBuf::from("/repo"),
                entities: vec![entity],
                roots: SessionRoots {
                    codex: vec![temp.path().to_path_buf()],
                    claude_code: Vec::new(),
                },
                previous_session_files: HashMap::new(),
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("scan");
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "running · worker"
        );
        assert_eq!(report.errors.len(), 1);
    }

    #[test]
    fn unreadable_root_is_a_scan_failure_instead_of_false_idle() {
        let temp = tempfile::tempdir().expect("temp");
        let not_a_directory = temp.path().join("session-root");
        fs::write(&not_a_directory, "not a directory").expect("fixture");
        let result = scan_local_sessions_with(
            &SessionScanRequest {
                workflow_dir: PathBuf::from("/repo/docs"),
                repo_root: PathBuf::from("/repo"),
                entities: vec![SessionScanEntity {
                    id: "069".to_string(),
                    path: PathBuf::from("/repo/docs/state/detect-entity-activity-state.md"),
                    worktree: None,
                    worktree_source: None,
                }],
                roots: SessionRoots {
                    codex: vec![not_a_directory],
                    claude_code: Vec::new(),
                },
                previous_session_files: HashMap::new(),
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        );
        assert!(
            result.is_err(),
            "root IO failure must preserve prior app state"
        );
    }
}
