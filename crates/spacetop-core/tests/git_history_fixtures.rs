use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use spacetop_core::git::StdGitRunner;
use spacetop_core::git_history::GitHistorySource;
use spacetop_core::query::HistoryUnavailable;
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
            "---\ncommissioned-by: spacedock@test\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: verify\n    - name: done\n      terminal: true\n---\n",
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

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git command");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={}, stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn load_history(repo: &RepoFixture) -> Vec<spacetop_core::index::StageEvent> {
    GitHistorySource::new(&StdGitRunner)
        .load(&repo.root, "docs/workflow")
        .expect("history")
}

#[test]
fn fixture_repo_can_make_status_commits() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");

    let output = Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("rev-list");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2");
}

#[test]
fn body_status_decoy_does_not_create_stage_event() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body mentions status: verify");
    repo.commit("plan");
    repo.write_entity("001.md", "plan", "body now mentions status: done");
    repo.commit("body only");

    let events = load_history(&repo);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from, None);
    assert_eq!(events[0].to, "plan");
}

#[test]
fn archive_rename_synthesizes_done_event() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");
    fs::create_dir_all(repo.workflow.join("_archive")).expect("archive dir");
    fs::rename(
        repo.workflow.join("001.md"),
        repo.workflow.join("_archive/001.md"),
    )
    .expect("archive rename");
    repo.commit("archive");

    let transitions: Vec<(Option<String>, String)> = load_history(&repo)
        .into_iter()
        .map(|event| (event.from, event.to))
        .collect();
    assert_eq!(
        transitions,
        [
            (None, "plan".to_string()),
            (Some("plan".to_string()), "verify".to_string()),
            (Some("verify".to_string()), "done".to_string()),
        ]
    );
}

#[test]
fn multi_rename_keeps_one_entity_timeline() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    fs::create_dir_all(repo.workflow.join("renamed")).expect("dir");
    fs::rename(
        repo.workflow.join("001.md"),
        repo.workflow.join("renamed/index.md"),
    )
    .expect("folder rename");
    repo.commit("folder form");
    repo.write_entity("renamed/index.md", "verify", "body");
    repo.commit("verify");

    let events = load_history(&repo);

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].entity_id, "test");
    assert_eq!(events[1].entity_id, "test");
    assert_eq!(events[1].from.as_deref(), Some("plan"));
    assert_eq!(events[1].to, "verify");
}

#[test]
fn shallow_clone_refuses_history() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");

    let holder = tempfile::tempdir().expect("holder");
    let source = format!("file://{}", repo.root.display());
    run_git(
        holder.path(),
        &["clone", "--depth", "1", &source, "shallow"],
    );
    let shallow = holder.path().join("shallow");

    let result = GitHistorySource::new(&StdGitRunner).load(&shallow, "docs/workflow");
    assert_eq!(result.unwrap_err(), HistoryUnavailable::ShallowClone);
}

#[test]
fn first_parent_merge_topology_ignores_body_branch() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    run_git(&repo.root, &["checkout", "-b", "feature"]);
    repo.write_entity("001.md", "plan", "branch body mentions status: done");
    repo.commit("body branch");
    run_git(&repo.root, &["checkout", "-"]);
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");
    run_git(
        &repo.root,
        &["merge", "--no-ff", "feature", "-m", "merge feature"],
    );

    let transitions: Vec<(Option<String>, String)> = load_history(&repo)
        .into_iter()
        .map(|event| (event.from, event.to))
        .collect();

    assert_eq!(
        transitions,
        [
            (None, "plan".to_string()),
            (Some("plan".to_string()), "verify".to_string()),
        ]
    );
}

#[test]
fn workflow_index_loads_history_events() {
    if !git_available() {
        return;
    }
    let repo = RepoFixture::new();
    repo.write_entity("001.md", "plan", "body");
    repo.commit("plan");
    repo.write_entity("001.md", "verify", "body");
    repo.commit("verify");

    let active = spacetop_core::sources::WorkflowSources::load_active(&repo.workflow, &repo.root)
        .expect("sources");
    let events = GitHistorySource::new(&StdGitRunner).load(&repo.root, "docs/workflow");
    let index =
        spacetop_core::index::WorkflowIndex::from_sources(active).with_history_result(events);

    let timeline = index.timeline("test").expect("timeline");
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].to, "plan");
    assert_eq!(timeline[1].to, "verify");
}
