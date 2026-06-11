use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use spacetop_core::git::StdGitRunner;
use spacetop_core::git_history::GitHistorySource;
use spacetop_core::index::StageEvent;
use spacetop_core::query::HistoryResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryWorkerRequest {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub workflow_rel: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryWorkerResult {
    pub workflow_dir: PathBuf,
    pub result: HistoryResult<Vec<StageEvent>>,
}

impl HistoryWorkerRequest {
    pub fn from_paths(workflow_dir: &Path, repo_root: &Path) -> Option<Self> {
        let rel = workflow_dir.strip_prefix(repo_root).ok()?;
        Some(Self {
            workflow_dir: workflow_dir.to_path_buf(),
            repo_root: repo_root.to_path_buf(),
            workflow_rel: path_to_git_rel(rel),
        })
    }
}

pub fn spawn_history_worker(request: HistoryWorkerRequest) -> Receiver<HistoryWorkerResult> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result =
            GitHistorySource::new(&StdGitRunner).load(&request.repo_root, &request.workflow_rel);
        let _ = tx.send(HistoryWorkerResult {
            workflow_dir: request.workflow_dir,
            result,
        });
    });
    rx
}

fn path_to_git_rel(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
