# SpaceTop v2 - Phase P2: Git History Source Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement trustworthy per-entity stage history and metrics from git history, including archive-move `done` synthesis, shallow-clone refusal, and read-only git guardrails.

**Architecture:** This phase builds on P1's `WorkflowIndex` and query API. Git history lives entirely in `spacetop-core`, behind a testable `GitRunner` seam, and is loaded into the index as `StageEvent`s. The TUI remains unchanged except for rendering "unavailable" states that now carry real reasons.

**Tech Stack:** Rust 2021, `std::process::Command`, existing `GitRunner` pattern from `git_sync`, `tempfile` fixture repos, `serde`, core-only tests.

---

## Prerequisites

- P0 and P1 are merged.
- `WorkflowIndex::timeline`, `metrics`, and `activity` currently return `HistoryUnavailable::NotImplemented`.
- `crates/spacetop-core/tests/no_write_git_calls.rs` scans both crate source trees.

## Hard constraints

- **Only read-only git commands.** P2 history may invoke only `rev-parse`, `rev-list`, `log`, and `show`.
- **No wrong metrics.** Shallow clone or unparseable history returns unavailable. Do not compute dwell/cycle from incomplete history.
- **First-parent only.** Stage timing is based on first-parent mainline history.
- **Frontmatter only.** Ignore `status:` strings in README files, bodies, fixtures, or acceptance criteria text.
- **Archive done synthesis.** Terminal `done` comes from rename into `_archive/`, not frontmatter.
- **No async runtime.** History loading may be synchronous in core tests and headless commands, but TUI history ingestion must run on a background thread and fold results back through an `mpsc` channel so startup/reload does not block the render/input loop.

## File map

- Create: `crates/spacetop-core/src/git.rs`
- Create: `crates/spacetop-core/src/git_history.rs`
- Create: `crates/spacetop-core/src/metrics.rs`
- Create: `crates/spacetop-core/tests/git_history_fixtures.rs`
- Modify: `crates/spacetop-core/src/git_sync.rs`
- Modify: `crates/spacetop-core/src/index.rs`
- Modify: `crates/spacetop-core/src/query.rs`
- Modify: `crates/spacetop/src/app/overview.rs`
- Create: `crates/spacetop/src/app/history_worker.rs`
- Modify: `crates/spacetop-core/src/lib.rs`
- Modify: `crates/spacetop-core/tests/no_write_git_calls.rs`

---

## Task 1: Extract shared read-only git runner seam

**Files:**
- Create: `crates/spacetop-core/src/git.rs`
- Modify: `crates/spacetop-core/src/git_sync.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write git seam tests**

Create `crates/spacetop-core/src/git.rs`:

```rust
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn recording_runner_records_calls_in_order() {
        let runner = RecordingGitRunner::new(vec![ok("first\n"), ok("second\n")]);
        let root = PathBuf::from("/tmp/repo");

        let first = runner.run(&root, &["rev-parse", "HEAD"]).expect("first");
        let second = runner.run(&root, &["log", "--first-parent"]).expect("second");

        assert_eq!(first.stdout, "first\n");
        assert_eq!(second.stdout, "second\n");
        let calls = runner.calls();
        assert_eq!(calls[0].args, ["rev-parse", "HEAD"]);
        assert_eq!(calls[1].args, ["log", "--first-parent"]);
    }
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test -p spacetop-core git::tests`

Expected: FAIL because the shared git seam is not implemented.

- [ ] **Step 3: Move the seam out of `git_sync`**

Move `GitCmdResult`, `GitRunner`, `StdGitRunner`, and the test support runner from `git_sync.rs` into `git.rs`:

```rust
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone)]
pub struct GitCmdResult {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

pub trait GitRunner {
    fn run(&self, repo_root: &Path, args: &[&str]) -> io::Result<GitCmdResult>;
}

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
        let mut q = self.responses.borrow_mut();
        if q.is_empty() {
            return Err(io::Error::other("no more queued responses"));
        }
        Ok(q.remove(0))
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
```

- [ ] **Step 4: Update `git_sync` imports**

In `git_sync.rs`, remove local seam definitions and add:

```rust
pub use crate::git::{GitCmdResult, GitRunner, StdGitRunner};
```

Update tests to import `crate::git::{err, ok, RecordingGitRunner}`.

- [ ] **Step 5: Export and verify**

In `lib.rs`, add:

```rust
pub mod git;
```

Run:

```bash
cargo test -p spacetop-core git::tests git_sync
```

Expected: PASS.

```bash
git add crates/spacetop-core/src/git.rs crates/spacetop-core/src/git_sync.rs crates/spacetop-core/src/lib.rs
git commit -m "refactor(core): share git runner seam"
```

---

## Task 2: Add git history model and shallow clone guard

**Files:**
- Create: `crates/spacetop-core/src/git_history.rs`
- Modify: `crates/spacetop-core/src/lib.rs`
- Modify: `crates/spacetop-core/src/query.rs`

- [ ] **Step 1: Write shallow guard tests**

Create `crates/spacetop-core/src/git_history.rs`:

```rust
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
        let runner = RecordingGitRunner::new(vec![
            ok("false\n"),
            ok(""),
        ]);
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
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test -p spacetop-core git_history::tests`

Expected: FAIL because `GitHistorySource` does not exist.

- [ ] **Step 3: Implement the first shell of `GitHistorySource`**

Add above the tests:

```rust
use std::path::Path;

use crate::git::GitRunner;
use crate::index::StageEvent;
use crate::query::{HistoryResult, HistoryUnavailable};

pub struct GitHistorySource<'a, R> {
    runner: &'a R,
}

