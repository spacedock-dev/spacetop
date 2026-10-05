#![cfg(unix)]
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

struct Fixture {
    temp: TempDir,
    project: PathBuf,
    helper: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project spaces ; $(touch NEVER)");
        fs::create_dir(&project).unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .arg(&project)
            .status()
            .unwrap()
            .success());
        let f = Self {
            temp,
            project: project.canonicalize().unwrap(),
            helper: PathBuf::from(env!("CARGO_BIN_EXE_spacetop-herdr")),
        };
        f.reset();
        f
    }
    fn root(&self) -> &Path {
        self.temp.path()
    }
    fn reset(&self) {
        fs::write(self.root().join("panes.json"), serde_json::to_vec(&json!([{"pane_id":"w1:p1","workspace_id":"w1","tab_id":"w1:t1","cwd":self.project,"focused":true}])).unwrap()).unwrap();
    }
    fn command(&self, action: &str) -> Command {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/herdr")
            .canonicalize()
            .unwrap();
        let mut c = Command::new(&self.helper);
        c.arg(action).current_dir(&fixtures).env("TEST_HERDR_ROOT",self.root())
            .env("HERDR_BIN_PATH",fixtures.join("herdr.py"))
            .env("SPACETOP_BIN",fixtures.join("spacetop.py"))
            .env("HERDR_PLUGIN_STATE_DIR",self.root().join("state"))
            .env("HERDR_SOCKET_PATH",self.root().join("socket"))
            .env("HERDR_PLUGIN_CONTEXT_JSON", json!({"workspace_id":"w1","tab_id":"w1:t1","focused_pane_id":"w1:p1","focused_pane_cwd":self.project}).to_string());
        c
    }
    fn run(&self, mode: &str) -> Output {
        self.command("open")
            .env("TEST_HERDR_MODE", mode)
            .output()
            .unwrap()
    }
    fn panes(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root().join("panes.json")).unwrap()).unwrap()
    }
    fn calls(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.root().join("argv.jsonl"))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn failure(output: Output, message: &str) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn opens_right_without_focus_reuses_and_reopens_after_close() {
    let f = Fixture::new();
    success(f.run(""));
    let calls = f.calls();
    let open = &calls[1];
    for pair in [
        ["--placement", "split"],
        ["--direction", "right"],
        ["--target-pane", "w1:p1"],
    ] {
        assert!(open.windows(2).any(|w| w == pair));
    }
    assert!(open.contains(&"--no-focus".into()));
    assert!(open
        .windows(2)
        .any(|w| w == ["--cwd", f.project.to_str().unwrap()]));
    assert!(!f.project.join("NEVER").exists());
    success(f.run(""));
    assert_eq!(f.panes().as_array().unwrap().len(), 2);
    assert_eq!(
        f.calls()
            .iter()
            .filter(|a| a[0..3] == ["plugin", "pane", "open"])
            .count(),
        1
    );
    f.reset();
    success(f.run(""));
    assert_eq!(
        f.calls()
            .iter()
            .filter(|a| a.len() > 2 && a[0..3] == ["plugin", "pane", "open"])
            .count(),
        2
    );
}

#[test]
fn invalid_context_and_missing_binary_fail_before_api() {
    let f = Fixture::new();
    failure(
        f.command("open")
            .env_remove("HERDR_PLUGIN_CONTEXT_JSON")
            .output()
            .unwrap(),
        "context missing",
    );
    failure(f.command("open").env("HERDR_PLUGIN_CONTEXT_JSON",json!({"workspace_id":"w1","tab_id":"w1:t1","focused_pane_id":"w1:p1","focused_pane_cwd":"relative"}).to_string()).output().unwrap(),"absolute directory");
    failure(
        f.command("open")
            .env("SPACETOP_BIN", "/nonexistent")
            .output()
            .unwrap(),
        "not executable",
    );
    assert!(!f.root().join("argv.jsonl").exists());
}

#[test]
fn fails_clearly_on_api_or_owner_or_duplicate_or_project_mismatch() {
    let f = Fixture::new();
    failure(f.run("api-fail"), "socket unavailable");
    failure(f.run("malformed"), "malformed Herdr");
    success(f.run(""));
    failure(f.run("focus-fail"), "not a plugin pane");
    failure(f.run("wrong-owner"), "mismatched sidecar");
    let mut panes = f.panes();
    panes[1]["cwd"] = json!("/elsewhere");
    fs::write(f.root().join("panes.json"), panes.to_string()).unwrap();
    failure(f.run(""), "another project");
    panes[1]["cwd"] = json!(f.project);
    let duplicate = panes[1].clone();
    panes.as_array_mut().unwrap().push(duplicate);
    fs::write(f.root().join("panes.json"), panes.to_string()).unwrap();
    failure(f.run(""), "multiple Spacetop");
}

#[test]
fn workspace_query_and_tab_context_are_checked() {
    let f = Fixture::new();
    let mut panes = f.panes();
    panes[0]["workspace_id"] = json!("w2");
    fs::write(f.root().join("panes.json"), panes.to_string()).unwrap();
    failure(f.run(""), "another workspace");
    f.reset();
    failure(f.command("open").env("HERDR_PLUGIN_CONTEXT_JSON",json!({"workspace_id":"w1","tab_id":"w1:t2","focused_pane_id":"w1:p1","focused_pane_cwd":f.project}).to_string()).output().unwrap(),"supplied tab");
}

