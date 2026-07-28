use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::domain::AgentRuntime;
use crate::entity_identity::entity_slug;

use super::projection::{ClaudeBlock, ProjectedRecord, ProjectedRecordKind, TeammateEnvelope};
use super::reducer::{push_event, ActivityEvent, ActivityEventKind};
use super::{call_scopes_entity, cwd_matches_entity, is_gate_question, SessionScanEntity};

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
    source_stem: String,
}

pub(crate) fn collect(
    records: &[&ProjectedRecord],
    entities: &[SessionScanEntity],
    repo_root: &Path,
    fallback_time: i64,
    per_entity: &mut HashMap<String, Vec<ActivityEvent>>,
) {
    let files = records_by_source(records);
    let teammate_meta = teammate_metadata(&files);

    for entity in entities {
        let Some(slug) = entity_slug(&entity.path) else {
            continue;
        };
        let mut dispatches = Vec::new();
        for file_records in files.values() {
            if is_sidechain_file(file_records) {
                continue;
            }
            let Some(parent_session_id) = claude_session_id(file_records) else {
                continue;
            };
            if !file_records.iter().any(|record| {
                message_cwd(record)
                    .is_some_and(|cwd| cwd_matches_entity(cwd.as_path(), repo_root, entity))
            }) {
                continue;
            }
            collect_first_officer(
                file_records,
                per_entity.entry(entity.id.clone()).or_default(),
                &parent_session_id,
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
            let child_records: Vec<_> = files
                .iter()
                .filter(|(path, records)| {
                    claude_parent_session_from_path(path).as_deref()
                        == Some(dispatch.parent_session_id.as_str())
                        && normalized_source_stem(path) == meta.source_stem
                        && records.iter().any(|record| {
                            matches!(
                                &record.kind,
                                ProjectedRecordKind::ClaudeMessage {
                                    is_sidechain: true,
                                    agent_id: Some(agent_id),
                                    cwd: Some(cwd),
                                    ..
                                } if agent_id == &meta.agent_id
                                    && cwd_matches_entity(cwd, repo_root, entity)
                            )
                        })
                })
                .flat_map(|(_, records)| records.iter().copied())
                .collect();
            if child_records.is_empty() {
                continue;
            }
            let parent_records: Vec<_> = files
                .values()
                .filter(|records| {
                    !is_sidechain_file(records)
                        && claude_session_id(records).as_deref()
                            == Some(dispatch.parent_session_id.as_str())
                })
                .flatten()
                .copied()
                .collect();
            collect_worker_lifecycle(
                &child_records,
                &parent_records,
                meta,
                same_name_dispatches == 1,
                per_entity.entry(entity.id.clone()).or_default(),
                fallback_time,
            );
        }
    }
}

fn collect_worker_lifecycle(
    child_records: &[&ProjectedRecord],
    parent_records: &[&ProjectedRecord],
    meta: &ClaudeTeammateMeta,
    idle_is_unambiguous: bool,
    events: &mut Vec<ActivityEvent>,
    fallback_time: i64,
) {
    let mut assistants: Vec<_> = child_records
        .iter()
        .filter(|record| {
            matches!(
                &record.kind,
                ProjectedRecordKind::ClaudeMessage {
                    record_type,
                    is_sidechain: true,
                    agent_id: Some(agent_id),
                    ..
                } if record_type == "assistant" && agent_id == &meta.agent_id
            )
        })
        .copied()
        .collect();
    assistants.sort();
    let Some(first) = assistants.first() else {
        return;
    };
    push_event(
        events,
        AgentRuntime::ClaudeCode,
        &meta.agent_id,
        &first.order,
        fallback_time,
        ActivityEventKind::WorkerStarted,
    );

    if !idle_is_unambiguous {
        return;
    }
    let mut idle_records: Vec<_> = parent_records
        .iter()
        .filter(|record| {
            teammate_envelope(record)
                .is_some_and(|envelope| is_idle_from(envelope, &meta.worker_name))
        })
        .copied()
        .collect();
    idle_records.sort();
    for idle in &idle_records {
        push_event(
            events,
            AgentRuntime::ClaudeCode,
            &meta.agent_id,
            &idle.order,
            fallback_time,
            ActivityEventKind::WorkerStopped,
        );
    }

    let mut boundaries: Vec<_> = parent_records
        .iter()
        .filter(|record| {
            teammate_envelope(record).is_some_and(|envelope| {
                envelope.from.as_deref() == Some(meta.worker_name.as_str())
                    && !is_idle_from(envelope, &meta.worker_name)
            })
        })
        .copied()
        .collect();
    boundaries.sort();
    for boundary in boundaries {
        let boundary_at = boundary.order.updated_unix.unwrap_or(fallback_time);
        let stopped_before = idle_records
            .iter()
            .any(|idle| idle.order.updated_unix.unwrap_or(fallback_time) < boundary_at);
        if !stopped_before {
            continue;
        }
        if let Some(reopened) = assistants
            .iter()
            .find(|assistant| assistant.order.updated_unix.unwrap_or(fallback_time) > boundary_at)
        {
            push_event(
                events,
                AgentRuntime::ClaudeCode,
                &meta.agent_id,
                &reopened.order,
                fallback_time,
                ActivityEventKind::WorkerStarted,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_first_officer(
    records: &[&ProjectedRecord],
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
        let ProjectedRecordKind::ClaudeMessage {
            record_type,
            blocks,
            stop_reason,
            teammate,
            ..
        } = &record.kind
        else {
            continue;
        };
        if handoff_pending && record_type == "assistant" {
            scoped = true;
            handoff_pending = false;
            push_event(
                events,
                AgentRuntime::ClaudeCode,
                session_id,
                &record.order,
                fallback_time,
                ActivityEventKind::FirstOfficerStarted,
            );
        }
        for block in blocks {
            match block {
                ClaudeBlock::ToolUse { id, name, input } => {
                    if call_scopes_entity(name, input, entity, slug, Some(session_id)) {
                        scoped = true;
                        push_event(
                            events,
                            AgentRuntime::ClaudeCode,
                            session_id,
                            &record.order,
                            fallback_time,
                            ActivityEventKind::FirstOfficerStarted,
                        );
                    }
                    if name == "Agent"
                        && call_scopes_entity(name, input, entity, slug, Some(session_id))
                    {
                        if let Some(worker_name) = input.task_name.as_ref() {
                            dispatched_names.insert(worker_name.clone());
                            dispatches.push(ClaudeDispatch {
                                parent_session_id: session_id.to_string(),
                                call_id: id.clone(),
                                worker_name: worker_name.clone(),
                            });
                        }
                    }
                    if scoped && name == "AskUserQuestion" && is_gate_question(input) {
                        push_event(
                            events,
                            AgentRuntime::ClaudeCode,
                            session_id,
                            &record.order,
                            fallback_time,
                            ActivityEventKind::HumanGateOpened {
                                call_id: id.clone(),
                            },
                        );
                    }
                }
                ClaudeBlock::ToolResult { tool_use_id } => push_event(
                    events,
                    AgentRuntime::ClaudeCode,
                    session_id,
                    &record.order,
                    fallback_time,
                    ActivityEventKind::HumanGateResolved {
                        call_id: tool_use_id.clone(),
                    },
                ),
            }
        }
        if scoped && stop_reason.as_deref() == Some("end_turn") {
            push_event(
                events,
                AgentRuntime::ClaudeCode,
                session_id,
                &record.order,
                fallback_time,
                ActivityEventKind::FirstOfficerStopped,
            );
            scoped = false;
        }
        if teammate.as_ref().is_some_and(|envelope| {
            envelope
                .from
                .as_ref()
                .is_some_and(|from| dispatched_names.contains(from))
                && is_idle_from(envelope, envelope.from.as_deref().unwrap_or_default())
        }) {
            scoped = false;
            handoff_pending = true;
        }
    }
}

fn teammate_metadata(files: &BTreeMap<PathBuf, Vec<&ProjectedRecord>>) -> Vec<ClaudeTeammateMeta> {
    files
        .iter()
        .filter_map(|(path, records)| {
            let (worker_name, explicit_agent, explicit_parent, parent_call_id) =
                records.iter().find_map(|record| {
                    let ProjectedRecordKind::ClaudeMeta {
                        worker_name,
                        agent_id,
                        parent_session_id,
                        parent_call_id,
                    } = &record.kind
                    else {
                        return None;
                    };
                    Some((
                        worker_name.clone(),
                        agent_id.clone(),
                        parent_session_id.clone(),
                        parent_call_id.clone(),
                    ))
                })?;
            let parent_from_path = claude_parent_session_from_path(path)?;
            let parent_session_id = explicit_parent.unwrap_or_else(|| parent_from_path.clone());
            if parent_session_id != parent_from_path {
                return None;
            }
            let source_stem = normalized_source_stem(path);
            let sibling_agent_ids: HashSet<_> = files
                .iter()
                .filter(|(candidate_path, _)| {
                    claude_parent_session_from_path(candidate_path).as_deref()
                        == Some(parent_session_id.as_str())
                        && normalized_source_stem(candidate_path) == source_stem
                })
                .flat_map(|(_, candidate_records)| candidate_records.iter())
                .filter_map(|record| {
                    let ProjectedRecordKind::ClaudeMessage {
                        is_sidechain: true,
                        agent_id,
                        ..
                    } = &record.kind
                    else {
                        return None;
                    };
                    agent_id.clone()
                })
                .collect();
            let agent_id = match explicit_agent {
                Some(agent_id)
                    if sibling_agent_ids.is_empty() || sibling_agent_ids.contains(&agent_id) =>
                {
                    agent_id
                }
                None if sibling_agent_ids.len() == 1 => {
                    sibling_agent_ids.into_iter().next().unwrap_or_default()
                }
                _ => return None,
            };
            Some(ClaudeTeammateMeta {
                parent_session_id,
                parent_call_id,
                worker_name,
                agent_id,
                source_stem,
            })
        })
        .collect()
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

fn is_sidechain_file(records: &[&ProjectedRecord]) -> bool {
    records.iter().any(|record| {
        matches!(
            record.kind,
            ProjectedRecordKind::ClaudeMessage {
                is_sidechain: true,
                ..
            }
        )
    })
}

fn claude_session_id(records: &[&ProjectedRecord]) -> Option<String> {
    records.iter().find_map(|record| {
        let ProjectedRecordKind::ClaudeMessage { session_id, .. } = &record.kind else {
            return None;
        };
        session_id.clone()
    })
}

fn message_cwd(record: &ProjectedRecord) -> Option<PathBuf> {
    let ProjectedRecordKind::ClaudeMessage { cwd, .. } = &record.kind else {
        return None;
    };
    cwd.clone()
}

fn teammate_envelope(record: &ProjectedRecord) -> Option<&TeammateEnvelope> {
    let ProjectedRecordKind::ClaudeMessage { teammate, .. } = &record.kind else {
        return None;
    };
    teammate.as_ref()
}

fn is_idle_from(envelope: &TeammateEnvelope, worker_name: &str) -> bool {
    envelope.envelope_type.as_deref() == Some("idle_notification")
        && envelope.idle_reason.as_deref() == Some("available")
        && envelope.from.as_deref() == Some(worker_name)
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

fn normalized_source_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .strip_suffix(".meta")
        .unwrap_or_else(|| path.file_stem().and_then(OsStr::to_str).unwrap_or_default())
        .to_string()
}