impl<'a, R: GitRunner> GitHistorySource<'a, R> {
    pub fn new(runner: &'a R) -> Self {
        Self { runner }
    }

    pub fn load(&self, repo_root: &Path, workflow_rel: &str) -> HistoryResult<Vec<StageEvent>> {
        self.ensure_not_shallow(repo_root)?;
        let pathspec = format!("{workflow_rel}/**");
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
        Ok(Vec::new())
    }

    fn ensure_not_shallow(&self, repo_root: &Path) -> HistoryResult<()> {
        let out = self
            .runner
            .run(repo_root, &["rev-parse", "--is-shallow-repository"])
            .map_err(|err| HistoryUnavailable::GitError(err.to_string()))?;
        if !out.status.success()
            && out.stderr.to_lowercase().contains("not a git repository")
        {
            return Err(HistoryUnavailable::NotGitRepository);
        }
        if !out.status.success() {
            return Err(HistoryUnavailable::GitError(out.stderr));
        }
        if out.status.success() && out.stdout.trim() == "true" {
            return Err(HistoryUnavailable::ShallowClone);
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Export the module**

In `lib.rs`, add:

```rust
pub mod git_history;
```

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop-core git_history::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/git_history.rs crates/spacetop-core/src/lib.rs
git commit -m "feat(core): add git history source shell and shallow guard"
```

---

## Task 3: Build fixture repos for verified failure modes

**Files:**
- Create: `crates/spacetop-core/tests/git_history_fixtures.rs`

- [ ] **Step 1: Add fixture builder helpers**

Create `crates/spacetop-core/tests/git_history_fixtures.rs` and add a real git fixture builder there. Keep these helpers out of `src/` because `no_write_git_calls.rs` statically scans source files for write-subcommand literals:

```rust
use std::fs;
use std::process::Command;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct RepoFixture {
    _tmp: TempDir,
    root: PathBuf,
    workflow: PathBuf,
}

impl RepoFixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        run_git(&root, &["init"]);
        run_git(&root, &["config", "user.email", "spacetop@example.test"]);
        run_git(&root, &["config", "user.name", "Spacetop Test"]);
        let workflow = root.join("docs/workflow");
        fs::create_dir_all(&workflow).expect("workflow dir");
        fs::write(
            workflow.join("README.md"),
            "---\ncommissioned-by: spacedock@test\n---\n",
        )
        .expect("readme");
        Self {
            _tmp: tmp,
            root,
            workflow,
        }
    }

    fn write_entity(&self, rel: &str, status: &str, body: &str) {
        let path = self.workflow.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("entity parent");
        }
        fs::write(
            path,
            format!("---\nid: test\nstatus: {status}\ntitle: Test\n---\n{body}\n"),
        )
        .expect("entity write");
    }

