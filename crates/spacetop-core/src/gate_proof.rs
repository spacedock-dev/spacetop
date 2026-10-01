//! Narrow read-only promotion proof. Only the physical authority source counts.
use crate::domain::{
    Entity, GatePreparation, StageDefinition, StateCheckoutDisposition, WorkflowStorage,
};
use crate::git::{GitRunner, StdGitRunner};
use std::path::Path;

pub fn preparation(
    entity: &Entity,
    stages: &[StageDefinition],
    storage: &WorkflowStorage,
) -> GatePreparation {
    let Some(stage) = stages
        .iter()
        .find(|s| s.name == entity.status && s.gate && !s.terminal)
    else {
        return GatePreparation::default();
    };
    if crate::gates::selected_attempt(&entity.gates, &entity.status).is_some()
        || matches!(entity.gates, crate::domain::GateData::Invalid { .. })
    {
        return GatePreparation::default();
    }
    if matches!(
        storage,
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::Unverified { .. },
            ..
        }
    ) {
        return GatePreparation {
            proven: false,
            diagnostics: vec![
                "state checkout topology is unverified; preparation Git probes skipped".into(),
            ],
        };
    }
    prove(&entity.path, &entity.body, stage, &StdGitRunner)
}
pub fn prove<R: GitRunner>(
    path: &Path,
    body: &str,
    stage: &StageDefinition,
    runner: &R,
) -> GatePreparation {
    let result = if !stage.initial && !complete_stage_report(body, &stage.name) {
        Err("latest exact-stage report is missing or incomplete".to_string())
    } else {
        clean_path(path, runner)
    };
    match result {
        Ok(()) => GatePreparation {
            proven: true,
            diagnostics: Vec::new(),
        },
        Err(error) => GatePreparation {
            proven: false,
            diagnostics: vec![error],
        },
    }
}
fn clean_path<R: GitRunner>(path: &Path, runner: &R) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("entity path unreadable: {e}"))?;
    let parent = path.parent().ok_or("entity has no parent")?;
    let top = runner
        .run(parent, &["rev-parse", "--show-toplevel"])
        .map_err(|e| format!("Git probe failed: {e}"))?;
    if !top.status.success() || top.stdout.trim().is_empty() {
        return Err("entity is not in a verified Git checkout".into());
    }
    let root = Path::new(top.stdout.trim())
        .canonicalize()
        .map_err(|e| format!("Git root unreadable: {e}"))?;
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| "entity is outside Git root")?
        .to_str()
        .ok_or("entity path is not UTF-8")?;
    let tracked = runner
        .run(
            &root,
            &[
                "--literal-pathspecs",
                "ls-files",
                "--error-unmatch",
                "--",
                relative,
            ],
        )
        .map_err(|e| format!("Git tracking probe failed: {e}"))?;
    if !tracked.status.success() {
        return Err("entity is untracked".into());
    }
    let clean = runner
        .run(
            &root,
            &[
                "--literal-pathspecs",
                "diff",
                "--quiet",
                "HEAD",
                "--",
                relative,
            ],
        )
        .map_err(|e| format!("Git cleanliness probe failed: {e}"))?;
    if !clean.status.success() {
        return Err("entity differs from local HEAD or HEAD could not be read".into());
    }
    Ok(())
}
/// Latest exact stage token and checklist boundaries match upstream gate_extract.
pub fn complete_stage_report(body: &str, stage: &str) -> bool {
    let lines: Vec<_> = body.lines().collect();
    let chosen = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| report_token(line) == Some(stage))
        .map(|(i, _)| i)
        .next_back();
    let Some(start) = chosen else {
        return false;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.starts_with("## "))
        .map_or(lines.len(), |(i, _)| i);
    let section = &lines[start + 1..end];
    let limit = section
        .iter()
        .position(|line| line.starts_with("### "))
        .unwrap_or(section.len());
    let items: Vec<_> = section[..limit]
        .iter()
        .enumerate()
        .filter_map(|(i, line)| checklist(line).map(|(status, text)| (i, status, text)))
        .collect();
    if items.is_empty() {
        return false;
    }
    for (n, (i, status, text)) in items.iter().enumerate() {
        let end = items.get(n + 1).map_or(limit, |(i, _, _)| *i);
        if *status == "FAILED"
            || text.trim().is_empty()
            || !section[i + 1..end]
                .iter()
                .any(|line| !line.trim().is_empty())
        {
            return false;
        }
    }
    let Some(summary) = section.iter().position(|line| line.trim() == "### Summary") else {
        return false;
    };
    section[summary + 1..]
        .iter()
        .take_while(|line| !heading_at_most_three(line))
        .any(|line| !line.trim().is_empty())
}
fn report_token(line: &str) -> Option<&str> {
    let tail = line.strip_prefix("##")?;
    if !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let tail = tail.trim_start().strip_prefix("Stage Report:")?;
    if !tail.starts_with(char::is_whitespace) {
        return None;
    }
    tail.split_whitespace().next()
}
fn checklist(line: &str) -> Option<(&str, &str)> {
    let tail = line.strip_prefix('-')?;
    if !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let (status, text) = tail.trim_start().split_once(':')?;
    matches!(status, "DONE" | "SKIPPED" | "FAILED").then_some((status, text))
}
fn heading_at_most_three(line: &str) -> bool {
    let line = line.trim_start_matches(' ');
    let level = line.chars().take_while(|c| *c == '#').count();
    (1..=3).contains(&level) && line[level..].starts_with(char::is_whitespace)
}
