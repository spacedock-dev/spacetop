use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::{ActivityHandler, AgentRuntime, EntityActivity};

use super::projection::EvidenceOrder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEvent {
    pub runtime: AgentRuntime,
    pub session_id: String,
    pub updated_unix: i64,
    pub source: PathBuf,
    pub byte_offset: u64,
    pub evidence_kind_rank: u8,
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
    let mut ordered: Vec<_> = events.iter().collect();
    ordered.sort_by_key(|event| {
        (
            event.updated_unix,
            &event.source,
            event.byte_offset,
            event.evidence_kind_rank,
            event_kind_rank(&event.kind),
            event.runtime,
            &event.session_id,
        )
    });

    let mut workers = HashMap::new();
    let mut first_officers = HashMap::new();
    let mut gates = HashMap::new();
    let mut latest = None;

    for event in ordered {
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

fn event_kind_rank(kind: &ActivityEventKind) -> u8 {
    match kind {
        ActivityEventKind::WorkerStarted => 0,
        ActivityEventKind::FirstOfficerStarted => 1,
        ActivityEventKind::HumanGateOpened { .. } => 2,
        ActivityEventKind::HumanGateResolved { .. } => 3,
        ActivityEventKind::WorkerStopped => 4,
        ActivityEventKind::FirstOfficerStopped => 5,
    }
}

pub(crate) fn push_event(
    events: &mut Vec<ActivityEvent>,
    runtime: AgentRuntime,
    session_id: &str,
    order: &EvidenceOrder,
    fallback_time: i64,
    kind: ActivityEventKind,
) {
    events.push(ActivityEvent {
        runtime,
        session_id: session_id.to_string(),
        updated_unix: order.updated_unix.unwrap_or(fallback_time),
        source: order.source.clone(),
        byte_offset: order.byte_offset,
        evidence_kind_rank: order.kind_rank,
        kind,
    });
}
