//! Recorded advisory review facts, never gate authority or inferred completion.
use super::{CanonicalBriefing, GateDecision, GateResolution, RoomDiagnostic};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReviewPointer {
    #[default]
    Absent,
    Valid {
        pointer: RoundPointer,
    },
    Invalid {
        diagnostic: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundPointer {
    pub id: String,
    pub stage: String,
    pub cycle: u32,
    pub briefing: RoundBinding,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundBinding {
    pub id: String,
    pub digest: String,
    #[serde(rename = "room-ref")]
    pub room_ref: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoundIdentity {
    pub origin: PathBuf,
    pub entity_id: String,
    pub stage: String,
    pub cycle: u32,
    pub briefing_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReviewRecordIdentity {
    pub round: RoundIdentity,
    pub id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundProvenance {
    CurrentPointer,
    Retained,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewCompleteness {
    Answered,
    Incomplete,
    Invalid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewAnnotation {
    pub body: Option<String>,
    pub target: Option<String>,
    pub kind: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewEntry {
    Annotation(ReviewAnnotation),
    Resolution(GateResolution),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub identity: ReviewRecordIdentity,
    pub ordinal: usize,
    pub by: String,
    pub at: String,
    pub includes: Vec<String>,
    pub entry: ReviewEntry,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDiagnostic {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub problem: RoomDiagnostic,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelReference {
    pub id: String,
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionRound {
    pub identity: RoundIdentity,
    pub provenance: RoundProvenance,
    pub room: PathBuf,
    pub briefing: Option<CanonicalBriefing>,
    pub digest: Option<String>,
    pub references: Vec<LabelReference>,
    pub records: Vec<ReviewRecord>,
    pub completeness: ReviewCompleteness,
    pub diagnostics: Vec<ReviewDiagnostic>,
    pub invalid_relationships: Vec<RecordedRelationship>,
}
impl CorrectionRound {
    pub fn reviewer(&self) -> Option<&ReviewRecord> {
        self.records
            .iter()
            .find(|r| matches!(r.entry, ReviewEntry::Resolution(_)))
    }
    pub fn decision(&self) -> Option<GateDecision> {
        match &self.reviewer()?.entry {
            ReviewEntry::Resolution(r) => Some(r.decision),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CorrectionHistory {
    pub rounds: Vec<CorrectionRound>,
    pub diagnostics: Vec<ReviewDiagnostic>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipKind {
    RecordedIncludes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipDirection {
    Outgoing,
    Incoming,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipRole {
    ReviewerEvidence,
    FollowUp,
    BeforeVerdict,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipValidation {
    Valid,
    Invalid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedRelationship {
    pub source: ReviewRecordIdentity,
    pub target: ReviewRecordIdentity,
    pub kind: RelationshipKind,
    pub direction: RelationshipDirection,
    pub role: RelationshipRole,
    pub ordinal: usize,
    pub validation: RelationshipValidation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrectionRelationships {
    Recorded(Vec<RecordedRelationship>),
    NoRecordedRelationship,
    Unavailable,
}
