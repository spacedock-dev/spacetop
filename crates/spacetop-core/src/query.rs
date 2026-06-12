use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityQuery {
    pub scope: QueryScope,
    pub status: Option<String>,
    pub text: Option<String>,
    pub field_filters: Vec<FieldFilter>,
    pub sort: EntitySort,
}

impl Default for EntityQuery {
    fn default() -> Self {
        Self {
            scope: QueryScope::Active,
            status: None,
            text: None,
            field_filters: Vec::new(),
            sort: EntitySort::Id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryScope {
    Active,
    Archived,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntitySort {
    Id,
    Status,
    /// Preserve parser-provided archive order: completed timestamp descending,
    /// filename ascending as deterministic tiebreaker.
    ArchiveDefault,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldFilter {
    HasIssue,
    HasPr,
    HasWorktreeSource,
    Verdict(String),
    MinScore(f64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryUnavailable {
    NotImplemented,
    Loading,
    NotGitRepository,
    ShallowClone,
    GitProbeError(String),
    GitLogError(String),
    GitBlobError { path: String, message: String },
    MetadataError { path: String, message: String },
}

impl HistoryUnavailable {
    pub fn user_message(&self) -> String {
        match self {
            Self::NotImplemented => "history is not available until v2 P2".to_string(),
            Self::Loading => "history is loading".to_string(),
            Self::NotGitRepository => "history unavailable: not a git repository".to_string(),
            Self::ShallowClone => "history unavailable: shallow clone".to_string(),
            Self::GitProbeError(_) => {
                "history unavailable: git repository state could not be read".to_string()
            }
            Self::GitLogError(_) => "history unavailable: git log could not be read".to_string(),
            Self::GitBlobError { path, message } => format_history_detail(
                "history unavailable: historical blob could not be read",
                path,
                message,
            ),
            Self::MetadataError { path, message } => format_history_detail(
                "history unavailable: historical entity metadata could not be parsed",
                path,
                message,
            ),
        }
    }
}

fn format_history_detail(prefix: &str, path: &str, message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        format!("{prefix} for {path}")
    } else {
        format!("{prefix} for {path}: {message}")
    }
}

pub type HistoryResult<T> = Result<T, HistoryUnavailable>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_targets_active_entities_sorted_by_id() {
        let query = EntityQuery::default();
        assert_eq!(query.scope, QueryScope::Active);
        assert_eq!(query.sort, EntitySort::Id);
        assert!(query.status.is_none());
        assert!(query.text.is_none());
        assert!(query.field_filters.is_empty());
    }

    #[test]
    fn history_unavailable_has_stable_user_message() {
        assert_eq!(
            HistoryUnavailable::NotImplemented.user_message(),
            "history is not available until v2 P2"
        );
        assert_eq!(
            HistoryUnavailable::GitLogError("fatal: bad object\n".to_string()).user_message(),
            "history unavailable: git log could not be read"
        );
        assert_eq!(
            HistoryUnavailable::GitBlobError {
                path: "docs/workflow/001.md".to_string(),
                message: "fatal: path not found\n".to_string(),
            }
            .user_message(),
            "history unavailable: historical blob could not be read for docs/workflow/001.md: fatal: path not found"
        );
        assert_eq!(
            HistoryUnavailable::MetadataError {
                path: "docs/workflow/001.md".to_string(),
                message: "missing status".to_string(),
            }
            .user_message(),
            "history unavailable: historical entity metadata could not be parsed for docs/workflow/001.md: missing status"
        );
    }
}
