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
    GitError(String),
}

impl HistoryUnavailable {
    pub fn user_message(&self) -> &str {
        match self {
            Self::NotImplemented => "history is not available until v2 P2",
            Self::Loading => "history is loading",
            Self::NotGitRepository => "history unavailable: not a git repository",
            Self::ShallowClone => "history unavailable: shallow clone",
            Self::GitError(_) => "history unavailable: git log could not be read",
        }
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
    }
}
