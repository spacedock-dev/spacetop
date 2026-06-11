use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::index::StageEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub stage_dwell_seconds: HashMap<String, i64>,
    pub cycle_time_seconds: HashMap<String, i64>,
    pub wip_by_stage: HashMap<String, usize>,
    pub throughput_completed: usize,
    pub completed_entities: usize,
}

impl Metrics {
    pub fn from_events(events: &[StageEvent]) -> Self {
        let mut by_entity: HashMap<&str, Vec<&StageEvent>> = HashMap::new();
        for event in events {
            by_entity.entry(&event.entity_id).or_default().push(event);
        }

        let mut stage_dwell_seconds = HashMap::new();
        let mut cycle_time_seconds = HashMap::new();
        let mut wip_by_stage = HashMap::new();
        let mut completed_entities = 0usize;

        for timeline in by_entity.values_mut() {
            timeline.sort_by_key(|event| event.at);
            for pair in timeline.windows(2) {
                let current = pair[0];
                let next = pair[1];
                let delta = next.at.0.saturating_sub(current.at.0);
                *stage_dwell_seconds.entry(current.to.clone()).or_insert(0) += delta;
            }
            if let (Some(first), Some(last)) = (timeline.first(), timeline.last()) {
                cycle_time_seconds.insert(
                    first.entity_id.clone(),
                    last.at.0.saturating_sub(first.at.0),
                );
                *wip_by_stage.entry(last.to.clone()).or_insert(0) += 1;
            }
            if timeline.iter().any(|event| event.to == "done") {
                completed_entities += 1;
            }
        }

        Self {
            stage_dwell_seconds,
            cycle_time_seconds,
            wip_by_stage,
            throughput_completed: completed_entities,
            completed_entities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{CommitId, CommitTime, StageEvent};

    #[test]
    fn metrics_computes_stage_dwell_seconds() {
        let events = vec![
            StageEvent {
                entity_id: "001".to_string(),
                from: None,
                to: "plan".to_string(),
                at: CommitTime(100),
                commit: CommitId("a".repeat(40)),
            },
            StageEvent {
                entity_id: "001".to_string(),
                from: Some("plan".to_string()),
                to: "verify".to_string(),
                at: CommitTime(160),
                commit: CommitId("b".repeat(40)),
            },
            StageEvent {
                entity_id: "001".to_string(),
                from: Some("verify".to_string()),
                to: "done".to_string(),
                at: CommitTime(220),
                commit: CommitId("c".repeat(40)),
            },
        ];

        let metrics = Metrics::from_events(&events);
        assert_eq!(metrics.stage_dwell_seconds.get("plan"), Some(&60));
        assert_eq!(metrics.stage_dwell_seconds.get("verify"), Some(&60));
        assert_eq!(metrics.cycle_time_seconds.get("001"), Some(&120));
        assert_eq!(metrics.completed_entities, 1);
        assert_eq!(metrics.throughput_completed, 1);
    }
}
