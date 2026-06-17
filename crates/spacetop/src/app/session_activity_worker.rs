use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use spacetop_core::domain::SessionScanReport;
use spacetop_core::session_activity::{
    scan_local_sessions, SessionRoots, SessionScanEntity, SessionScanError, SessionScanRequest,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionActivityWorkerRequest {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub entities: Vec<SessionScanEntity>,
    pub roots: SessionRoots,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionActivityWorkerResult {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub result: Result<SessionScanReport, SessionScanError>,
}

impl SessionActivityWorkerRequest {
    pub fn from_state(
        workflow_dir: &Path,
        repo_root: &Path,
        entities: Vec<SessionScanEntity>,
    ) -> Self {
        Self {
            workflow_dir: workflow_dir.to_path_buf(),
            repo_root: repo_root.to_path_buf(),
            entities,
            roots: SessionRoots::from_env(),
        }
    }
}

pub fn spawn_session_activity_worker(
    request: SessionActivityWorkerRequest,
) -> Receiver<SessionActivityWorkerResult> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let scan_request = SessionScanRequest {
            workflow_dir: request.workflow_dir.clone(),
            repo_root: request.repo_root.clone(),
            entities: request.entities,
            roots: request.roots,
        };
        let result = scan_local_sessions(scan_request);
        let _ = tx.send(SessionActivityWorkerResult {
            workflow_dir: request.workflow_dir,
            repo_root: request.repo_root,
            result,
        });
    });
    rx
}
