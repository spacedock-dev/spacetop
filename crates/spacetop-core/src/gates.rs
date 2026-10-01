//! Pure recorded-state projection, compatible with Spacedock v0.27.3.
use crate::domain::*;

pub fn selected_attempt<'a>(
    data: &'a GateData,
    status: &str,
) -> Option<(&'a GateRecord, &'a GateAttempt)> {
    let record = data
        .document()?
        .records
        .iter()
        .find(|record| record.stage == status)?;
    Some((record, record.attempts.last()?))
}
pub fn readiness(
    data: &GateData,
    status: &str,
    stages: &[StageDefinition],
    promotion_proven: bool,
) -> Option<GateReadiness> {
    use GateReadiness as R;
    if matches!(data, GateData::Invalid { .. }) {
        return Some(R::Invalid);
    }
    let current = stages.iter().find(|stage| stage.name == status)?;
    if !current.gate || current.terminal {
        return None;
    }
    let Some((_, attempt)) = selected_attempt(data, status) else {
        return Some(if promotion_proven {
            R::NeedsPreparation
        } else {
            R::Validating
        });
    };
    if attempt.withdrawal.is_some() {
        return Some(R::WithdrawnAwaitingPrepare);
    }
    let Some(resolution) = &attempt.resolution else {
        return Some(R::AwaitingCaptain);
    };
    let Some(app) = &attempt.application else {
        return Some(match resolution.decision {
            GateDecision::Hold => R::Held,
            GateDecision::Revise => R::FeedbackPending,
            GateDecision::Approve => R::Invalid,
        });
    };
    Some(match app.state {
        GateApplicationState::Consumed => R::Consumed,
        GateApplicationState::Superseded => R::Superseded,
        GateApplicationState::Pending => {
            if app.target_stage == status || resolution.decision != GateDecision::Approve {
                return Some(R::Invalid);
            }
            match stages.iter().find(|stage| stage.name == app.target_stage) {
                Some(target) if target.terminal => R::ApprovedAwaitingMerge,
                Some(_) => R::ApprovedAwaitingAdvance,
                None => R::Invalid,
            }
        }
    })
}
pub fn details(entity: &Entity) -> GateDetails {
    let selected = selected_attempt(&entity.gates, &entity.status);
    GateDetails {
        readiness: entity.gate_readiness,
        data: entity.gates.clone(),
        preparation: entity.gate_preparation.clone(),
        stage: entity.status.clone(),
        selected_gate: selected.map(|(r, _)| r.id.clone()),
        selected_attempt: selected.map(|(_, a)| a.id.clone()),
    }
}
