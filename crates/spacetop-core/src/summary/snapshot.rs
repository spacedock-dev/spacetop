use super::{digest, prompt, SummaryError, SummaryProvider};
use crate::domain::Entity;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SummaryKey {
    pub workflow: PathBuf,
    pub entity: String,
    pub source: PathBuf,
    pub input_digest: String,
    pub provider: SummaryProvider,
    pub model: String,
    pub prompt_version: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummarySnapshot {
    pub key: SummaryKey,
    pub input: Vec<u8>,
}

/// Captures only the already selected task copies, never linked evidence.
/// Authority is the query's typed entity, not worktree frontmatter.
pub fn capture(
    workflow: &Path,
    entity: &Entity,
    provider: SummaryProvider,
) -> Result<SummarySnapshot, SummaryError> {
    let source = entity.worktree_source.as_ref().unwrap_or(&entity.path);
    let selected = read_source(source)?;
    let body = match crate::parser::split_frontmatter(&selected) {
        Some(crate::parser::SplitFrontmatter::Ok { body, .. }) => body,
        _ => return Err(SummaryError::Changed),
    };
    if body != entity.body {
        return Err(SummaryError::Changed);
    }
    let authority_path = entity.evidence_origin.as_ref().unwrap_or(&entity.path);
    let authority_text = if authority_path == source {
        selected.clone()
    } else {
        read_source(authority_path)?
    };
    let current = crate::parser::parse_work_item_contents(
        authority_path,
        &authority_text,
        std::slice::from_ref(&entity.status),
        Some("slug"),
    )
    .map_err(|_| SummaryError::Changed)?;
    if facts(&current) != facts(entity)
        || current.body != *entity.main_body.as_ref().unwrap_or(&entity.body)
    {
        return Err(SummaryError::Changed);
    }
    let authoritative = serde_json::json!({
        "id": entity.id, "title": entity.title, "status": entity.status,
        "verdict": entity.verdict, "started": entity.started, "completed": entity.completed,
        "gates": entity.gates, "gate_readiness": entity.gate_readiness,
        "review_round": entity.review_round,
    });
    let input = serde_json::to_vec(&serde_json::json!({
        "authoritative_recorded_facts": authoritative,
        "selected_source_copy": if entity.worktree_source.is_none() { "main" } else if authority_path != source { "worktree; frontmatter is not authoritative" } else { "worktree-only; recorded facts come from this copy" },
        "selected_source_exact_text": selected,
        "authority_source_exact_text": if authority_path != source { Some(authority_text) } else { None },
        "main_body_when_different": entity.main_body,
    })).map_err(|_| SummaryError::Changed)?;
    if input.len() > prompt::MAX_INPUT {
        return Err(SummaryError::InputLimit);
    }
    Ok(SummarySnapshot {
        key: SummaryKey {
            workflow: workflow.canonicalize().map_err(|_| SummaryError::Changed)?,
            entity: entity.id.clone(),
            source: source.canonicalize().map_err(|_| SummaryError::Changed)?,
            input_digest: digest(&input),
            provider,
            model: provider.model().into(),
            prompt_version: prompt::VERSION.into(),
        },
        input,
    })
}
fn read_source(path: &Path) -> Result<String, SummaryError> {
    // A selected file replaced by a FIFO must not block the UI during capture.
    // Reject final-component symlinks rather than follow a changed source copy.
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| SummaryError::Changed)?;
    let file = std::fs::File::from(fd);
    if !file
        .metadata()
        .map_err(|_| SummaryError::Changed)?
        .is_file()
    {
        return Err(SummaryError::Changed);
    }
    let mut bytes = Vec::new();
    file.take(prompt::MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SummaryError::Changed)?;
    if bytes.len() > prompt::MAX_INPUT {
        return Err(SummaryError::InputLimit);
    }
    String::from_utf8(bytes).map_err(|_| SummaryError::Changed)
}

fn facts(entity: &Entity) -> serde_json::Value {
    serde_json::json!({"id": entity.id, "title": entity.title, "status": entity.status,
        "verdict": entity.verdict, "started": entity.started, "completed": entity.completed,
        "gates": entity.gates, "review_round": entity.review_round, "source": entity.source,
        "score": entity.score, "worktree": entity.worktree, "issue": entity.issue, "pr": entity.pr})
}
