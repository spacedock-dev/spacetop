//! Sync a workflow root from its configured remote.
//!
//! Spacetop is read-only against the workflow tree by default; the one
//! user-initiated exception is the Sync action, which shells out to
//! `git -C {root} pull --ff-only`. All other git invocations from this
//! module are pure read-only probes (`rev-parse`, `remote get-url`,
//! `rev-list --count`).
//!
//! Two seams keep this testable without a real git installed:
//!
//! * [`GitRunner`] — abstracts `git -C {repo_root} {args...}`. The
//!   production [`StdGitRunner`] shells out via [`std::process::Command`].
//!   Tests use [`RecordingGitRunner`] to inject deterministic responses
//!   and observe the exact argv sequence issued.
//!
//! The functional surface is two pure functions:
//!
//! * [`probe_availability`] — three read-only probes that classify the
//!   workflow root as `Available` or one of three `Unavailable` variants.
//! * [`sync`] — calls `probe_availability` first; on `Available`, captures
//!   `HEAD` before the pull, runs `git pull --ff-only`, then computes the
//!   pulled commit count via `rev-list --count {before}..HEAD`.
//!
//! All user-facing strings (`"Syncing…"`, `"Synced (already up to date)"`,
//! `"Synced (N new commits)"`, `"Sync failed: {message}"`, and the three
//! `Unavailable` hints) are pinned by tests below per the project's
//! "stable user-facing strings" convention.

use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// Result of probing the workflow root for sync readiness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAvailability {
    /// The workflow root is a git repo with an upstream-tracked branch
    /// and an `origin` remote.
    Available,
    /// The workflow root cannot be synced; the variant explains why.
    Unavailable(UnavailableReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableReason {
    NotGitRepo,
    NoUpstream,
    NoOriginRemote,
}

impl UnavailableReason {
    /// Short user-facing string for the status pill. Pinned by tests.
    pub fn hint(self) -> &'static str {
        match self {
            UnavailableReason::NotGitRepo => "not a git repository",
            UnavailableReason::NoUpstream => "no upstream for branch",
            UnavailableReason::NoOriginRemote => "no origin remote",
        }
    }
}

/// Outcome of a sync attempt against an available repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// The pull completed with no new commits.
    UpToDate,
    /// The pull fast-forwarded `new_commits` commits onto the working tree.
    Pulled { new_commits: u32 },
    /// The pull failed; `message` is the trimmed stderr (or a synthesized
    /// reason for unavailability).
    Failed { message: String },
}