    fn commit(&self, message: &str) {
        run_git(&self.root, &["add", "."]);
        run_git(&self.root, &["commit", "-m", message]);
    }
}

fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success(), "git {:?} failed", args);
}
```

- [ ] **Step 2: Add fixture smoke test**

Add:

```rust
#[test]
fn fixture_repo_can_make_status_commits() {
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");

    let out = Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("rev-list");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "2");
}
```

- [ ] **Step 3: Run the fixture smoke test**

Run: `cargo test -p spacetop-core --test git_history_fixtures fixture_repo_can_make_status_commits`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/spacetop-core/tests/git_history_fixtures.rs
git commit -m "test(core): add git history fixture repo builder"
```

---

## Task 4: Parse frontmatter status changes only from entity files

**Files:**
- Modify: `crates/spacetop-core/src/git_history.rs`
- Modify: `crates/spacetop-core/tests/git_history_fixtures.rs`

- [ ] **Step 1: Add body-decoy test**

Add to `crates/spacetop-core/tests/git_history_fixtures.rs`:

```rust
#[test]
fn body_status_decoy_does_not_create_stage_event() {
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body mentions status: verify");
    repo.commit("plan");
    repo.write_entity("001.md", "plan", "body now mentions status: done");
    repo.commit("body only");

    let events = spacetop_core::git_history::GitHistorySource::new(
        &spacetop_core::git::StdGitRunner,
    )
        .load(&repo.root, "docs/workflow")
        .expect("history");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from, None);
    assert_eq!(events[0].to, "plan");
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core --test git_history_fixtures body_status_decoy_does_not_create_stage_event`

Expected: FAIL because events are still empty.

- [ ] **Step 3: Implement commit parsing and blob frontmatter extraction**

