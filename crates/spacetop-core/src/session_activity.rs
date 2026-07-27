use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::domain::{
    ActivityHandler, AgentRuntime, Entity, EntityActivity, EntityActivityAttribution,
    SessionScanReport,
};
use crate::entity_identity::entity_slug;

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
            let records = match parse_session_file(entry.path(), &mut errors) {
                Ok(records) => records,
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

fn parse_session_file(path: &Path, errors: &mut Vec<String>) -> Result<Vec<Value>, std::io::Error> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    if path.extension().and_then(OsStr::to_str) == Some("json") {
        return Ok(match serde_json::from_reader(reader) {
            Ok(value) => project_record(value).into_iter().collect(),
            Err(err) => {
                errors.push(format!(
                    "malformed session record {}: {err}",
                    path.display()
                ));
                Vec::new()
            }
        });
    }
    Ok(reader
        .lines()
        .enumerate()
        .filter_map(|(line_number, line)| match line {
            Ok(line) => {
                if line.trim().is_empty() {
                    return None;
                }
                match serde_json::from_str(&line) {
                    Ok(value) => project_record(value),
                    Err(err) => {
                        errors.push(format!(
                            "malformed session record {}:{}: {err}",
                            path.display(),
                            line_number + 1
                        ));
                        None
                    }
                }
            }
            Err(err) => {
                errors.push(format!(
                    "session record read failed {}:{}: {err}",
                    path.display(),
                    line_number + 1
                ));
                None
            }
        })
        .collect())
}

fn project_record(record: Value) -> Option<Value> {
    if record.get("taskKind").is_some() {
        return Some(project_claude_meta(&record));
    }

    let record_type = record.get("type").and_then(Value::as_str)?;
    let timestamp = record.get("timestamp").cloned().unwrap_or(Value::Null);
    match record_type {
        "session_meta" => Some(json!({
            "type": record_type,
            "timestamp": timestamp,
            "payload": {
                "id": record.pointer("/payload/id").cloned().unwrap_or(Value::Null),
                "source": {
                    "subagent": {
                        "thread_spawn": {
                            "agent_path": record.pointer("/payload/source/subagent/thread_spawn/agent_path").cloned().unwrap_or(Value::Null),
                            "parent_thread_id": record.pointer("/payload/source/subagent/thread_spawn/parent_thread_id").cloned().unwrap_or(Value::Null),
                        }
                    }
                }
            }
        })),
        "event_msg" => Some(json!({
            "type": record_type,
            "timestamp": timestamp,
            "payload": {
                "type": record.pointer("/payload/type").cloned().unwrap_or(Value::Null),
                "turn_id": record.pointer("/payload/turn_id").cloned().unwrap_or(Value::Null),
                "kind": record.pointer("/payload/kind").cloned().unwrap_or(Value::Null),
                "agent_thread_id": record.pointer("/payload/agent_thread_id").cloned().unwrap_or(Value::Null),
                "agent_path": record.pointer("/payload/agent_path").cloned().unwrap_or(Value::Null),
            }
        })),
        "response_item" => project_codex_response_item(&record, timestamp),
        "assistant" | "user" => project_claude_record(&record, timestamp),
        _ => None,
    }
}

fn project_claude_meta(record: &Value) -> Value {
    json!({
        "taskKind": record.get("taskKind").cloned().unwrap_or(Value::Null),
        "name": record.get("name").cloned().unwrap_or(Value::Null),
        "agentId": record.get("agentId").cloned().unwrap_or(Value::Null),
        "parentSessionId": first_value(record, &["parentSessionId", "parentSessionID", "parent_session_id"]),
        "parentToolUseId": first_value(record, &["parentToolUseId", "parentToolUseID", "parent_tool_use_id"]),
    })
}

