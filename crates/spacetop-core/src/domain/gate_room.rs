//! Read-only room evidence, separate from recorded decisions and authentication.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomFormat {
    Current,
    Retained,
    RequestBacked,
    ExactFile,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomProblem {
    Missing,
    UnsupportedRoot,
    UnsupportedScheme,
    UnsafePath,
    Symlink,
    InvalidJson,
    InvalidSchema,
    IdentityMismatch,
    BindingMismatch,
    RequestDigestMismatch,
    BriefingDigestMismatch,
    ArtifactDigestMismatch,
    Nonregular,
    SizeLimit,
    Replaced,
    MissingObject,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomDiagnostic {
    pub kind: RoomProblem,
    pub message: String,
}
impl RoomDiagnostic {
    pub fn new(kind: RoomProblem, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for RoomDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomItemKind {
    Artifact,
    Reference,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomItem {
    pub kind: RoomItemKind,
    pub id: String,
    pub uri: String,
    pub revision: String,
    pub summary: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalBriefing {
    pub id: String,
    pub question: String,
    pub items: Vec<RoomItem>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateRoomView {
    pub format: Option<RoomFormat>,
    pub briefing: Option<CanonicalBriefing>,
    pub diagnostics: Vec<RoomDiagnostic>,
}
impl GateRoomView {
    pub fn failed(problem: RoomDiagnostic) -> Self {
        Self {
            format: None,
            briefing: None,
            diagnostics: vec![problem],
        }
    }
    pub fn verified(&self) -> bool {
        self.briefing.is_some() && self.diagnostics.is_empty()
    }
}
