//! Recorded Spacedock v1 gate facts. These do not authenticate a decision.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GateData {
    #[default]
    Absent,
    Valid {
        document: GateDocument,
        warnings: Vec<GateWarning>,
    },
    Invalid {
        diagnostics: Vec<String>,
    },
}
impl GateData {
    pub fn document(&self) -> Option<&GateDocument> {
        match self {
            Self::Valid { document, .. } => Some(document),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateWarning {
    pub path: String,
    pub field: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateDocument {
    pub version: u32,
    pub records: Vec<GateRecord>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRecord {
    pub id: String,
    pub stage: String,
    pub attempts: Vec<GateAttempt>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateAttempt {
    pub id: String,
    pub briefing: GateBriefing,
    pub withdrawal: Option<GateWithdrawal>,
    pub resolution: Option<GateResolution>,
    pub application: Option<GateApplication>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateBriefing {
    pub id: String,
    pub digest: String,
    #[serde(
        rename = "request-digest",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub request_digest: Option<String>,
    #[serde(rename = "room-ref")]
    pub room_ref: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateWithdrawal {
    pub by: String,
    pub at: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateResolution {
    #[serde(rename = "type")]
    pub record_type: String,
    pub id: String,
    pub briefing: String,
    pub by: String,
    pub at: String,
    pub decision: GateDecision,
    #[serde(default)]
    pub reason: String,
    pub conn: Option<GateConn>,
    #[serde(default)]
    pub includes: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateConn {
    pub quote: String,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateApplication {
    #[serde(rename = "target-stage")]
    pub target_stage: String,
    pub state: GateApplicationState,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateDecision {
    Approve,
    Revise,
    Hold,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateApplicationState {
    Pending,
    Consumed,
    Superseded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateReadiness {
    Validating,
    NeedsPreparation,
    AwaitingCaptain,
    WithdrawnAwaitingPrepare,
    FeedbackPending,
    #[serde(rename = "not-applicable")]
    Held,
    ApprovedAwaitingAdvance,
    ApprovedAwaitingMerge,
    Consumed,
    Superseded,
    Invalid,
}
impl GateReadiness {
    pub fn label(self) -> &'static str {
        match self {
            Self::Held => "not-applicable",
            Self::Validating => "validating",
            Self::NeedsPreparation => "needs-preparation",
            Self::AwaitingCaptain => "awaiting-captain",
            Self::WithdrawnAwaitingPrepare => "withdrawn-awaiting-prepare",
            Self::FeedbackPending => "feedback-pending",
            Self::ApprovedAwaitingAdvance => "approved-awaiting-advance",
            Self::ApprovedAwaitingMerge => "approved-awaiting-merge",
            Self::Consumed => "consumed",
            Self::Superseded => "superseded",
            Self::Invalid => "invalid",
        }
    }
}
impl std::str::FromStr for GateReadiness {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        [
            Self::Held,
            Self::Validating,
            Self::NeedsPreparation,
            Self::AwaitingCaptain,
            Self::WithdrawnAwaitingPrepare,
            Self::FeedbackPending,
            Self::ApprovedAwaitingAdvance,
            Self::ApprovedAwaitingMerge,
            Self::Consumed,
            Self::Superseded,
            Self::Invalid,
        ]
        .into_iter()
        .find(|state| state.label() == value)
        .ok_or_else(|| format!("invalid gate readiness: {value}"))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GatePreparation {
    pub proven: bool,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDetails {
    pub readiness: Option<GateReadiness>,
    pub data: GateData,
    pub preparation: GatePreparation,
    pub selected_gate: Option<String>,
    pub selected_attempt: Option<String>,
    pub stage: String,
}