In `git_history.rs`, add helper structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct Touch {
    commit: String,
    unix_time: i64,
    status: String,
    path: String,
}
```

Implement:

```rust
fn is_entity_path(workflow_rel: &str, path: &str) -> bool {
    let Some(rel) = path.strip_prefix(workflow_rel).and_then(|p| p.strip_prefix('/')) else {
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

fn frontmatter_status(body: &str) -> Option<String> {
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
    let value: serde_yaml::Value = serde_yaml::from_str(&yaml).ok()?;
    value
        .get("status")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}
```

When processing each commit/path from `git log --name-status`, call:

```rust
let spec = format!("{commit}:{path}");
let blob = self.runner.run(repo_root, &["show", &spec])?;
```

Then parse only `frontmatter_status(&blob.stdout)`. Build a single initial `StageEvent` per entity when the first seen status appears, and later events only when the frontmatter status changes.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop-core --test git_history_fixtures body_status_decoy_does_not_create_stage_event`

Expected: PASS.

```bash
git add crates/spacetop-core/src/git_history.rs crates/spacetop-core/tests/git_history_fixtures.rs
git commit -m "feat(core): derive stage events from entity frontmatter only"
```

---

## Task 5: Synthesize terminal done from archive rename

**Files:**
- Modify: `crates/spacetop-core/src/git_history.rs`
- Modify: `crates/spacetop-core/tests/git_history_fixtures.rs`

- [ ] **Step 1: Add archive rename test**

Add to `crates/spacetop-core/tests/git_history_fixtures.rs`:

```rust
#[test]
fn archive_rename_synthesizes_done_event() {
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");
    fs::create_dir_all(repo.workflow.join("_archive")).expect("archive dir");
    fs::rename(repo.workflow.join("001.md"), repo.workflow.join("_archive/001.md"))
        .expect("archive rename");
    repo.commit("archive");

    let events = spacetop_core::git_history::GitHistorySource::new(
        &spacetop_core::git::StdGitRunner,
    )
        .load(&repo.root, "docs/workflow")
        .expect("history");

    let transitions: Vec<(Option<String>, String)> =
        events.into_iter().map(|e| (e.from, e.to)).collect();
    assert_eq!(
        transitions,
        [
            (None, "plan".to_string()),
            (Some("plan".to_string()), "verify".to_string()),
            (Some("verify".to_string()), "done".to_string()),
        ]
    );
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core --test git_history_fixtures archive_rename_synthesizes_done_event`

Expected: FAIL because archive rename is not synthesized.

- [ ] **Step 3: Detect `R` name-status rows into `_archive`**

In the `git log --name-status -M` parser, handle rows like:

```text
R100    docs/workflow/001.md    docs/workflow/_archive/001.md
```

When `new_path.contains("/_archive/")` and both paths are entity paths:

```rust
StageEvent {
    entity_id,
    from: last_status.clone(),
    to: "done".to_string(),
    at: CommitTime(unix_time),
    commit: CommitId(commit.clone()),
}
```

Use the entity id from the archived blob frontmatter. Do not require `status: done`.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop-core --test git_history_fixtures archive_rename_synthesizes_done_event`

Expected: PASS.

```bash
git add crates/spacetop-core/src/git_history.rs crates/spacetop-core/tests/git_history_fixtures.rs
git commit -m "feat(core): synthesize done event from archive rename"
```

---

## Task 6: Stitch multiple renames

**Files:**
- Modify: `crates/spacetop-core/src/git_history.rs`
- Modify: `crates/spacetop-core/tests/git_history_fixtures.rs`

- [ ] **Step 1: Add multi-rename test**

Add to `crates/spacetop-core/tests/git_history_fixtures.rs`:

```rust
#[test]
fn multi_rename_keeps_one_entity_timeline() {
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    fs::create_dir_all(repo.workflow.join("renamed")).expect("dir");
    fs::rename(repo.workflow.join("001.md"), repo.workflow.join("renamed/index.md"))
        .expect("folder rename");
    repo.commit("folder form");
    repo.write_entity("renamed/index.md", "verify", "body");
    repo.commit("verify");

    let events = spacetop_core::git_history::GitHistorySource::new(
        &spacetop_core::git::StdGitRunner,
    )
        .load(&repo.root, "docs/workflow")
        .expect("history");

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].entity_id, "test");
    assert_eq!(events[1].entity_id, "test");
    assert_eq!(events[1].from.as_deref(), Some("plan"));
    assert_eq!(events[1].to, "verify");
}
```

- [ ] **Step 2: Run and verify it fails if paths split the timeline**

Run: `cargo test -p spacetop-core --test git_history_fixtures multi_rename_keeps_one_entity_timeline`

Expected: PASS only after entity id, not path, is the stable timeline key. If it already passes, keep the test and proceed.

- [ ] **Step 3: Key aggregation by entity id**

Ensure status aggregation uses the parsed `id` field from each blob. Path/rename records only decide which blobs to inspect and when to synthesize `done`.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop-core --test git_history_fixtures multi_rename_keeps_one_entity_timeline`

Expected: PASS.

```bash
git add crates/spacetop-core/src/git_history.rs crates/spacetop-core/tests/git_history_fixtures.rs
git commit -m "test(core): pin multi-rename history stitching"
```

---

## Task 6A: Prove shallow clone and merge topology behavior with real git fixtures

**Files:**
- Modify: `crates/spacetop-core/tests/git_history_fixtures.rs`

- [ ] **Step 1: Add a real shallow-clone fixture test**

Add a test that creates a source fixture repo with two status commits, clones it with `git clone --depth 1 file://...`, and calls `GitHistorySource::load` against the shallow clone. Expected: `Err(HistoryUnavailable::ShallowClone)`.

Run:

```bash
cargo test -p spacetop-core --test git_history_fixtures shallow_clone_refuses_history
```

Expected: PASS.

- [ ] **Step 2: Add a merge-topology fixture test**

Add a test that creates:

- mainline commit: `status: plan`
- feature branch commit changing body text only
- mainline commit: `status: verify`
- merge commit bringing the feature branch back

Then call `GitHistorySource::load` and assert the timeline has exactly `None -> plan` and `plan -> verify`; it must not add a duplicate status event from the merged body-only branch.

Run:

