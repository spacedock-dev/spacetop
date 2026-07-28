use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::domain::AgentRuntime;
use crate::entity_identity::entity_slug;

use super::projection::{
    contains_dispatch, ProjectedRecord, ProjectedRecordKind, ProjectedToolInput,
};
use super::reducer::{push_event, ActivityEvent, ActivityEventKind};
use super::{call_scopes_entity, cwd_matches_entity, is_gate_question, SessionScanEntity};

pub(crate) fn collect(
    records: &[&ProjectedRecord],
    entities: &[SessionScanEntity],
    repo_root: &Path,
    fallback_time: i64,
    per_entity: &mut HashMap<String, Vec<ActivityEvent>>,
) {
    let files = records_by_source(records);
    for entity in entities {
        let Some(slug) = entity_slug(&entity.path) else {
            continue;
        };
        let mut matched_children = Vec::new();
        for file_records in files.values() {
            let Some(meta) = codex_meta(file_records) else {
                continue;
            };
            let child_started = file_records.iter().any(|record| {
                matches!(
                    &record.kind,
                    ProjectedRecordKind::CodexEvent { event_type, turn_id, .. }
                        if event_type == "task_started" && turn_id.is_some()
                )
            });
            let legacy_assignment = file_records.iter().any(|record| {
                matches!(
                    &record.kind,
                    ProjectedRecordKind::CodexAssignment { dispatches }
                        if contains_dispatch(dispatches, &slug, None)
                )
            });
            let parent_started = meta.parent_thread_id.as_deref().is_some_and(|parent_id| {
                parent_confirms_start(
                    &files,
                    parent_id,
                    &meta.session_id,
                    meta.agent_path.as_deref().unwrap_or_default(),
                    entity,
                    repo_root,
                )
            });
            let child_matches = meta
                .agent_path
                .as_deref()
                .is_some_and(|path| canonical_codex_name(path, &slug))
                && meta
                    .parent_thread_id
                    .as_deref()
                    .is_some_and(|parent| !parent.is_empty())
                && meta
                    .cwd
                    .as_deref()
                    .is_some_and(|cwd| cwd_matches_entity(cwd, repo_root, entity))
                && child_started
                && (legacy_assignment || parent_started);

            if child_matches {
                let agent_path = meta.agent_path.clone().unwrap_or_default();
                matched_children.push((meta.session_id.clone(), agent_path));
                collect_worker(
                    file_records,
                    per_entity.entry(entity.id.clone()).or_default(),
                    &meta.session_id,
                    fallback_time,
                );
            } else {
                collect_first_officer(
                    file_records,
                    per_entity.entry(entity.id.clone()).or_default(),
                    &meta.session_id,
                    entity,
                    &slug,
                    fallback_time,
                );
            }
        }

        for file_records in files.values() {
            for record in file_records {
                let ProjectedRecordKind::CodexEvent {
                    event_type,
                    kind,
                    agent_thread_id,
                    agent_path,
                    ..
                } = &record.kind
                else {
                    continue;
                };
                if event_type != "sub_agent_activity" || kind.as_deref() != Some("interrupted") {
                    continue;
                }
                if let Some((session_id, _)) = matched_children.iter().find(|(session, path)| {
                    agent_thread_id.as_deref() == Some(session.as_str())
                        && agent_path.as_deref() == Some(path.as_str())
                }) {
                    push_event(
                        per_entity.entry(entity.id.clone()).or_default(),
                        AgentRuntime::Codex,
                        session_id,
                        &record.order,
                        fallback_time,
                        ActivityEventKind::WorkerStopped,
                    );
                }
            }
        }
    }
}

struct CodexMeta {
    session_id: String,
    parent_thread_id: Option<String>,
    agent_path: Option<String>,
    cwd: Option<PathBuf>,
}

fn codex_meta(records: &[&ProjectedRecord]) -> Option<CodexMeta> {
    records.iter().find_map(|record| {
        let ProjectedRecordKind::CodexSession {
            session_id,
            parent_thread_id,
            agent_path,
            cwd,
        } = &record.kind
        else {
            return None;
        };
        Some(CodexMeta {
            session_id: session_id.clone(),
            parent_thread_id: parent_thread_id.clone(),
            agent_path: agent_path.clone(),
            cwd: cwd.clone(),
        })
    })
}