fn project_codex_response_item(record: &Value, timestamp: Value) -> Option<Value> {
    let payload = record.get("payload")?;
    let payload_type = payload.get("type").and_then(Value::as_str)?;
    match payload_type {
        "message" if payload.get("role").and_then(Value::as_str) == Some("user") => {
            let dispatches: Vec<Value> = payload
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("input_text"))
                        .and_then(Value::as_str)
                })
                .flat_map(dispatch_markers)
                .map(|text| json!({ "type": "input_text", "text": text }))
                .collect();
            (!dispatches.is_empty()).then(|| {
                json!({
                    "type": "response_item",
                    "timestamp": timestamp,
                    "payload": {
                        "type": "message",
                        "role": "user",
                        "content": dispatches,
                    }
                })
            })
        }
        "function_call" | "custom_tool_call" => {
            let name = payload.get("name").and_then(Value::as_str)?;
            let raw = payload
                .get("arguments")
                .or_else(|| payload.get("input"))
                .cloned()
                .unwrap_or(Value::Null);
            Some(json!({
                "type": "response_item",
                "timestamp": timestamp,
                "payload": {
                    "type": payload_type,
                    "name": name,
                    "call_id": payload.get("call_id").or_else(|| payload.get("id")).cloned().unwrap_or(Value::Null),
                    "arguments": project_tool_input(name, raw),
                }
            }))
        }
        "function_call_output" => Some(json!({
            "type": "response_item",
            "timestamp": timestamp,
            "payload": {
                "type": payload_type,
                "call_id": payload.get("call_id").cloned().unwrap_or(Value::Null),
            }
        })),
        _ => None,
    }
}

fn project_claude_record(record: &Value, timestamp: Value) -> Option<Value> {
    let record_type = record.get("type").and_then(Value::as_str)?;
    let projected_content: Vec<Value> = record
        .pointer("/message/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|block| match block.get("type").and_then(Value::as_str) {
            Some("tool_use") => {
                let name = block.get("name").and_then(Value::as_str)?;
                Some(json!({
                    "type": "tool_use",
                    "id": block.get("id").cloned().unwrap_or(Value::Null),
                    "name": name,
                    "input": project_tool_input(name, block.get("input").cloned().unwrap_or(Value::Null)),
                }))
            }
            Some("tool_result") => Some(json!({
                "type": "tool_result",
                "tool_use_id": block.get("tool_use_id").cloned().unwrap_or(Value::Null),
            })),
            _ => None,
        })
        .collect();
    let teammate_content = record
        .pointer("/message/content")
        .and_then(Value::as_str)
        .and_then(project_teammate_envelope);

    Some(json!({
        "type": record_type,
        "timestamp": timestamp,
        "sessionId": record.get("sessionId").cloned().unwrap_or(Value::Null),
        "isSidechain": record.get("isSidechain").cloned().unwrap_or(Value::Bool(false)),
        "agentId": record.get("agentId").cloned().unwrap_or(Value::Null),
        "parentSessionId": first_value(record, &["parentSessionId", "parentSessionID", "parent_session_id"]),
        "message": {
            "content": teammate_content.unwrap_or(Value::Array(projected_content)),
            "stop_reason": record.pointer("/message/stop_reason").cloned().unwrap_or(Value::Null),
        }
    }))
}

fn project_tool_input(name: &str, raw: Value) -> Value {
    let parsed = raw
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(raw);
    if name == "exec" || name == "Bash" {
        return command_text(&parsed)
            .map(|command| json!({ "cmd": command }))
            .unwrap_or(Value::Null);
    }
    if name.ends_with("spawn_agent") || name == "Agent" {
        return json!({
            "task_name": parsed.get("task_name").or_else(|| parsed.get("name")).cloned().unwrap_or(Value::Null),
            "message": parsed
                .get("message")
                .or_else(|| parsed.get("prompt"))
                .and_then(Value::as_str)
                .map(dispatch_markers)
                .unwrap_or_default()
                .join("\n"),
        });
    }
    if matches!(name, "request_user_input" | "AskUserQuestion") {
        return json!({ "questions": project_questions(parsed.get("questions")) });
    }
    json!({
        "file_path": parsed.get("file_path").cloned().unwrap_or(Value::Null),
        "path": parsed.get("path").cloned().unwrap_or(Value::Null),
        "cmd": parsed.get("cmd").cloned().unwrap_or(Value::Null),
        "command": parsed.get("command").cloned().unwrap_or(Value::Null),
        "uri": parsed.get("uri").cloned().unwrap_or(Value::Null),
    })
}