```bash
cargo test -p spacetop-core --test git_history_fixtures first_parent_merge_topology_ignores_body_branch
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/spacetop-core/tests/git_history_fixtures.rs
git commit -m "test(core): cover shallow clone and first-parent merge history"
```

---

## Task 7: Add metrics over stage events

**Files:**
- Create: `crates/spacetop-core/src/metrics.rs`
- Modify: `crates/spacetop-core/src/index.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write metrics tests**

Create `crates/spacetop-core/src/metrics.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{CommitId, CommitTime, StageEvent};

    #[test]
    fn metrics_computes_stage_dwell_seconds() {
        let events = vec![
            StageEvent {
                entity_id: "001".to_string(),
                from: None,
                to: "plan".to_string(),
                at: CommitTime(100),
                commit: CommitId("a".repeat(40)),
            },
            StageEvent {
                entity_id: "001".to_string(),
                from: Some("plan".to_string()),
                to: "verify".to_string(),
                at: CommitTime(160),
                commit: CommitId("b".repeat(40)),
            },
            StageEvent {
                entity_id: "001".to_string(),
                from: Some("verify".to_string()),
                to: "done".to_string(),
                at: CommitTime(220),
                commit: CommitId("c".repeat(40)),
            },
        ];

        let metrics = Metrics::from_events(&events);
        assert_eq!(metrics.stage_dwell_seconds.get("plan"), Some(&60));
        assert_eq!(metrics.stage_dwell_seconds.get("verify"), Some(&60));
        assert_eq!(metrics.cycle_time_seconds.get("001"), Some(&120));
        assert_eq!(metrics.completed_entities, 1);
        assert_eq!(metrics.throughput_completed, 1);
    }
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core metrics::tests`

Expected: FAIL because metrics types do not exist in this module.

- [ ] **Step 3: Implement minimal metrics**

Add:

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::index::StageEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub stage_dwell_seconds: HashMap<String, i64>,
    pub cycle_time_seconds: HashMap<String, i64>,
    pub wip_by_stage: HashMap<String, usize>,
    pub throughput_completed: usize,
    pub completed_entities: usize,
}

impl Metrics {
    pub fn from_events(events: &[StageEvent]) -> Self {
        let mut by_entity: HashMap<&str, Vec<&StageEvent>> = HashMap::new();
        for event in events {
            by_entity.entry(&event.entity_id).or_default().push(event);
        }

        let mut stage_dwell_seconds = HashMap::new();
        let mut cycle_time_seconds = HashMap::new();
        let mut wip_by_stage = HashMap::new();
        let mut completed_entities = 0usize;
        for timeline in by_entity.values_mut() {
            timeline.sort_by_key(|event| event.at);
            for pair in timeline.windows(2) {
                let current = pair[0];
                let next = pair[1];
                let delta = next.at.0.saturating_sub(current.at.0);
                *stage_dwell_seconds.entry(current.to.clone()).or_insert(0) += delta;
            }
            if let (Some(first), Some(last)) = (timeline.first(), timeline.last()) {
                cycle_time_seconds.insert(
                    first.entity_id.clone(),
                    last.at.0.saturating_sub(first.at.0),
                );
                *wip_by_stage.entry(last.to.clone()).or_insert(0) += 1;
            }
            if timeline.iter().any(|event| event.to == "done") {
                completed_entities += 1;
            }
        }

        Self {
            stage_dwell_seconds,
            cycle_time_seconds,
            wip_by_stage,
            throughput_completed: completed_entities,
            completed_entities,
        }
    }
}
```

- [ ] **Step 4: Wire index metrics**

In `index.rs`, replace the P1 placeholder `Metrics` type with `crate::metrics::Metrics`, store these fields in `WorkflowIndex`, and implement:

```rust
history_events: Vec<StageEvent>,
history_unavailable: Option<HistoryUnavailable>,
```