fn parent_confirms_start(
    files: &BTreeMap<PathBuf, Vec<&ProjectedRecord>>,
    parent_id: &str,
    child_id: &str,
    agent_path: &str,
    entity: &SessionScanEntity,
    repo_root: &Path,
) -> bool {
    files.values().any(|records| {
        let Some(parent) = codex_meta(records) else {
            return false;
        };
        parent.session_id == parent_id
            && parent
                .cwd
                .as_deref()
                .is_some_and(|cwd| cwd_matches_entity(cwd, repo_root, entity))
            && records.iter().any(|record| {
                matches!(
                    &record.kind,
                    ProjectedRecordKind::CodexEvent {
                        event_type,
                        kind,
                        agent_thread_id,
                        agent_path: started_path,
                        ..
                    } if event_type == "sub_agent_activity"
                        && kind.as_deref() == Some("started")
                        && agent_thread_id.as_deref() == Some(child_id)
                        && started_path.as_deref() == Some(agent_path)
                )
            })
    })
}

fn collect_worker(
    records: &[&ProjectedRecord],
    events: &mut Vec<ActivityEvent>,
    session_id: &str,
    fallback_time: i64,
) {
    let mut open_turn = None;
    for record in records {
        let ProjectedRecordKind::CodexEvent {
            event_type,
            turn_id,
            ..
        } = &record.kind
        else {
            continue;
        };
        match event_type.as_str() {
            "task_started" if turn_id.is_some() => {
                open_turn.clone_from(turn_id);
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    &record.order,
                    fallback_time,
                    ActivityEventKind::WorkerStarted,
                );
            }
            "task_complete" if open_turn.as_deref() == turn_id.as_deref() => {
                push_event(
                    events,
                    AgentRuntime::Codex,
                    session_id,
                    &record.order,
                    fallback_time,
                    ActivityEventKind::WorkerStopped,
                );
                open_turn = None;
            }
            _ => {}
        }
    }
}

fn collect_first_officer(
    records: &[&ProjectedRecord],
    events: &mut Vec<ActivityEvent>,
    session_id: &str,
    entity: &SessionScanEntity,
    slug: &str,
    fallback_time: i64,
) {
    let mut open_turn = None;
    let mut scoped_turns = HashSet::new();
    for record in records {
        match &record.kind {
            ProjectedRecordKind::CodexEvent {
                event_type,
                turn_id,
                ..
            } if event_type == "task_started" => {
                open_turn.clone_from(turn_id);
            }
            ProjectedRecordKind::CodexToolCall {
                name,
                call_id,
                input,
            } => {
                let Some(turn) = open_turn.clone() else {
                    continue;
                };
                if call_scopes_entity(name, input, entity, slug, None) {
                    scoped_turns.insert(turn.clone());
                    push_event(
                        events,
                        AgentRuntime::Codex,
                        session_id,
                        &record.order,
                        fallback_time,
                        ActivityEventKind::FirstOfficerStarted,
                    );
                }
                if scoped_turns.contains(&turn)
                    && name == "request_user_input"
                    && is_gate_question(input)
                {
                    push_event(
                        events,
                        AgentRuntime::Codex,
                        session_id,
                        &record.order,
                        fallback_time,
                        ActivityEventKind::HumanGateOpened {
                            call_id: call_id.clone(),
                        },
                    );
                }
            }
            ProjectedRecordKind::CodexToolResult { call_id } => push_event(
                events,
                AgentRuntime::Codex,
                session_id,
                &record.order,
                fallback_time,
                ActivityEventKind::HumanGateResolved {
                    call_id: call_id.clone(),
                },
            ),
            ProjectedRecordKind::CodexEvent {
                event_type,
                turn_id,
                ..
            } if event_type == "task_complete" => {
                let completed = turn_id.clone().unwrap_or_default();
                if scoped_turns.remove(&completed) {
                    push_event(
                        events,
                        AgentRuntime::Codex,
                        session_id,
                        &record.order,
                        fallback_time,
                        ActivityEventKind::FirstOfficerStopped,
                    );
                }
                if open_turn.as_deref() == Some(completed.as_str()) {
                    open_turn = None;
                }
            }
            _ => {}
        }
    }
}

fn records_by_source<'a>(
    records: &'a [&ProjectedRecord],
) -> BTreeMap<PathBuf, Vec<&'a ProjectedRecord>> {
    let mut files: BTreeMap<PathBuf, Vec<_>> = BTreeMap::new();
    for record in records {
        files
            .entry(record.order.source.clone())
            .or_default()
            .push(*record);
    }
    for records in files.values_mut() {
        records.sort();
    }
    files
}

fn canonical_codex_name(path: &str, slug: &str) -> bool {
    super::STAGES.iter().any(|stage| {
        path.rsplit('/').next()
            == Some(format!("spacedock_ensign_{}_{}", slug.replace('-', "_"), stage).as_str())
            || path.rsplit('/').next() == Some(format!("spacedock_ensign_{slug}_{stage}").as_str())
    })
}

#[allow(dead_code)]
fn _projected_input_is_typed(_: &ProjectedToolInput) {}