/// Result of one git invocation through the [`GitRunner`] seam.
#[derive(Debug, Clone)]
pub struct GitCmdResult {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Abstracts the `git -C {repo_root} {args...}` invocation so the sync
/// helper is testable without `git` on `PATH`.
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

/// Probe the three independent prerequisites for syncing, in a fixed
/// order. The probes are pure reads — none of them mutate state.
///
/// Order matters because each later probe's failure mode is only
/// meaningful when the previous ones succeed:
///
/// 1. `rev-parse --is-inside-work-tree` — is this a git repo at all?
/// 2. `rev-parse --abbrev-ref --symbolic-full-name @{u}` — does the
///    current branch track an upstream?
/// 3. `remote get-url origin` — is there an `origin` remote? Probed
///    last because most repos with an upstream also have `origin`, but
///    a user on an orphan/detached branch might still benefit from the
///    guidance.
pub fn probe_availability<R: GitRunner>(runner: &R, repo_root: &Path) -> SyncAvailability {
    match runner.run(repo_root, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(out) if out.status.success() && out.stdout.trim() == "true" => {}
        _ => return SyncAvailability::Unavailable(UnavailableReason::NotGitRepo),
    }
    match runner.run(
        repo_root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    ) {
        Ok(out) if out.status.success() => {}
        _ => return SyncAvailability::Unavailable(UnavailableReason::NoUpstream),
    }
    match runner.run(repo_root, &["remote", "get-url", "origin"]) {
        Ok(out) if out.status.success() => {}
        _ => return SyncAvailability::Unavailable(UnavailableReason::NoOriginRemote),
    }
    SyncAvailability::Available
}

/// Sync the workflow root: probe, then `git pull --ff-only`, then count
/// any new commits. Never invokes `git push`, `git commit`, or `git
/// checkout`.
pub fn sync<R: GitRunner>(runner: &R, repo_root: &Path) -> SyncOutcome {
    match probe_availability(runner, repo_root) {
        SyncAvailability::Available => {}
        SyncAvailability::Unavailable(reason) => {
            return SyncOutcome::Failed {
                message: reason.hint().to_string(),
            };
        }
    }

    // Capture HEAD before the pull so we can count commits accurately
    // regardless of how `git pull` formats its stdout across versions.
    let head_before = match runner.run(repo_root, &["rev-parse", "HEAD"]) {
        Ok(out) if out.status.success() => out.stdout.trim().to_string(),
        _ => String::new(),
    };

    let pull = match runner.run(repo_root, &["pull", "--ff-only"]) {
        Ok(out) => out,
        Err(err) => {
            return SyncOutcome::Failed {
                message: err.to_string(),
            };
        }
    };

    if !pull.status.success() {
        let message = first_nonempty_line(&pull.stderr)
            .or_else(|| first_nonempty_line(&pull.stdout))
            .unwrap_or_else(|| "git pull failed".to_string());
        return SyncOutcome::Failed { message };
    }

    if head_before.is_empty() {
        // We couldn't capture HEAD before; fall back to the stdout hint.
        if pull.stdout.contains("Already up to date") {
            return SyncOutcome::UpToDate;
        }
        return SyncOutcome::Pulled { new_commits: 0 };
    }

    let head_after = match runner.run(repo_root, &["rev-parse", "HEAD"]) {
        Ok(out) if out.status.success() => out.stdout.trim().to_string(),
        _ => return SyncOutcome::UpToDate,
    };

    if head_after == head_before {
        return SyncOutcome::UpToDate;
    }

    let range = format!("{head_before}..{head_after}");
    let count = match runner.run(repo_root, &["rev-list", "--count", &range]) {
        Ok(out) if out.status.success() => out.stdout.trim().parse::<u32>().unwrap_or(0),
        _ => 0,
    };
    SyncOutcome::Pulled { new_commits: count }
}

/// Return the first non-empty trimmed line of `s`, useful for surfacing
/// a one-line summary of multi-line git error output in the status pill.
fn first_nonempty_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Shared test seam: a `RecordingGitRunner` that returns a queue of
    //! pre-canned [`GitCmdResult`]s in FIFO order and records every
    //! invocation's argv. Exposed at crate-visibility so integration
    //! tests under `tests/` can reuse the same double without
    //! re-implementing it.
    use std::cell::RefCell;
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::path::{Path, PathBuf};
    use std::process::ExitStatus;

    use super::{GitCmdResult, GitRunner};

    #[derive(Debug, Clone)]
    pub struct GitCall {
        #[allow(dead_code)] // exposed for future tests asserting repo-root targeting
        pub repo_root: PathBuf,
        pub args: Vec<String>,
    }

    pub struct RecordingGitRunner {
        responses: RefCell<Vec<GitCmdResult>>,
        calls: RefCell<Vec<GitCall>>,
    }

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

