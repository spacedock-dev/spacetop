//! Attention is a projection of current recorded facts, never runtime activity.
use super::{GateData, GatePreparation, GateReadiness};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionReason {
    ReviewRequested,
    RecordedBlocker,
    RecordedFollowUp,
    PreparationNeeded,
    ReplacementNeeded,
    AdvancePending,
    MergePending,
    InvalidInformation,
    MissingOrUnverifiedInformation,
}

impl AttentionReason {
    pub fn from_gate(
        data: &GateData,
        readiness: Option<GateReadiness>,
        preparation: &GatePreparation,
    ) -> Option<Self> {
        if matches!(data, GateData::Invalid { .. }) {
            return Some(Self::InvalidInformation);
        }
        match readiness? {
            GateReadiness::AwaitingCaptain => Some(Self::ReviewRequested),
            GateReadiness::Held => Some(Self::RecordedBlocker),
            GateReadiness::FeedbackPending => Some(Self::RecordedFollowUp),
            GateReadiness::NeedsPreparation => Some(Self::PreparationNeeded),
            GateReadiness::WithdrawnAwaitingPrepare => Some(Self::ReplacementNeeded),
            GateReadiness::ApprovedAwaitingAdvance => Some(Self::AdvancePending),
            GateReadiness::ApprovedAwaitingMerge => Some(Self::MergePending),
            GateReadiness::Invalid => Some(Self::InvalidInformation),
            GateReadiness::Validating if !preparation.diagnostics.is_empty() => {
                Some(Self::MissingOrUnverifiedInformation)
            }
            GateReadiness::Validating | GateReadiness::Consumed | GateReadiness::Superseded => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_facts_are_exhaustive_and_invalid_data_takes_precedence() {
        use AttentionReason::*;
        use GateReadiness as R;
        let cases = [
            (R::AwaitingCaptain, Some(ReviewRequested)),
            (R::Held, Some(RecordedBlocker)),
            (R::FeedbackPending, Some(RecordedFollowUp)),
            (R::NeedsPreparation, Some(PreparationNeeded)),
            (R::WithdrawnAwaitingPrepare, Some(ReplacementNeeded)),
            (R::ApprovedAwaitingAdvance, Some(AdvancePending)),
            (R::ApprovedAwaitingMerge, Some(MergePending)),
            (R::Invalid, Some(InvalidInformation)),
            (R::Validating, None),
            (R::Consumed, None),
            (R::Superseded, None),
        ];
        for (readiness, expected) in cases {
            let mut preparation = GatePreparation::default();
            assert_eq!(
                AttentionReason::from_gate(&GateData::Absent, Some(readiness), &preparation),
                expected
            );
            preparation
                .diagnostics
                .push("Exact verification failure".into());
            assert_eq!(
                AttentionReason::from_gate(&GateData::Absent, Some(readiness), &preparation),
                if readiness == R::Validating {
                    Some(MissingOrUnverifiedInformation)
                } else {
                    expected
                }
            );
            assert_eq!(
                AttentionReason::from_gate(
                    &GateData::Invalid {
                        diagnostics: vec![]
                    },
                    Some(readiness),
                    &preparation
                ),
                Some(InvalidInformation)
            );
        }
        assert_eq!(
            AttentionReason::from_gate(&GateData::Absent, None, &GatePreparation::default()),
            None
        );
        assert_eq!(
            AttentionReason::from_gate(
                &GateData::Invalid {
                    diagnostics: vec![]
                },
                None,
                &GatePreparation::default()
            ),
            Some(InvalidInformation)
        );
    }
}
