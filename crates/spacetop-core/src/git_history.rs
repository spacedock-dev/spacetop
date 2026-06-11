use std::collections::HashMap;
use std::path::Path;

use serde_yaml::Value;

use crate::git::GitRunner;
use crate::index::{CommitId, CommitTime, StageEvent};
use crate::query::{HistoryResult, HistoryUnavailable};

pub struct GitHistorySource<'a, R> {
    runner: &'a R,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntityMetadata {
    id: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommitTouch {
    commit: String,
    unix_time: i64,
    path: String,
    archive_rename: bool,
}

impl<'a, R: GitRunner> GitHistorySource<'a, R> {
    pub fn new(runner: &'a R) -> Self {
        Self { runner }
    }

    pub fn load(&self, repo_root: &Path, workflow_rel: &str) -> HistoryResult<Vec<StageEvent>> {
        self.ensure_not_shallow(repo_root)?;
        let pathspec = if workflow_rel.is_empty() || workflow_rel == "." {
            "**".to_string()
        } else {
            format!("{workflow_rel}/**")
        };
        let out = self
            .runner
            .run(
                repo_root,
                &[
                    "log",
                    "--first-parent",
                    "--reverse",
                    "--date=unix",
                    "--pretty=format:%H%x00%ct",
                    "--name-status",
                    "-M",
                    "--",
                    &pathspec,
                ],
            )
            .map_err(|err| HistoryUnavailable::GitError(err.to_string()))?;
        if !out.status.success() {
            return Err(HistoryUnavailable::GitError(out.stderr));
        }

        self.events_from_log(repo_root, workflow_rel, &out.stdout)
    }

    fn ensure_not_shallow(&self, repo_root: &Path) -> HistoryResult<()> {
        let out = self
            .runner
            .run(repo_root, &["rev-parse", "--is-shallow-repository"])
            .map_err(|err| HistoryUnavailable::GitError(err.to_string()))?;
        if !out.status.success() && out.stderr.to_lowercase().contains("not a git repository") {
            return Err(HistoryUnavailable::NotGitRepository);
        }
        if !out.status.success() {
            return Err(HistoryUnavailable::GitError(out.stderr));
        }
        if out.stdout.trim() == "true" {
            return Err(HistoryUnavailable::ShallowClone);
        }
        Ok(())
    }

    fn events_from_log(
        &self,
        repo_root: &Path,
        workflow_rel: &str,
        log: &str,
    ) -> HistoryResult<Vec<StageEvent>> {
        let mut last_status_by_id: HashMap<String, String> = HashMap::new();
        let mut events = Vec::new();

        for touch in parse_touches(workflow_rel, log) {
            let metadata = self.blob_metadata(repo_root, &touch.commit, &touch.path)?;
            if touch.archive_rename {
                if last_status_by_id
                    .get(&metadata.id)
                    .is_some_and(|status| status == "done")
                {
                    continue;
                }
                let from = last_status_by_id.insert(metadata.id.clone(), "done".to_string());
                events.push(StageEvent {
                    entity_id: metadata.id,
                    from,
                    to: "done".to_string(),
                    at: CommitTime(touch.unix_time),
                    commit: CommitId(touch.commit),
                });
                continue;
            }

            let prior = last_status_by_id.get(&metadata.id).cloned();
            if prior.as_deref() == Some(metadata.status.as_str()) {
                continue;
            }
            last_status_by_id.insert(metadata.id.clone(), metadata.status.clone());
            events.push(StageEvent {
                entity_id: metadata.id,
                from: prior,
                to: metadata.status,
                at: CommitTime(touch.unix_time),
                commit: CommitId(touch.commit),
            });
        }

        Ok(events)
    }

    fn blob_metadata(
        &self,
        repo_root: &Path,
        commit: &str,
        path: &str,
    ) -> HistoryResult<EntityMetadata> {
        let spec = format!("{commit}:{path}");
        let blob = self
            .runner
            .run(repo_root, &["show", &spec])
            .map_err(|err| HistoryUnavailable::GitError(err.to_string()))?;
        if !blob.status.success() {
            return Err(HistoryUnavailable::GitError(blob.stderr));
        }
        frontmatter_metadata(&blob.stdout).ok_or_else(|| {
            HistoryUnavailable::GitError(format!("missing entity frontmatter in {path}"))
        })
    }
}