    impl GitRunner for RecordingGitRunner {
        fn run(&self, repo_root: &Path, args: &[&str]) -> io::Result<GitCmdResult> {
            self.calls.borrow_mut().push(GitCall {
                repo_root: repo_root.to_path_buf(),
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            let mut q = self.responses.borrow_mut();
            if q.is_empty() {
                return Err(io::Error::other("no more queued responses"));
            }
            Ok(q.remove(0))
        }
    }

    pub fn ok(stdout: &str) -> GitCmdResult {
        GitCmdResult {
            status: ExitStatus::from_raw(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    pub fn err(code: i32, stderr: &str) -> GitCmdResult {
        // `from_raw` interprets its argument as a wait status word; on
        // Linux the exit code lives in bits 8..15 (`code << 8`).
        GitCmdResult {
            status: ExitStatus::from_raw(code << 8),
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{err, ok, RecordingGitRunner};
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from("/tmp/spacetop-sync-test")
    }

    #[test]
    fn probe_available_runs_three_probes_in_order() {
        let runner = RecordingGitRunner::new(vec![
            ok("true\n"),
            ok("origin/main\n"),
            ok("git@github.com:foo/bar.git\n"),
        ]);
        let result = probe_availability(&runner, &root());
        assert_eq!(result, SyncAvailability::Available);
        let calls = runner.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].args, vec!["rev-parse", "--is-inside-work-tree"]);
        assert_eq!(
            calls[1].args,
            vec!["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]
        );
        assert_eq!(calls[2].args, vec!["remote", "get-url", "origin"]);
    }

    #[test]
    fn probe_not_a_git_repo_short_circuits() {
        let runner = RecordingGitRunner::new(vec![err(128, "fatal: not a git repository")]);
        let result = probe_availability(&runner, &root());
        assert_eq!(
            result,
            SyncAvailability::Unavailable(UnavailableReason::NotGitRepo)
        );
        assert_eq!(runner.calls().len(), 1, "should not probe further");
    }

    #[test]
    fn probe_no_upstream_returns_no_upstream() {
        let runner = RecordingGitRunner::new(vec![
            ok("true\n"),
            err(128, "fatal: no upstream configured"),
        ]);
        let result = probe_availability(&runner, &root());
        assert_eq!(
            result,
            SyncAvailability::Unavailable(UnavailableReason::NoUpstream)
        );
        assert_eq!(runner.calls().len(), 2, "should not probe origin remote");
    }

    #[test]
    fn probe_no_origin_remote_returns_no_origin() {
        let runner = RecordingGitRunner::new(vec![
            ok("true\n"),
            ok("upstream/main\n"),
            err(2, "error: No such remote 'origin'"),
        ]);
        let result = probe_availability(&runner, &root());
        assert_eq!(
            result,
            SyncAvailability::Unavailable(UnavailableReason::NoOriginRemote)
        );
        assert_eq!(runner.calls().len(), 3);
    }

    #[test]
    fn unavailable_hint_strings_are_stable() {
        // Pinned user-facing strings; update tests and the UI together.
        assert_eq!(UnavailableReason::NotGitRepo.hint(), "not a git repository");
        assert_eq!(
            UnavailableReason::NoUpstream.hint(),
            "no upstream for branch"
        );
        assert_eq!(UnavailableReason::NoOriginRemote.hint(), "no origin remote");
    }

    #[test]
    fn sync_already_up_to_date_returns_up_to_date() {
        let runner = RecordingGitRunner::new(vec![
            // probe
            ok("true\n"),
            ok("origin/main\n"),
            ok("git@github.com:foo/bar.git\n"),
            // HEAD before
            ok("abc123\n"),
            // pull --ff-only
            ok("Already up to date.\n"),
            // HEAD after
            ok("abc123\n"),
        ]);
        let outcome = sync(&runner, &root());
        assert_eq!(outcome, SyncOutcome::UpToDate);
    }

    #[test]
    fn sync_fast_forward_returns_pulled_with_commit_count() {
        let runner = RecordingGitRunner::new(vec![
            ok("true\n"),
            ok("origin/main\n"),
            ok("git@github.com:foo/bar.git\n"),
            ok("abc123\n"),
            ok("Updating abc123..def456\nFast-forward\n .../foo | 2 +-\n"),
            ok("def456\n"),
            ok("3\n"),
        ]);
        let outcome = sync(&runner, &root());
        assert_eq!(outcome, SyncOutcome::Pulled { new_commits: 3 });
        // Ensure the rev-list call used the before..after range form.
        let calls = runner.calls();
        let last = calls.last().expect("at least one call");
        assert_eq!(last.args, vec!["rev-list", "--count", "abc123..def456"]);
    }

    #[test]
    fn sync_failed_pull_carries_first_stderr_line() {
        let runner = RecordingGitRunner::new(vec![
            ok("true\n"),
            ok("origin/main\n"),
            ok("git@github.com:foo/bar.git\n"),
            ok("abc123\n"),
            err(
                1,
                "fatal: unable to access 'https://example.invalid/': could not resolve host\n",
            ),
        ]);
        let outcome = sync(&runner, &root());
        match outcome {
            SyncOutcome::Failed { message } => {
                assert!(
                    message.starts_with("fatal: unable to access"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn sync_unavailable_short_circuits_without_invoking_pull() {
        let runner = RecordingGitRunner::new(vec![err(
            128,
            "fatal: not a git repository (or any parent)",
        )]);
        let outcome = sync(&runner, &root());
        assert_eq!(
            outcome,
            SyncOutcome::Failed {
                message: "not a git repository".to_string()
            }
        );
        let calls = runner.calls();
        assert_eq!(
            calls.len(),
            1,
            "sync must not invoke any further git commands when unavailable"
        );
        assert!(!calls.iter().any(|c| c.args.contains(&"pull".to_string())));
    }
}