fn command_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    ["cmd", "command", "input"]
        .iter()
        .find_map(|key| value.get(*key).and_then(command_text))
}

fn project_questions(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|question| {
            let options: Vec<Value> = question
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|option| option.get("label").and_then(Value::as_str))
                .map(|label| json!({ "label": label }))
                .collect();
            json!({
                "id": question.get("id").cloned().unwrap_or(Value::Null),
                "header": question.get("header").cloned().unwrap_or(Value::Null),
                "options": options,
            })
        })
        .collect()
}

fn project_teammate_envelope(content: &str) -> Option<Value> {
    let start = content.find("<teammate-message>")? + "<teammate-message>".len();
    let end = content[start..].find("</teammate-message>")? + start;
    let envelope: Value = serde_json::from_str(content[start..end].trim()).ok()?;
    let projected = json!({
        "type": envelope.get("type").cloned().unwrap_or(Value::Null),
        "from": envelope.get("from").cloned().unwrap_or(Value::Null),
        "idleReason": envelope.get("idleReason").cloned().unwrap_or(Value::Null),
    });
    Some(Value::String(format!(
        "<teammate-message>{projected}</teammate-message>"
    )))
}

fn dispatch_markers(text: &str) -> Vec<String> {
    let mut markers = Vec::new();
    let mut remainder = text;
    while let Some(start) = remainder.find(DISPATCH_PREFIX) {
        let candidate = &remainder[start..];
        let Some(end) = candidate.find(".md") else {
            break;
        };
        let marker = &candidate[..end + 3];
        let stem = &candidate[DISPATCH_PREFIX.len()..end];
        if marker.len() <= 512
            && stem
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
            && STAGES
                .iter()
                .any(|stage| stem.ends_with(&format!("-{stage}")))
        {
            markers.push(marker.to_string());
        }
        remainder = &candidate[end + 3..];
    }
    markers
}