```rust
pub fn with_history_result(mut self, result: HistoryResult<Vec<StageEvent>>) -> Self {
    match result {
        Ok(events) => {
            self.history_events = events;
            self.history_unavailable = None;
        }
        Err(reason) => {
            self.history_events.clear();
            self.history_unavailable = Some(reason);
        }
    }
    self
}

pub fn metrics(&self) -> HistoryResult<crate::metrics::Metrics> {
    if self.history_events.is_empty() {
        return Err(self.history_unavailable());
    }
    Ok(crate::metrics::Metrics::from_events(&self.history_events))
}

fn history_unavailable(&self) -> HistoryUnavailable {
    self.history_unavailable
        .clone()
        .unwrap_or(HistoryUnavailable::NotImplemented)
}
```

- [ ] **Step 5: Export and verify**

In `lib.rs`, add:

```rust
pub mod metrics;
```

Run:

```bash
cargo test -p spacetop-core metrics::tests index::tests
```

Expected: PASS.

```bash
git add crates/spacetop-core/src/lib.rs crates/spacetop-core/src/index.rs crates/spacetop-core/src/metrics.rs
git commit -m "feat(core): compute metrics from stage events"
```

---

## Task 8: Integrate history into index loading

**Files:**
- Modify: `crates/spacetop-core/src/index.rs`
- Modify: `crates/spacetop-core/src/git_history.rs`
- Modify: `crates/spacetop-core/tests/git_history_fixtures.rs`
- Modify: `crates/spacetop/src/app/overview.rs`
- Create: `crates/spacetop/src/app/history_worker.rs`

- [ ] **Step 1: Add an integration test**

In `crates/spacetop-core/tests/git_history_fixtures.rs`, add:

```rust
#[test]
fn workflow_index_loads_history_events() {
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");

    let workflow_rel = "docs/workflow";
    let active = spacetop_core::sources::WorkflowSources::load_active(&repo.workflow, &repo.root)
        .expect("sources");
    let events = GitHistorySource::new(&spacetop_core::git::StdGitRunner)
        .load(&repo.root, workflow_rel);
    let index = spacetop_core::index::WorkflowIndex::from_sources(active)
        .with_history_result(events);

    let timeline = index.timeline("test").expect("timeline");
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].to, "plan");
    assert_eq!(timeline[1].to, "verify");
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core --test git_history_fixtures workflow_index_loads_history_events`

Expected: FAIL until `timeline` reads stored events.

- [ ] **Step 3: Implement timeline and activity from stored events**

In `index.rs`, implement:

```rust
pub fn timeline(&self, entity_id: &str) -> HistoryResult<Vec<StageEvent>> {
    if self.history_events.is_empty() {
        return Err(self.history_unavailable());
    }
    let mut events: Vec<StageEvent> = self
        .history_events
        .iter()
        .filter(|event| event.entity_id == entity_id)
        .cloned()
        .collect();
    events.sort_by_key(|event| event.at);
    Ok(events)
}

pub fn activity(&self, since: Option<CommitTime>) -> HistoryResult<Vec<ActivityEvent>> {
    if self.history_events.is_empty() {
        return Err(self.history_unavailable());
    }
    let mut events = self.history_events.clone();
    if let Some(since) = since {
        events.retain(|event| event.at >= since);
    }
    events.sort_by_key(|event| std::cmp::Reverse(event.at));
    Ok(events
        .into_iter()
        .map(|event| ActivityEvent {
            entity_id: event.entity_id.clone(),
            event,
        })
        .collect())
}
```

- [ ] **Step 4: Return stored unavailable reasons from timeline/activity**

Update `timeline` and `activity` so they match the metrics behavior from Task 7:

```rust
if self.history_events.is_empty() {
    return Err(self.history_unavailable());
}
```

This preserves `ShallowClone` and `GitError` instead of collapsing every empty history into `NotImplemented`.

- [ ] **Step 5: Add `WorkflowIndex::load_with_history`**

Add:

```rust
pub fn load_with_history<R: crate::git::GitRunner>(
    workflow_dir: &Path,
    repo_root: &Path,
    workflow_rel: &str,
    runner: &R,
) -> Result<Self, crate::parser::ParseError> {
    let index = Self::load(workflow_dir, repo_root)?;
    let history = crate::git_history::GitHistorySource::new(runner).load(repo_root, workflow_rel);
    Ok(index.with_history_result(history))
}
```

