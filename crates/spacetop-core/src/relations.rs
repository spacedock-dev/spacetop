use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDetails {
    pub id: String,
    pub title: String,
    pub status: String,
    pub worktree: Option<String>,
    pub relations: Vec<RelationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationView {
    Issue { value: String },
    PullRequest { value: String },
    FeedbackStage { from: String, to: String },
}

impl RelationView {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Issue { .. } => "issue",
            Self::PullRequest { .. } => "pr",
            Self::FeedbackStage { .. } => "feedback-to",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_relation_labels_are_stable() {
        assert_eq!(
            RelationView::Issue {
                value: "https://example.test/1".to_string()
            }
            .label(),
            "issue"
        );
        assert_eq!(
            RelationView::PullRequest {
                value: "https://example.test/pr/1".to_string()
            }
            .label(),
            "pr"
        );
    }

    #[test]
    fn entity_details_groups_core_facts_without_ui_inference() {
        let details = EntityDetails {
            id: "050".to_string(),
            title: "Roadmap".to_string(),
            status: "verify".to_string(),
            worktree: Some("p3".to_string()),
            relations: vec![RelationView::FeedbackStage {
                from: "verify".to_string(),
                to: "plan".to_string(),
            }],
        };

        assert_eq!(details.relations[0].label(), "feedback-to");
    }
}