fn first_value(value: &Value, keys: &[&str]) -> Value {
    keys.iter()
        .find_map(|key| value.get(*key))
        .cloned()
        .unwrap_or(Value::Null)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaudeDispatch {
    parent_session_id: String,
    call_id: String,
    worker_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaudeTeammateMeta {
    parent_session_id: String,
    parent_call_id: Option<String>,
    worker_name: String,
    agent_id: String,
}

fn collect_claude_events(
    files: &[ParsedFile],
    entities: &[SessionScanEntity],
    fallback_time: i64,
    per_entity: &mut HashMap<String, Vec<ActivityEvent>>,
) {
    let teammate_meta: Vec<ClaudeTeammateMeta> =
        files.iter().filter_map(claude_teammate_meta).collect();

    for entity in entities {
        let Some(slug) = entity_slug(&entity.path) else {
            continue;
        };
        let mut dispatches = Vec::new();
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
                &mut dispatches,
            );
        }

        for dispatch in &dispatches {
            let matching_meta: Vec<_> = teammate_meta
                .iter()
                .filter(|meta| {
                    meta.parent_session_id == dispatch.parent_session_id
                        && meta.worker_name == dispatch.worker_name
                        && meta
                            .parent_call_id
                            .as_deref()
                            .is_none_or(|call_id| call_id == dispatch.call_id)
                })
                .collect();
            let same_name_dispatches = dispatches
                .iter()
                .filter(|candidate| {
                    candidate.parent_session_id == dispatch.parent_session_id
                        && candidate.worker_name == dispatch.worker_name
                })
                .count();
            if matching_meta.len() != 1
                || (matching_meta[0].parent_call_id.is_none() && same_name_dispatches != 1)
            {
                continue;
            }
            let meta = matching_meta[0];
            for file in files {
                if claude_parent_session_from_path(&file.path).as_deref()
                    != Some(dispatch.parent_session_id.as_str())
                {
                    continue;
                }
                let agent_id = file.records.iter().find_map(|record| {
                    is_claude_sidechain(record)
                        .then(|| string_at(record, &["agentId"]))
                        .flatten()
                });
                if agent_id.as_deref() != Some(meta.agent_id.as_str()) {
                    continue;
                }
                if let Some(start) = file.records.iter().find(|record| {
                    is_claude_sidechain(record)
                        && string_at(record, &["type"]).as_deref() == Some("assistant")
                }) {
                    push_event(
                        per_entity.entry(entity.id.clone()).or_default(),
                        AgentRuntime::ClaudeCode,
                        &meta.agent_id,
                        record_timestamp(start, fallback_time),
                        ActivityEventKind::WorkerStarted,
                    );
                }
            }
            if same_name_dispatches == 1 {
                for file in files {
                    if file.records.iter().any(is_claude_sidechain) {
                        continue;
                    }
                    if claude_session_id(file) != dispatch.parent_session_id {
                        continue;
                    }
                    if let Some(stop) = file.records.iter().find(|record| {
                        teammate_idle_notification(record)
                            .is_some_and(|from| from == dispatch.worker_name)
                    }) {
                        push_event(
                            per_entity.entry(entity.id.clone()).or_default(),
                            AgentRuntime::ClaudeCode,
                            &meta.agent_id,
                            record_timestamp(stop, fallback_time),
                            ActivityEventKind::WorkerStopped,
                        );
                    }
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
    dispatches: &mut Vec<ClaudeDispatch>,
) {
    let mut scoped = false;
    let mut handoff_pending = false;
    let mut dispatched_names = HashSet::new();
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
                        if let Some(worker_name) = input
                            .get("task_name")
                            .or_else(|| input.get("name"))
                            .and_then(Value::as_str)
                        {
                            dispatched_names.insert(worker_name.to_string());
                            dispatches.push(ClaudeDispatch {
                                parent_session_id: session_id.to_string(),
                                call_id: call_id.clone(),
                                worker_name: worker_name.to_string(),
                            });
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

fn claude_teammate_meta(file: &ParsedFile) -> Option<ClaudeTeammateMeta> {
    let record = file.records.iter().find(|record| {
        string_at(record, &["taskKind"]).as_deref() == Some("in_process_teammate")
    })?;
    let meta = ClaudeTeammateMeta {
        parent_session_id: string_at(record, &["parentSessionId"])
            .or_else(|| claude_parent_session_from_path(&file.path))?,
        parent_call_id: string_at(record, &["parentToolUseId"]),
        worker_name: string_at(record, &["name"])?,
        agent_id: string_at(record, &["agentId"])?,
    };
    claude_parent_session_from_path(&file.path)
        .as_deref()
        .is_some_and(|parent| parent == meta.parent_session_id)
        .then_some(meta)
}

fn claude_parent_session_from_path(path: &Path) -> Option<String> {
    let subagents = path
        .ancestors()
        .find(|ancestor| ancestor.file_name().and_then(OsStr::to_str) == Some("subagents"))?;
    subagents
        .parent()?
        .file_name()
        .and_then(OsStr::to_str)
        .map(str::to_string)
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
        let name = first_string(arguments, &["task_name", "name"]).unwrap_or_default();
        let prompt = first_string(arguments, &["message", "prompt"]).unwrap_or_default();
        return (name.starts_with(&expected_name) || name.starts_with(&expected_codex_name))
            && contains_dispatch(prompt, slug);
    }

    if matches!(tool_name, "exec" | "Bash") {
        return command_text(arguments).is_some_and(|command| {
            command_contains_exact_path(&command, &entity.path) || contains_dispatch(&command, slug)
        });
    }

    let keys: &[&str] = match tool_name {
        "Read" | "Edit" | "Write" => &["file_path", "path"],
        _ => &["path", "uri"],
    };
    first_string(arguments, keys).is_some_and(|value| {
        value == entity.path.to_string_lossy() || contains_dispatch(value, slug)
    })
}

fn first_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
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
    fn codex_exec_custom_call_scopes_raw_and_nested_structured_inputs() {
        for fixture in ["codex-fo-exec", "codex-fo-exec-nested"] {
            let report = scan_fixture(AgentRuntime::Codex, fixture);
            assert_eq!(
                report.attributions[0].activity.status_label(),
                "running · FO",
                "fixture {fixture} must recognize exact entity scope"
            );
        }
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
    fn claude_same_name_activity_stays_linked_to_exact_parent_and_call() {
        let report = scan_fixture(AgentRuntime::ClaudeCode, "claude-two-parent-same-name");
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "running · worker"
        );
        assert_eq!(
            report.attributions[0].activity.session_id(),
            Some("worker-a"),
            "parent B metadata and idle notification must not start or stop parent A's worker"
        );
    }

    #[test]
    fn claude_ambiguous_same_parent_calls_without_call_metadata_fail_closed() {
        let temp = tempfile::tempdir().expect("temp");
        let subagents = temp.path().join("parent/subagents");
        fs::create_dir_all(&subagents).expect("subagents");
        let dispatch = "Read /tmp/spacedock-dispatch/spacedock-ensign-detect-entity-activity-state-implement.md and treat its content as your assignment.";
        fs::write(
            temp.path().join("parent.jsonl"),
            format!(
                r#"{{"timestamp":1,"type":"assistant","sessionId":"parent","isSidechain":false,"message":{{"content":[{{"type":"tool_use","id":"call-a","name":"Agent","input":{{"name":"spacedock-ensign-detect-entity-activity-state-implement","prompt":"{dispatch}"}}}},{{"type":"tool_use","id":"call-b","name":"Agent","input":{{"name":"spacedock-ensign-detect-entity-activity-state-implement","prompt":"{dispatch}"}}}}],"stop_reason":"end_turn"}}}}"#
            ),
        )
        .expect("parent");
        fs::write(
            subagents.join("worker.meta.json"),
            r#"{"taskKind":"in_process_teammate","name":"spacedock-ensign-detect-entity-activity-state-implement","agentId":"worker"}"#,
        )
        .expect("meta");
        fs::write(
            subagents.join("worker.jsonl"),
            r#"{"timestamp":2,"type":"assistant","sessionId":"child","isSidechain":true,"agentId":"worker","message":{"content":[{"type":"text","text":"accepted"}],"stop_reason":null}}"#,
        )
        .expect("child");

        let report = scan_local_sessions_with(
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
                    codex: Vec::new(),
                    claude_code: vec![temp.path().to_path_buf()],
                },
                previous_session_files: HashMap::new(),
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("scan");
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "idle",
            "metadata without a call id must not choose between same-parent duplicate names"
        );
    }

    #[test]
    fn claude_ambiguous_same_parent_idle_name_does_not_stop_exact_call() {
        let temp = tempfile::tempdir().expect("temp");
        let subagents = temp.path().join("parent/subagents");
        fs::create_dir_all(&subagents).expect("subagents");
        let dispatch = "Read /tmp/spacedock-dispatch/spacedock-ensign-detect-entity-activity-state-implement.md and treat its content as your assignment.";
        fs::write(
            temp.path().join("parent.jsonl"),
            format!(
                r#"{{"timestamp":1,"type":"assistant","sessionId":"parent","isSidechain":false,"message":{{"content":[{{"type":"tool_use","id":"call-a","name":"Agent","input":{{"name":"spacedock-ensign-detect-entity-activity-state-implement","prompt":"{dispatch}"}}}},{{"type":"tool_use","id":"call-b","name":"Agent","input":{{"name":"spacedock-ensign-detect-entity-activity-state-implement","prompt":"{dispatch}"}}}}],"stop_reason":"end_turn"}}}}
{{"timestamp":3,"type":"user","sessionId":"parent","isSidechain":false,"message":{{"content":"<teammate-message>{{\"type\":\"idle_notification\",\"from\":\"spacedock-ensign-detect-entity-activity-state-implement\",\"idleReason\":\"available\"}}</teammate-message>"}}}}"#
            ),
        )
        .expect("parent");
        fs::write(
            subagents.join("worker.meta.json"),
            r#"{"taskKind":"in_process_teammate","name":"spacedock-ensign-detect-entity-activity-state-implement","agentId":"worker","parentSessionId":"parent","parentToolUseId":"call-a"}"#,
        )
        .expect("meta");
        fs::write(
            subagents.join("worker.jsonl"),
            r#"{"timestamp":2,"type":"assistant","sessionId":"child","isSidechain":true,"agentId":"worker","message":{"content":[{"type":"text","text":"accepted"}],"stop_reason":null}}"#,
        )
        .expect("child");

        let report = scan_local_sessions_with(
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
                    codex: Vec::new(),
                    claude_code: vec![temp.path().to_path_buf()],
                },
                previous_session_files: HashMap::new(),
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("scan");
        assert_eq!(
            report.attributions[0].activity.status_label(),
            "running · worker",
            "name-only idle evidence must not choose between same-parent duplicate dispatches"
        );
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
    fn large_append_truncation_and_deletion_never_create_false_idle() {
        use std::io::Write;

        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("rollout.jsonl");
        let fixture = fs::read_to_string(fixture_root("codex-worker-open/rollout.jsonl"))
            .expect("source fixture");
        let mut file = fs::File::create(&path).expect("large fixture");
        for _ in 0..90_000 {
            writeln!(file, r#"{{"type":"noise","padding":"{}"}}"#, "x".repeat(32)).expect("noise");
        }
        file.write_all(fixture.as_bytes()).expect("worker records");
        drop(file);
        assert!(
            fs::metadata(&path).expect("metadata").len() > 4_000_000,
            "regression fixture must exceed the removed cutoff"
        );

        let base_request = SessionScanRequest {
            workflow_dir: PathBuf::from("/repo/docs"),
            repo_root: PathBuf::from("/repo"),
            entities: vec![SessionScanEntity {
                id: "069".to_string(),
                path: PathBuf::from("/repo/docs/state/detect-entity-activity-state.md"),
                worktree: None,
                worktree_source: None,
            }],
            roots: SessionRoots {
                codex: vec![temp.path().to_path_buf()],
                claude_code: Vec::new(),
            },
            previous_session_files: HashMap::new(),
        };
        let first = scan_local_sessions_with_snapshots(&base_request, &StdProcessProbe, UNIX_EPOCH)
            .expect("large scan");
        assert_eq!(
            first.report.attributions[0].activity.status_label(),
            "running · worker"
        );

        let mut append = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append");
        writeln!(
            append,
            r#"{{"timestamp":"2026-07-27T10:00:03Z","type":"event_msg","payload":{{"type":"task_complete","turn_id":"worker-turn-redacted"}}}}"#
        )
        .expect("terminal event");
        let appended = scan_local_sessions_with_snapshots(
            &SessionScanRequest {
                previous_session_files: first.session_files,
                ..base_request.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("append scan");
        assert_eq!(
            appended.report.attributions[0].activity.status_label(),
            "idle"
        );

        fs::write(&path, &fixture).expect("truncate to open worker");
        let truncated = scan_local_sessions_with_snapshots(
            &SessionScanRequest {
                previous_session_files: appended.session_files,
                ..base_request.clone()
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("truncation scan");
        assert_eq!(
            truncated.report.attributions[0].activity.status_label(),
            "running · worker"
        );

        fs::remove_file(&path).expect("delete fixture");
        let deleted = scan_local_sessions_with_snapshots(
            &SessionScanRequest {
                previous_session_files: truncated.session_files,
                ..base_request
            },
            &StdProcessProbe,
            UNIX_EPOCH,
        )
        .expect("deletion scan");
        assert_eq!(
            deleted.report.attributions[0].activity.status_label(),
            "idle"
        );
        assert!(deleted.session_files.is_empty());
    }

    #[test]
    fn record_projection_drops_transcript_text() {
        let projected = project_record(json!({
            "timestamp": 1,
            "type": "assistant",
            "sessionId": "session",
            "isSidechain": false,
            "message": {
                "content": [
                    {"type": "text", "text": "private transcript"},
                    {"type": "tool_use", "id": "call", "name": "Read", "input": {"file_path": "/repo/entity.md"}}
                ],
                "stop_reason": null
            }
        }))
        .expect("projection");
        assert!(!projected.to_string().contains("private transcript"));
        assert!(projected.to_string().contains("/repo/entity.md"));
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
