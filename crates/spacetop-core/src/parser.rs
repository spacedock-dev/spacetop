use std::fs;
use std::fs::DirEntry;
use std::path::Path;

use thiserror::Error;

mod archive;
mod frontmatter;
mod item;
mod readme;
mod snapshot;
mod worktree;

pub use archive::{archive_dir, load_archived_items};
pub(crate) use frontmatter::{split_frontmatter, SplitFrontmatter};
pub use item::parse_work_item;
pub use readme::parse_workflow_readme;
pub use snapshot::load_workflow_dir;

pub use crate::domain::EntityParseError;

impl ParseError {
    /// Return `true` when this parse error originated from a single entity
    /// (frontmatter or schema validation) rather than from filesystem
    /// failures. Used by the snapshot loader to decide whether to capture the
    /// error as a per-entity `EntityParseError` or bail with a hard `Err`.
    pub(crate) fn is_per_entity_parse_failure(&self) -> bool {
        matches!(
            self,
            ParseError::MissingFrontmatter { .. }
                | ParseError::UnterminatedFrontmatter { .. }
                | ParseError::MalformedYaml { .. }
                | ParseError::MissingRequiredField { .. }
                | ParseError::UnknownStatus { .. }
        )
    }

    /// Derive `(line, column)` from the underlying `serde_yaml::Error` when
    /// this is a `MalformedYaml` variant. Returns `(None, None)` for all
    /// other variants and when the YAML error has no location information.
    pub(crate) fn yaml_location(&self) -> (Option<u32>, Option<u32>) {
        if let ParseError::MalformedYaml { source, .. } = self {
            if let Some(loc) = source.location() {
                return (Some(loc.line() as u32), Some(loc.column() as u32));
            }
        }
        (None, None)
    }
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{path}: failed to read file: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: failed to read directory: {source}")]
    ReadDirectory {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: missing YAML frontmatter delimited by ---")]
    MissingFrontmatter { path: String },
    #[error("{path}: unterminated YAML frontmatter delimited by ---")]
    UnterminatedFrontmatter { path: String },
    #[error("{path}: malformed YAML frontmatter: {source}")]
    MalformedYaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("{path}: missing required field '{field}'")]
    MissingRequiredField { path: String, field: &'static str },
    #[error("{path}: unknown status '{status}'; allowed statuses: {allowed}")]
    UnknownStatus {
        path: String,
        status: String,
        allowed: String,
    },
}

pub(crate) fn read_directory(path: &Path) -> Result<Vec<DirEntry>, ParseError> {
    let path_label = display_path(path);
    fs::read_dir(path)
        .map_err(|source| ParseError::ReadDirectory {
            path: path_label.clone(),
            source,
        })?
        .map(|entry| {
            entry.map_err(|source| ParseError::ReadDirectory {
                path: path_label.clone(),
                source,
            })
        })
        .collect()
}

pub(crate) fn is_readme_path(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("README.md")
}

pub(crate) fn is_markdown_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("md")
}

pub(crate) fn required(
    value: Option<String>,
    path: &Path,
    field: &'static str,
) -> Result<String, ParseError> {
    let Some(value) = optional_text(value) else {
        return Err(ParseError::MissingRequiredField {
            path: display_path(path),
            field,
        });
    };
    Ok(value)
}

pub(crate) fn optional_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests;
