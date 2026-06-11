use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// Result of one git invocation through the [`GitRunner`] seam.
#[derive(Debug, Clone)]
pub struct GitCmdResult {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Abstracts the `git -C {repo_root} {args...}` invocation so git-backed
/// helpers are testable without `git` on `PATH`.
pub trait GitRunner {
    fn run(&self, repo_root: &Path, args: &[&str]) -> io::Result<GitCmdResult>;
}

/// Production [`GitRunner`] that shells out to `git`.
pub struct StdGitRunner;

impl GitRunner for StdGitRunner {
    fn run(&self, repo_root: &Path, args: &[&str]) -> io::Result<GitCmdResult> {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(args)
            .output()?;
        Ok(GitCmdResult {
            status: out.status,
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct GitCall {
    pub repo_root: PathBuf,
    pub args: Vec<String>,
}

#[cfg(test)]
pub struct RecordingGitRunner {
    responses: RefCell<Vec<GitCmdResult>>,
    calls: RefCell<Vec<GitCall>>,
}

#[cfg(test)]
impl RecordingGitRunner {
    pub fn new(responses: Vec<GitCmdResult>) -> Self {
        Self {
            responses: RefCell::new(responses),
            calls: RefCell::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<GitCall> {
        self.calls.borrow().clone()
    }
}

#[cfg(test)]
impl GitRunner for RecordingGitRunner {
    fn run(&self, repo_root: &Path, args: &[&str]) -> io::Result<GitCmdResult> {
        self.calls.borrow_mut().push(GitCall {
            repo_root: repo_root.to_path_buf(),
            args: args.iter().map(|s| s.to_string()).collect(),
        });
        let mut responses = self.responses.borrow_mut();
        if responses.is_empty() {
            return Err(io::Error::other("no more queued responses"));
        }
        Ok(responses.remove(0))
    }
}

#[cfg(test)]
pub fn ok(stdout: &str) -> GitCmdResult {
    use std::os::unix::process::ExitStatusExt;

    GitCmdResult {
        status: ExitStatus::from_raw(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

#[cfg(test)]
pub fn err(code: i32, stderr: &str) -> GitCmdResult {
    use std::os::unix::process::ExitStatusExt;

    GitCmdResult {
        status: ExitStatus::from_raw(code << 8),
        stdout: String::new(),
        stderr: stderr.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn recording_runner_records_calls_in_order() {
        let runner = RecordingGitRunner::new(vec![ok("first\n"), ok("second\n")]);
        let root = PathBuf::from("/tmp/repo");

        let first = runner.run(&root, &["rev-parse", "HEAD"]).expect("first");
        let second = runner
            .run(&root, &["log", "--first-parent"])
            .expect("second");

        assert_eq!(first.stdout, "first\n");
        assert_eq!(second.stdout, "second\n");
        let calls = runner.calls();
        assert_eq!(calls[0].args, ["rev-parse", "HEAD"]);
        assert_eq!(calls[1].args, ["log", "--first-parent"]);
    }
}
