use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{parse_rfc3339_timestamp, EvidenceTimestamp, STAGES};

const DISPATCH_DIR: &str = "/tmp/spacedock-dispatch/";
const DISPATCH_BASENAME_PREFIX: &str = "spacedock-ensign-";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EvidenceOrder {
    pub(crate) timestamp: Option<EvidenceTimestamp>,
    pub(crate) source: PathBuf,
    pub(crate) byte_offset: u64,
    pub(crate) kind_rank: u8,
}

impl EvidenceOrder {
    pub(crate) fn effective_timestamp(&self, fallback_time: i64) -> EvidenceTimestamp {
        self.timestamp
            .unwrap_or_else(|| EvidenceTimestamp::whole_seconds(fallback_time))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProjectedRecord {
    pub(crate) order: EvidenceOrder,
    pub(crate) kind: ProjectedRecordKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProjectedRecordKind {
    CodexSession {
        session_id: String,
        parent_thread_id: Option<String>,
        agent_path: Option<String>,
        cwd: Option<PathBuf>,
    },
    CodexEvent {
        event_type: String,
        turn_id: Option<String>,
        kind: Option<String>,
        agent_thread_id: Option<String>,
        agent_path: Option<String>,
    },
    CodexAssignment {
        dispatches: Vec<String>,
    },
    CodexToolCall {
        name: String,
        call_id: String,
        input: ProjectedToolInput,
    },
    CodexToolResult {
        call_id: String,
    },
    ClaudeMeta {
        worker_name: String,
        agent_id: Option<String>,
        parent_session_id: Option<String>,
        parent_call_id: Option<String>,
    },
    ClaudeMessage {
        record_type: String,
        session_id: Option<String>,
        is_sidechain: bool,
        agent_id: Option<String>,
        parent_session_id: Option<String>,
        cwd: Option<PathBuf>,
        blocks: Vec<ClaudeBlock>,
        stop_reason: Option<String>,
        teammate: Option<TeammateEnvelope>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProjectedToolInput {
    pub(crate) task_name: Option<String>,
    pub(crate) dispatches: Vec<String>,
    pub(crate) commands: Vec<String>,
    pub(crate) questions: Vec<ProjectedQuestion>,
    pub(crate) file_path: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProjectedQuestion {
    pub(crate) id: Option<String>,
    pub(crate) header: Option<String>,
    pub(crate) labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ClaudeBlock {
    ToolUse {
        id: String,
        name: String,
        input: ProjectedToolInput,
    },
    ToolResult {
        tool_use_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TeammateEnvelope {
    pub(crate) envelope_type: Option<String>,
    pub(crate) from: Option<String>,
    pub(crate) idle_reason: Option<String>,
}

pub(crate) fn project_record(
    record: Value,
    source: &Path,
    byte_offset: u64,
) -> Option<ProjectedRecord> {
    let timestamp = record.get("timestamp").and_then(|timestamp| {
        timestamp
            .as_i64()
            .map(EvidenceTimestamp::whole_seconds)
            .or_else(|| timestamp.as_str().and_then(parse_rfc3339_timestamp))
    });
    let kind = if record.get("taskKind").is_some() {
        project_claude_meta(&record)?
    } else {
        match record.get("type").and_then(Value::as_str)? {
            "session_meta" => project_codex_session(&record)?,
            "event_msg" => project_codex_event(&record)?,
            "response_item" => project_codex_response_item(&record)?,
            "assistant" | "user" => project_claude_message(&record)?,
            _ => return None,
        }
    };
    Some(ProjectedRecord {
        order: EvidenceOrder {
            timestamp,
            source: source.to_path_buf(),
            byte_offset,
            kind_rank: kind.rank(),
        },
        kind,
    })
}

impl ProjectedRecordKind {
    fn rank(&self) -> u8 {
        match self {
            Self::CodexSession { .. } => 0,
            Self::ClaudeMeta { .. } => 1,
            Self::CodexAssignment { .. } => 2,
            Self::CodexEvent { .. } => 3,
            Self::CodexToolCall { .. } => 4,
            Self::CodexToolResult { .. } => 5,
            Self::ClaudeMessage { .. } => 6,
        }
    }
}

fn project_codex_session(record: &Value) -> Option<ProjectedRecordKind> {
    Some(ProjectedRecordKind::CodexSession {
        session_id: record.pointer("/payload/id")?.as_str()?.to_string(),
        parent_thread_id: string_at(
            record,
            &[
                "payload",
                "source",
                "subagent",
                "thread_spawn",
                "parent_thread_id",
            ],
        ),
        agent_path: string_at(
            record,
            &[
                "payload",
                "source",
                "subagent",
                "thread_spawn",
                "agent_path",
            ],
        ),
        cwd: path_at(record, &["payload", "cwd"]),
    })
}

fn project_codex_event(record: &Value) -> Option<ProjectedRecordKind> {
    Some(ProjectedRecordKind::CodexEvent {
        event_type: string_at(record, &["payload", "type"])?,
        turn_id: string_at(record, &["payload", "turn_id"]),
        kind: string_at(record, &["payload", "kind"]),
        agent_thread_id: string_at(record, &["payload", "agent_thread_id"]),
        agent_path: string_at(record, &["payload", "agent_path"]),
    })
}

fn project_codex_response_item(record: &Value) -> Option<ProjectedRecordKind> {
    let payload = record.get("payload")?;
    match payload.get("type").and_then(Value::as_str)? {
        "message" if payload.get("role").and_then(Value::as_str) == Some("user") => {
            let dispatches: Vec<String> = payload
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
                .collect();
            (!dispatches.is_empty()).then_some(ProjectedRecordKind::CodexAssignment { dispatches })
        }
        "function_call" | "custom_tool_call" => {
            let name = payload.get("name").and_then(Value::as_str)?.to_string();
            let raw = payload
                .get("arguments")
                .or_else(|| payload.get("input"))
                .cloned()
                .unwrap_or(Value::Null);
            Some(ProjectedRecordKind::CodexToolCall {
                input: project_tool_input(&name, raw),
                name,
                call_id: payload
                    .get("call_id")
                    .or_else(|| payload.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        }
        "function_call_output" => Some(ProjectedRecordKind::CodexToolResult {
            call_id: payload
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        _ => None,
    }
}

fn project_claude_meta(record: &Value) -> Option<ProjectedRecordKind> {
    (record.get("taskKind").and_then(Value::as_str) == Some("in_process_teammate")).then(|| {
        ProjectedRecordKind::ClaudeMeta {
            worker_name: record
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            agent_id: string_from_keys(record, &["agentId"]),
            parent_session_id: string_from_keys(
                record,
                &["parentSessionId", "parentSessionID", "parent_session_id"],
            ),
            parent_call_id: string_from_keys(
                record,
                &["parentToolUseId", "parentToolUseID", "parent_tool_use_id"],
            ),
        }
    })
}

fn project_claude_message(record: &Value) -> Option<ProjectedRecordKind> {
    let mut blocks = Vec::new();
    for block in record
        .pointer("/message/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match block.get("type").and_then(Value::as_str) {
            Some("tool_use") => {
                let name = block.get("name").and_then(Value::as_str)?.to_string();
                blocks.push(ClaudeBlock::ToolUse {
                    id: block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    input: project_tool_input(
                        &name,
                        block.get("input").cloned().unwrap_or(Value::Null),
                    ),
                    name,
                });
            }
            Some("tool_result") => blocks.push(ClaudeBlock::ToolResult {
                tool_use_id: block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }),
            _ => {}
        }
    }
    let teammate = record
        .pointer("/message/content")
        .and_then(Value::as_str)
        .and_then(project_teammate_envelope);
    Some(ProjectedRecordKind::ClaudeMessage {
        record_type: record.get("type")?.as_str()?.to_string(),
        session_id: string_from_keys(record, &["sessionId"]),
        is_sidechain: record
            .get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        agent_id: string_from_keys(record, &["agentId"]),
        parent_session_id: string_from_keys(
            record,
            &["parentSessionId", "parentSessionID", "parent_session_id"],
        ),
        cwd: path_at(record, &["cwd"]),
        blocks,
        stop_reason: string_at(record, &["message", "stop_reason"]),
        teammate,
    })
}

fn project_tool_input(name: &str, raw: Value) -> ProjectedToolInput {
    let parsed = raw
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(raw);
    let mut input = ProjectedToolInput {
        task_name: string_from_keys(&parsed, &["task_name", "name"]),
        dispatches: string_from_keys(&parsed, &["message", "prompt"])
            .map(|text| dispatch_markers(&text))
            .unwrap_or_default(),
        questions: project_questions(parsed.get("questions")),
        file_path: string_from_keys(&parsed, &["file_path"]),
        path: string_from_keys(&parsed, &["path"]),
        uri: string_from_keys(&parsed, &["uri"]),
        ..ProjectedToolInput::default()
    };
    input.commands = match name {
        "exec" => code_mode_exec_commands(&parsed),
        "exec_command" | "Bash" => command_text(&parsed).into_iter().collect(),
        _ => Vec::new(),
    };
    input
}

fn project_questions(value: Option<&Value>) -> Vec<ProjectedQuestion> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|question| ProjectedQuestion {
            id: string_from_keys(question, &["id"]),
            header: string_from_keys(question, &["header"]),
            labels: question
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|option| option.get("label").and_then(Value::as_str))
                .map(str::to_string)
                .collect(),
        })
        .collect()
}

fn project_teammate_envelope(content: &str) -> Option<TeammateEnvelope> {
    let start = content.find("<teammate-message>")? + "<teammate-message>".len();
    let end = content[start..].find("</teammate-message>")? + start;
    let envelope: Value = serde_json::from_str(content[start..end].trim()).ok()?;
    Some(TeammateEnvelope {
        envelope_type: string_from_keys(&envelope, &["type"]),
        from: string_from_keys(&envelope, &["from"]),
        idle_reason: string_from_keys(&envelope, &["idleReason"]),
    })
}

pub(crate) fn contains_dispatch(
    dispatches: &[String],
    slug: &str,
    parent_session_id: Option<&str>,
) -> bool {
    dispatches.iter().any(|marker| {
        let Some(basename) = marker.strip_prefix(DISPATCH_DIR) else {
            return false;
        };
        STAGES.iter().any(|stage| {
            let canonical = format!("{DISPATCH_BASENAME_PREFIX}{slug}-{stage}.md");
            basename == canonical
                || parent_session_id
                    .is_some_and(|parent| basename == format!("{parent}-{canonical}"))
        })
    })
}

fn dispatch_markers(text: &str) -> Vec<String> {
    let mut markers = Vec::new();
    let mut remainder = text;
    while let Some(start) = remainder.find(DISPATCH_DIR) {
        let candidate = &remainder[start..];
        let Some(end) = candidate.find(".md") else {
            break;
        };
        let marker = &candidate[..end + 3];
        let basename = &marker[DISPATCH_DIR.len()..];
        if marker.len() <= 640
            && basename.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '.')
            })
            && basename.contains(DISPATCH_BASENAME_PREFIX)
            && STAGES
                .iter()
                .any(|stage| basename.ends_with(&format!("-{stage}.md")))
        {
            markers.push(marker.to_string());
        }
        remainder = &candidate[end + 3..];
    }
    markers
}

fn command_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    ["cmd", "command", "input"]
        .iter()
        .find_map(|key| value.get(*key).and_then(command_text))
}

fn code_mode_exec_commands(value: &Value) -> Vec<String> {
    if let Some(module) = value.as_str() {
        return nested_exec_commands(module);
    }
    if let Some(command) = value
        .get("cmd")
        .or_else(|| value.get("command"))
        .and_then(command_text)
    {
        return vec![command];
    }
    ["input", "arguments"]
        .iter()
        .find_map(|key| value.get(*key))
        .map(code_mode_exec_commands)
        .unwrap_or_default()
}

fn nested_exec_commands(module: &str) -> Vec<String> {
    const CALL: &str = "tools.exec_command";

    let mut commands = Vec::new();
    let mut offset = 0;
    while let Some(relative_start) = module[offset..].find(CALL) {
        let call_start = offset + relative_start + CALL.len();
        let after_name = &module[call_start..];
        let whitespace = after_name.len() - after_name.trim_start().len();
        let argument_start = call_start + whitespace;
        if module.as_bytes().get(argument_start) != Some(&b'(') {
            offset = call_start;
            continue;
        }
        let source = &module[argument_start + 1..];
        let Some((argument, consumed)) = balanced_call_argument(source) else {
            break;
        };
        if let Ok(value) = serde_json::from_str::<Value>(argument.trim()) {
            if let Some(command) = command_text(&value) {
                commands.push(command);
            }
        }
        offset = argument_start + 1 + consumed;
    }
    commands
}

fn balanced_call_argument(source: &str) -> Option<(&str, usize)> {
    let mut depth = 1_u32;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
        if let Some(expected) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == expected {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&source[..index], index + character.len_utf8()));
                }
            }
            _ => {}
        }
    }
    None
}

fn string_from_keys(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn path_at(value: &Value, path: &[&str]) -> Option<PathBuf> {
    string_at(value, path).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn projection_drops_transcript_text() {
        let projected = project_record(
            json!({
                "timestamp": 1,
                "type": "assistant",
                "sessionId": "session",
                "cwd": "/repo",
                "isSidechain": false,
                "message": {
                    "content": [
                        {"type": "text", "text": "private transcript"},
                        {"type": "tool_use", "id": "call", "name": "Read", "input": {"file_path": "/repo/entity.md"}}
                    ],
                    "stop_reason": null
                }
            }),
            Path::new("session.jsonl"),
            0,
        )
        .expect("projection");
        assert!(!format!("{projected:?}").contains("private transcript"));
        assert!(format!("{projected:?}").contains("/repo/entity.md"));
    }

    #[test]
    fn prefixed_dispatch_is_scoped_to_exact_parent() {
        let dispatches =
            dispatch_markers("Read /tmp/spacedock-dispatch/parent-spacedock-ensign-task-plan.md");
        assert!(contains_dispatch(&dispatches, "task", Some("parent")));
        assert!(!contains_dispatch(&dispatches, "task", Some("other")));
        assert!(!contains_dispatch(&dispatches, "other", Some("parent")));
    }
}