fn parse_touches(workflow_rel: &str, log: &str) -> Vec<CommitTouch> {
    let mut touches = Vec::new();
    let mut current_commit: Option<String> = None;
    let mut current_time: Option<i64> = None;

    for line in log.lines().filter(|line| !line.trim().is_empty()) {
        if let Some((commit, unix_time)) = parse_commit_header(line) {
            current_commit = Some(commit.to_string());
            current_time = Some(unix_time);
            continue;
        }

        let (Some(commit), Some(unix_time)) = (&current_commit, current_time) else {
            continue;
        };
        let columns: Vec<&str> = line.split('\t').collect();
        let Some(kind) = columns.first().copied() else {
            continue;
        };

        if kind.starts_with('R') {
            let (Some(old_path), Some(new_path)) = (columns.get(1), columns.get(2)) else {
                continue;
            };
            if !is_entity_path(workflow_rel, old_path) || !is_entity_path(workflow_rel, new_path) {
                continue;
            }
            touches.push(CommitTouch {
                commit: commit.clone(),
                unix_time,
                path: (*new_path).to_string(),
                archive_rename: is_archive_path(workflow_rel, new_path),
            });
            continue;
        }

        let path = if kind.starts_with('C') {
            columns.get(2).copied()
        } else {
            columns.get(1).copied()
        };
        let Some(path) = path else {
            continue;
        };
        if kind == "D" || !is_entity_path(workflow_rel, path) {
            continue;
        }
        touches.push(CommitTouch {
            commit: commit.clone(),
            unix_time,
            path: path.to_string(),
            archive_rename: false,
        });
    }

    touches
}

fn parse_commit_header(line: &str) -> Option<(&str, i64)> {
    let (commit, unix_time) = line.split_once('\0')?;
    Some((commit, unix_time.parse().ok()?))
}

fn is_entity_path(workflow_rel: &str, path: &str) -> bool {
    let Some(rel) = workflow_item_rel(workflow_rel, path) else {
        return false;
    };
    if rel == "README.md" || rel.starts_with("_mods/") || !rel.ends_with(".md") {
        return false;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        [file] => *file != "README.md",
        [_slug, "index.md"] => true,
        ["_archive", file] => *file != "README.md",
        ["_archive", _slug, "index.md"] => true,
        _ => false,
    }
}

fn is_archive_path(workflow_rel: &str, path: &str) -> bool {
    workflow_item_rel(workflow_rel, path).is_some_and(|rel| rel.starts_with("_archive/"))
}

fn workflow_item_rel<'a>(workflow_rel: &str, path: &'a str) -> Option<&'a str> {
    if workflow_rel.is_empty() || workflow_rel == "." {
        return Some(path);
    }
    path.strip_prefix(workflow_rel)
        .and_then(|path| path.strip_prefix('/'))
}

fn frontmatter_metadata(body: &str) -> Option<EntityMetadata> {
    let mut lines = body.lines();
    if lines.next()? != "---" {
        return None;
    }
    let mut yaml = String::new();
    for line in lines {
        if line == "---" {
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    let value: Value = serde_yaml::from_str(&yaml).ok()?;
    Some(EntityMetadata {
        id: value.get("id")?.as_str()?.to_string(),
        status: value.get("status")?.as_str()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{err, ok, RecordingGitRunner};
    use crate::query::HistoryUnavailable;
    use std::path::PathBuf;

    #[test]
    fn shallow_repository_refuses_history() {
        let runner = RecordingGitRunner::new(vec![ok("true\n")]);
        let result = GitHistorySource::new(&runner).load(&PathBuf::from("/repo"), "docs/workflow");
        assert_eq!(result.unwrap_err(), HistoryUnavailable::ShallowClone);
    }

    #[test]
    fn non_git_repository_is_unavailable_without_metrics() {
        let runner = RecordingGitRunner::new(vec![err(128, "fatal: not a git repository\n")]);
        let result = GitHistorySource::new(&runner).load(&PathBuf::from("/repo"), "docs/workflow");
        assert_eq!(result.unwrap_err(), HistoryUnavailable::NotGitRepository);
    }

    #[test]
    fn non_shallow_repository_runs_first_parent_log() {
        let runner = RecordingGitRunner::new(vec![ok("false\n"), ok("")]);
        let _ = GitHistorySource::new(&runner)
            .load(&PathBuf::from("/repo"), "docs/workflow")
            .expect("history load");
        let calls = runner.calls();
        assert_eq!(calls[0].args, ["rev-parse", "--is-shallow-repository"]);
        assert!(
            calls[1].args.contains(&"--first-parent".to_string()),
            "history log must use --first-parent: {:?}",
            calls[1].args
        );
    }

    #[test]
    fn history_source_uses_only_approved_read_commands() {
        let runner = RecordingGitRunner::new(vec![ok("false\n"), ok("")]);
        let _ = GitHistorySource::new(&runner)
            .load(&PathBuf::from("/repo"), "docs/workflow")
            .expect("history");

        let allowed = ["rev-parse", "rev-list", "log", "show"];
        for call in runner.calls() {
            let command = call.args.first().expect("git command");
            assert!(
                allowed.contains(&command.as_str()),
                "history command must be read-only, got {:?}",
                call.args
            );
        }
    }

    #[test]
    fn parse_touches_ignores_non_entity_paths() {
        let log = "abc\x00100\nM\tdocs/workflow/README.md\nM\tdocs/workflow/_mods/mod.md\n";
        assert!(parse_touches("docs/workflow", log).is_empty());
    }
}