- [ ] **Step 6: Add non-blocking TUI history ingestion**

Keep `WorkflowIndex::load_with_history(...)` as the synchronous core/headless API. In the TUI, `OverviewState::load` and `OverviewState::reload` must still load the active working-tree index immediately, compute the workflow path relative to `repo_root`, then start a background history worker:

```rust
pub struct HistoryWorkerResult {
    pub workflow_dir: PathBuf,
    pub result: HistoryResult<Vec<StageEvent>>,
}
```

Create `crates/spacetop/src/app/history_worker.rs` with a small `std::thread::spawn` + `std::sync::mpsc` wrapper around `GitHistorySource::load`. The event loop should poll the receiver alongside watcher/sync events and call:

```rust
overview.apply_history_result(result);
```

`apply_history_result` updates the existing index with `with_history_result(result)` only if the result's workflow path matches the active overview state. If `GitHistorySource` returns `ShallowClone`, `NotGitRepository`, or `GitError`, keep the active entities visible and store the unavailable reason in the index so P3 views can render the exact message.

When the worker is started, mark the index history state as `HistoryUnavailable::Loading` until the result arrives. Add app tests that construct an index with `HistoryUnavailable::ShallowClone` and assert `timeline`, `metrics`, and `activity(None)` surface that reason through `OverviewState::index()`. Add a second test proving `OverviewState::load` returns before a fake history worker responds, exposes `Loading` while pending, and that `apply_history_result` later populates the timeline.

- [ ] **Step 7: Verify and commit**

Run:

```bash
cargo test -p spacetop-core git_history::tests index::tests metrics::tests
cargo test -p spacetop-core --test git_history_fixtures workflow_index_loads_history_events
cargo test -p spacetop app::tests
```

Expected: PASS.

```bash
git add crates/spacetop-core/src/index.rs crates/spacetop-core/src/git_history.rs crates/spacetop-core/tests/git_history_fixtures.rs crates/spacetop/src/app/overview.rs crates/spacetop/src/app/history_worker.rs
git commit -m "feat(core): attach git history events to workflow index"
```

---

## Task 9: Extend read-only guardrails for history

**Files:**
- Modify: `crates/spacetop-core/tests/no_write_git_calls.rs`
- Modify: `crates/spacetop-core/src/git_history.rs`

- [ ] **Step 1: Add behavioral git command assertion**

In `git_history.rs` tests, add:

```rust
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
```

- [ ] **Step 2: Run the behavioral guard**

Run: `cargo test -p spacetop-core git_history::tests::history_source_uses_only_approved_read_commands`

Expected: PASS.

- [ ] **Step 3: Keep static guard passing**

Run:

```bash
cargo test -p spacetop-core --test no_write_git_calls
```

Expected: PASS. If `--ff-only` count changed, fix the code so it still appears exactly once in sync code only.

- [ ] **Step 4: Full verification and commit**

Run:

```bash
cargo test --workspace
make lint
cargo test -p spacetop-core --test no_terminal_deps
```

Expected: all PASS.

```bash
git add crates/spacetop-core/src/git_history.rs crates/spacetop-core/tests/no_write_git_calls.rs
git commit -m "test(core): guard git history read-only commands"
```

## Definition of done (P2)

- [ ] `GitHistorySource` derives frontmatter-only stage events.
- [ ] Archive rename into `_archive/` synthesizes `to = "done"`.
- [ ] Multi-rename fixture keeps one entity timeline.
- [ ] Shallow clones return `HistoryUnavailable::ShallowClone`.
- [ ] Real git fixture coverage proves shallow clone and first-parent merge behavior.
- [ ] `WorkflowIndex::timeline` returns ordered events for one entity.
- [ ] `WorkflowIndex::metrics` returns dwell, cycle, WIP, and throughput data from events.
- [ ] TUI overview load/reload starts active-only, non-blocking history ingestion and folds worker results into the index.
- [ ] `WorkflowIndex::activity(since)` filters and returns newest-first events.
- [ ] Static and behavioral git guardrails pass.
- [ ] `cargo test --workspace` passes.
- [ ] `make lint` passes.
