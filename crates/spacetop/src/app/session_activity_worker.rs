use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use spacetop_core::domain::SessionScanReport;
use spacetop_core::session_activity::{
    scan_local_sessions_with_snapshots, SessionFileSnapshot, SessionRoots, SessionScanEntity,
    SessionScanError, SessionScanRequest,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionActivityWorkerRequest {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub entities: Vec<SessionScanEntity>,
    pub roots: SessionRoots,
    pub previous_session_files: HashMap<PathBuf, SessionFileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionActivityWorkerResult {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub result: Result<SessionScanReport, SessionScanError>,
    pub session_files: HashMap<PathBuf, SessionFileSnapshot>,
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
            previous_session_files: HashMap::new(),
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
            previous_session_files: request.previous_session_files,
        };
        let result = scan_local_sessions_with_snapshots(
            &scan_request,
            &spacetop_core::session_activity::StdProcessProbe,
            std::time::SystemTime::now(),
        );
        let (result, session_files) = match result {
            Ok(scan) => (Ok(scan.report), scan.session_files),
            Err(err) => (Err(err), HashMap::new()),
        };
        let _ = tx.send(SessionActivityWorkerResult {
            workflow_dir: request.workflow_dir,
            repo_root: request.repo_root,
            result,
            session_files,
        });
    });
    rx
}