#[test]
fn pane_executes_binary_with_project_as_one_argument_and_preserves_git_state() {
    let f = Fixture::new();
    let before = fs::read(f.project.join(".git/HEAD")).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/herdr/spacetop.py")
        .canonicalize()
        .unwrap();
    success(
        f.command("pane")
            .env("SPACETOP_SIDECAR_PROJECT", &f.project)
            .env("SPACETOP_SIDECAR_BIN", fixture)
            .output()
            .unwrap(),
    );
    let record: Value =
        serde_json::from_slice(&fs::read(f.root().join("spacetop.json")).unwrap()).unwrap();
    assert_eq!(record["argv"], json!(["--workflow-dir", f.project]));
    assert_eq!(record["cwd"], json!(f.project));
    assert_eq!(before, fs::read(f.project.join(".git/HEAD")).unwrap());
    assert!(!f.project.join("NEVER").exists());
}

#[test]
fn overlapping_actions_serialize_to_one_open() {
    let f = Fixture::new();
    let mut first = f
        .command("open")
        .env("TEST_HERDR_MODE", "slow")
        .spawn()
        .unwrap();
    success(f.run("slow"));
    assert!(first.wait().unwrap().success());
    assert_eq!(
        f.calls()
            .iter()
            .filter(|a| a.len() > 2 && a[0..3] == ["plugin", "pane", "open"])
            .count(),
        1
    );
}

#[test]
fn linked_worktree_subdirectory_keeps_worktree_instead_of_main_checkout() {
    let mut f = Fixture::new();
    fs::write(f.project.join("marker"), "main").unwrap();
    for args in [
        vec!["add", "marker"],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&f.project)
            .status()
            .unwrap()
            .success());
    }
    let worktree = f.root().join("linked worktree");
    assert!(Command::new("git")
        .args(["worktree", "add", "-qb", "test"])
        .arg(&worktree)
        .current_dir(&f.project)
        .status()
        .unwrap()
        .success());
    f.project = worktree.canonicalize().unwrap();
    f.reset();
    let subdir = f.project.join("nested");
    fs::create_dir(&subdir).unwrap();
    success(f.command("open").env("HERDR_PLUGIN_CONTEXT_JSON",json!({"workspace_id":"w1","tab_id":"w1:t1","focused_pane_id":"w1:p1","focused_pane_cwd":subdir,"workspace_cwd":f.root()}).to_string()).output().unwrap());
    assert_eq!(f.panes()[1]["cwd"], json!(f.project));
}

#[test]
fn state_lock_never_writes_inside_project_and_non_git_context_fails() {
    let f = Fixture::new();
    let forbidden = f.project.join("workflow/state");
    failure(
        f.command("open")
            .env("HERDR_PLUGIN_STATE_DIR", &forbidden)
            .output()
            .unwrap(),
        "outside the project",
    );
    assert!(!forbidden.exists());
    let link = f.root().join("state-link");
    std::os::unix::fs::symlink(&f.project, &link).unwrap();
    failure(
        f.command("open")
            .env("HERDR_PLUGIN_STATE_DIR", link.join("new-state"))
            .output()
            .unwrap(),
        "inside the project",
    );
    assert!(!f.project.join("new-state").exists());
    failure(f.command("open").env("HERDR_PLUGIN_CONTEXT_JSON",json!({"workspace_id":"w1","tab_id":"w1:t1","focused_pane_id":"w1:p1","focused_pane_cwd":f.root()}).to_string()).output().unwrap(),"not in an accessible Git");
}

#[test]
fn stale_lock_fails_with_recovery_path_and_wrong_open_focus_is_rejected() {
    let f = Fixture::new();
    failure(f.run("wrong-open"), "unfocused sidecar");
    f.reset();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    Some(f.root().join("socket").into_os_string()).hash(&mut hash);
    "w1".hash(&mut hash);
    let lock = f
        .root()
        .join("state")
        .join(format!("open-{:016x}.lock", hash.finish()));
    fs::write(lock, "stale").unwrap();
    failure(f.run(""), "remove a stale lock");
}

#[test]
fn project_longer_than_host_token_limit_reuses_full_pane_cwd() {
    let mut f = Fixture::new();
    let long_path = f.root().join("long project ".repeat(12));
    fs::rename(&f.project, &long_path).unwrap();
    f.project = long_path.canonicalize().unwrap();
    assert!(f.project.to_str().unwrap().len() > 80);
    f.reset();
    success(f.run(""));
    success(f.run(""));
    assert_eq!(f.panes().as_array().unwrap().len(), 2);
    assert_eq!(f.panes()[1]["cwd"], json!(f.project));
    assert!(f.panes()[1]["tokens"].get("spacetop_project").is_none());
    assert_eq!(
        f.calls()
            .iter()
            .filter(|a| a.len() > 2 && a[0..3] == ["plugin", "pane", "open"])
            .count(),
        1
    );
}
