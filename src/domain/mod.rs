use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemFrontmatter {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid workflow frontmatter: {0}")]
    InvalidFrontmatter(#[from] serde_yaml::Error),
}

pub fn parse_work_item_frontmatter(input: &str) -> Result<WorkItemFrontmatter, DomainError> {
    Ok(serde_yaml::from_str(input)?)
}

#[cfg(test)]
mod tests {
    use super::parse_work_item_frontmatter;

    #[test]
    fn parses_basic_work_item_frontmatter() {
        let item = parse_work_item_frontmatter(
            r#"
id: "001"
title: Scaffold Rust CLI Project
status: implement
"#,
        )
        .expect("frontmatter should parse");

        assert_eq!(item.id, "001");
        assert_eq!(item.title, "Scaffold Rust CLI Project");
        assert_eq!(item.status, "implement");
    }
}
