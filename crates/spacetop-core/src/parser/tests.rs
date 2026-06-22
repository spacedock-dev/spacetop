use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    load_archived_items, load_archived_items_with_errors, load_workflow_dir, parse_work_item,
    parse_workflow_readme, ParseError,
};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev")
}

fn write_temp_markdown(name: &str, contents: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spacetop-parser-test-{unique}"));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let path = dir.join(name);
    fs::write(&path, contents).expect("temp markdown should be written");
    path
}

fn stage_names(root: &Path) -> Vec<String> {
    parse_workflow_readme(&root.join("README.md"))
        .expect("workflow README should parse")
        .stages
        .into_iter()
        .map(|stage| stage.name)
        .collect()
}

#[test]
fn parses_workflow_readme_stage_metadata_with_defaults_and_overrides() {
    let root = fixture_root();
    let workflow =
        parse_workflow_readme(&root.join("README.md")).expect("workflow README should parse");

    assert_eq!(workflow.root, root);
    assert_eq!(workflow.id_style.as_deref(), Some("sequential"));
    assert_eq!(workflow.entity_type.as_deref(), Some("development_task"));
    assert_eq!(
        workflow
            .stages
            .iter()
            .map(|stage| stage.name.as_str())
            .collect::<Vec<_>>(),
        ["shape", "plan", "implement", "verify", "done"]
    );

    let shape = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "shape")
        .expect("shape stage should exist");
    assert!(shape.initial);
    assert!(!shape.terminal);
    assert_eq!(shape.concurrency, Some(2));

    let implement = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "implement")
        .expect("implement stage should exist");
    assert!(implement.worktree);
    assert_eq!(implement.concurrency, Some(2));

    let verify = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "verify")
        .expect("verify stage should exist");
    assert!(verify.gate);
    assert!(verify.fresh);
    assert_eq!(verify.feedback_to.as_deref(), Some("implement"));

    let done = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "done")
        .expect("done stage should exist");
    assert!(done.terminal);
}

#[test]
fn parses_work_item_frontmatter_and_preserves_markdown_body() {
    let root = fixture_root();
    let allowed_statuses = stage_names(&root);
    let path = write_temp_markdown(
        "work-item.md",
        r#"---
id: "002"
title: Parse Spacedock Workflow Files
status: implement
source: commission seed
score: 1.0
worktree: .worktrees/spacedock-ensign-parse-spacedock-workflow-files
---

Read Spacedock workflow files into typed models.

## Acceptance criteria

Body text should be preserved without frontmatter.
"#,
    );
    let item = parse_work_item(&path, &allowed_statuses, None).expect("work item should parse");

    assert_eq!(item.id, "002");
    assert_eq!(item.title, "Parse Spacedock Workflow Files");
    assert_eq!(item.status, "implement");
    assert_eq!(item.source.as_deref(), Some("commission seed"));
    assert_eq!(item.score, Some(1.0));
    assert_eq!(
        item.worktree.as_deref(),
        Some(".worktrees/spacedock-ensign-parse-spacedock-workflow-files")
    );
    assert!(item.body.starts_with("Read Spacedock workflow"));
    assert!(item.body.contains("## Acceptance criteria"));
    assert!(!item.body.starts_with("---"));
}

#[test]
fn loads_workflow_snapshot_from_directory_ignoring_mods_and_archive() {
    let root = unique_temp_dir("snapshot");
    // The real `docs/spacetop-dev` README is now split-root (`state:`), so its
    // entities live in a separate checkout. This test exercises the single-root
    // active-loading + `_mods`/`_archive` ignore contract with entities beside
    // the README, so strip the `state:` declaration from the copied README to
    // keep it single-root while preserving the real 5-stage definition.
    let readme = fs::read_to_string(fixture_root().join("README.md")).expect("read README fixture");
    let readme: String = readme
        .lines()
        .filter(|line| !line.trim_start().starts_with("state:"))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(root.join("README.md"), readme).expect("README fixture should write");
    write_markdown(
        &root.join("active.md"),
        r#"---
id: "001"
title: Active
status: shape
---

Active body.
"#,
    );
    write_markdown(
        &root.join("_mods/ignored.md"),
        r#"---
id: "002"
title: Ignored Mod
status: shape
---

Ignored.
"#,
    );
    write_markdown(
        &root.join("_archive/archived.md"),
        r#"---
id: "003"
title: Ignored Archived
status: done
---

Ignored.
"#,
    );
    let snapshot = load_workflow_dir(&root, &root).expect("workflow directory should load");

    assert_eq!(snapshot.definition.stages.len(), 5);
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Active");
    assert!(snapshot
        .items
        .iter()
        .all(|item| !item.path.components().any(|component| {
            let value = component.as_os_str();
            value == "_mods" || value == "_archive"
        })));
    let allowed_statuses = snapshot
        .definition
        .stages
        .iter()
        .map(|stage| stage.name.as_str())
        .collect::<Vec<_>>();
    assert!(snapshot
        .items
        .iter()
        .all(|item| allowed_statuses.contains(&item.status.as_str())));
}

#[test]
fn missing_frontmatter_error_names_file_and_context() {
    let path = write_temp_markdown("missing.md", "# Missing\n");
    let error = parse_work_item(&path, &["design".to_string()], None)
        .expect_err("missing frontmatter should fail")
        .to_string();

    assert!(error.contains("missing YAML frontmatter"));
    assert!(error.contains("missing.md"));
}

#[test]
fn unknown_status_error_includes_value_and_allowed_context() {
    let path = write_temp_markdown(
        "unknown.md",
        r#"---
id: "999"
title: Unknown Status
status: impossible
---

Body
"#,
    );
    let error = parse_work_item(&path, &["design".to_string(), "done".to_string()], None)
        .expect_err("unknown status should fail")
        .to_string();

    assert!(error.contains("unknown status 'impossible'"));
    assert!(error.contains("allowed statuses: design, done"));
}

#[test]
fn malformed_yaml_error_is_distinct_from_validation_errors() {
    let path = write_temp_markdown(
        "malformed.md",
        r#"---
id: [
---

Body
"#,
    );
    let error = parse_work_item(&path, &["design".to_string()], None)
        .expect_err("malformed YAML should fail")
        .to_string();

    assert!(error.contains("malformed YAML frontmatter"));
    assert!(error.contains("malformed.md"));
}

#[test]
fn parses_flat_frontmatter_with_unquoted_colon_in_title() {
    let path = write_temp_markdown(
        "colon-title.md",
        r#"---
id: 132
title: Codex first officer: derive reusable context visibility
status: ideation
source: FO observation
score: 0.62
---

Body
"#,
    );
    let item = parse_work_item(
        &path,
        &[
            "backlog".to_string(),
            "ideation".to_string(),
            "done".to_string(),
        ],
        None,
    )
    .expect("flat frontmatter fallback should parse");

    assert_eq!(item.id, "132");
    assert_eq!(
        item.title,
        "Codex first officer: derive reusable context visibility"
    );
    assert_eq!(item.status, "ideation");
    assert_eq!(item.source.as_deref(), Some("FO observation"));
    assert_eq!(item.score, Some(0.62));
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("spacetop-archive-{label}-{unique}"));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn write_markdown(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should be created");
    }
    fs::write(path, contents).expect("markdown should be written");
}

#[test]
fn load_archived_items_returns_entries_from_flat_files() {
    // The real `docs/spacetop-dev` archive moved into the split-root state
    // checkout, which is not present in a code worktree. Build a single-root
    // archive fixture to exercise flat-file loading + status preservation.
    let root = unique_temp_dir("archive-flat");
    let archive = root.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");
    let allowed = vec!["design".to_string(), "done".to_string()];
    write_markdown(
        &archive.join("scaffold.md"),
        &entity_md_with_status("001", "Scaffold Rust CLI Project", "done", "scaffold body"),
    );
    write_markdown(
        &archive.join("parse.md"),
        &entity_md_with_status(
            "002",
            "Parse Spacedock Workflow Files",
            "done",
            "parse body",
        ),
    );
    write_markdown(
        &archive.join("tui.md"),
        &entity_md_with_status("003", "Build Initial TUI Overview", "done", "tui body"),
    );

    let items = load_archived_items(&root, &allowed, None).expect("archive should load");

    assert!(items.len() >= 3, "expected at least 3 archived entries");
    let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
    assert!(titles.contains(&"Scaffold Rust CLI Project"));
    assert!(titles.contains(&"Parse Spacedock Workflow Files"));
    assert!(titles.contains(&"Build Initial TUI Overview"));
    assert!(
        items.iter().any(|item| item.status == "done"),
        "archived items should preserve terminal frontmatter status"
    );
    assert!(
        items
            .iter()
            .all(|item| allowed.iter().any(|status| status == &item.status)),
        "archived items should preserve statuses from the workflow stage set"
    );
}

#[test]
fn load_archived_items_sorts_by_completed_desc_with_missing_last() {
    let dir = unique_temp_dir("sort");
    let archive = dir.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");

    write_markdown(
        &archive.join("early.md"),
        r#"---
id: "001"
title: Early
status: done
completed: 2026-04-24T14:49:53Z
---

Body
"#,
    );
    write_markdown(
        &archive.join("late.md"),
        r#"---
id: "002"
title: Late
status: done
completed: 2026-04-24T15:00:00Z
---

Body
"#,
    );
    write_markdown(
        &archive.join("unknown.md"),
        r#"---
id: "003"
title: Unknown
status: done
---

Body
"#,
    );

    let items = load_archived_items(&dir, &["done".to_string()], None).expect("archive load");
    let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
    assert_eq!(titles, vec!["Late", "Early", "Unknown"]);
}

#[test]
fn load_archived_items_reads_folder_entity_index_md() {
    let dir = unique_temp_dir("folder");
    let archive = dir.join("_archive");
    let entity = archive.join("foo");
    fs::create_dir_all(&entity).expect("entity dir");

    write_markdown(
        &entity.join("index.md"),
        r#"---
id: "010"
title: Folder Entity
status: done
completed: 2026-04-24T10:00:00Z
---

Body
"#,
    );
    write_markdown(
        &entity.join("notes.md"),
        r#"---
id: "011"
title: Should Be Ignored
status: done
---

Body
"#,
    );

    let items = load_archived_items(&dir, &["done".to_string()], None).expect("archive load");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Folder Entity");
}

#[test]
fn load_archived_items_missing_archive_dir_is_empty_ok() {
    let dir = unique_temp_dir("missing");
    let items = load_archived_items(&dir, &["done".to_string()], None).expect("should be Ok");
    assert!(items.is_empty());
}

#[test]
fn load_archived_items_returns_empty_when_all_entries_are_malformed() {
    let dir = unique_temp_dir("broken");
    let archive = dir.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");
    write_markdown(
        &archive.join("broken.md"),
        r#"---
id: [
---

Body
"#,
    );

    let items = load_archived_items(&dir, &["done".to_string()], None).expect("archive load");
    assert!(items.is_empty());
}

#[test]
fn load_archived_items_skips_malformed_entries_and_keeps_valid_ones() {
    let dir = unique_temp_dir("archive-skip-broken");
    let archive = dir.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");
    write_markdown(
        &archive.join("good.md"),
        r#"---
id: "001"
title: Good
status: done
completed: 2026-04-24T15:00:00Z
---

Body
"#,
    );
    write_markdown(
        &archive.join("broken.md"),
        r#"---
id: 131
title: Broken: archive entry
status: validation
---

Body
"#,
    );

    let items = load_archived_items(&dir, &["done".to_string()], None).expect("archive load");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Good");
}

#[test]
fn load_archived_items_with_errors_collects_malformed_entries_and_keeps_valid_ones() {
    let dir = unique_temp_dir("archive-collect-broken");
    let archive = dir.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");
    write_markdown(
        &archive.join("good.md"),
        r#"---
id: "001"
title: Good
status: done
completed: 2026-04-24T15:00:00Z
---

Body
"#,
    );
    write_markdown(
        &archive.join("broken.md"),
        r#"---
id: [
---

Body
"#,
    );

    let (items, parse_errors) =
        load_archived_items_with_errors(&dir, &["done".to_string()], None).expect("archive load");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Good");
    assert_eq!(parse_errors.len(), 1);
    assert!(parse_errors[0].message.contains("malformed YAML"));

    let compatibility_items =
        load_archived_items(&dir, &["done".to_string()], None).expect("archive load");
    assert_eq!(compatibility_items.len(), 1);
    assert_eq!(compatibility_items[0].title, "Good");
}

#[cfg(unix)]
#[test]
fn load_archived_items_returns_io_errors_instead_of_silently_skipping_them() {
    let dir = unique_temp_dir("archive-io-error");
    let archive = dir.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");
    std::os::unix::fs::symlink(archive.join("missing-target.md"), archive.join("broken.md"))
        .expect("symlink");

    let err = load_archived_items(&dir, &["done".to_string()], None)
        .expect_err("archive load should fail");
    assert!(
        matches!(err, ParseError::ReadFile { .. }),
        "expected ReadFile error, got {err:?}"
    );
}

#[test]
fn missing_required_work_item_field_error_names_field() {
    let path = write_temp_markdown(
        "missing-title.md",
        r#"---
id: "999"
status: design
---

Body
"#,
    );
    let error = parse_work_item(&path, &["design".to_string()], None)
        .expect_err("missing title should fail")
        .to_string();

    assert!(error.contains("missing required field 'title'"));
    assert!(error.contains("missing-title.md"));
}

// ---- Worktree scan tests (AC-1 through AC-4, AC-6, AC-7) ----

/// Write a minimal workflow README and an optional entity file into `dir`.
fn write_minimal_workflow(dir: &Path, entity_name: Option<&str>, entity_content: Option<&str>) {
    fs::create_dir_all(dir).expect("workflow dir");
    fs::write(
            dir.join("README.md"),
            "---\ncommissioned-by: spacedock@0.10.1\nstages:\n  states:\n    - name: design\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
        )
        .expect("write README");
    if let (Some(name), Some(content)) = (entity_name, entity_content) {
        write_markdown(&dir.join(name), content);
    }
}

fn entity_md(id: &str, title: &str) -> String {
    format!("---\nid: \"{id}\"\ntitle: {title}\nstatus: design\n---\n\n{title} body.\n")
}

#[test]
fn worktree_items_included() {
    // AC-1, AC-6: two worktrees each with a distinct entity
    let root = unique_temp_dir("wt-included");
    let wf = root.join("docs/wf");
    write_minimal_workflow(
        &wf,
        Some("main-task.md"),
        Some(&entity_md("001", "Main Task")),
    );
    let wt_a = root.join(".worktrees/wt-a/docs/wf");
    write_minimal_workflow(&wt_a, Some("task-a.md"), Some(&entity_md("002", "Task A")));
    let wt_b = root.join(".worktrees/wt-b/docs/wf");
    write_minimal_workflow(&wt_b, Some("task-b.md"), Some(&entity_md("003", "Task B")));

    let snapshot = load_workflow_dir(&wf, &root).expect("load workflow dir");
    let titles: Vec<&str> = snapshot.items.iter().map(|i| i.title.as_str()).collect();
    assert!(
        titles.contains(&"Main Task"),
        "main task missing: {titles:?}"
    );
    assert!(titles.contains(&"Task A"), "task-a missing: {titles:?}");
    assert!(titles.contains(&"Task B"), "task-b missing: {titles:?}");
    assert_eq!(snapshot.items.len(), 3);
}

#[test]
fn main_only_items_preserved() {
    // AC-2: main has aaa, worktree has bbb — both appear
    let root = unique_temp_dir("main-only");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, Some("aaa.md"), Some(&entity_md("001", "AAA")));
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt, Some("bbb.md"), Some(&entity_md("002", "BBB")));

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    let titles: Vec<&str> = snapshot.items.iter().map(|i| i.title.as_str()).collect();
    assert!(
        titles.contains(&"AAA"),
        "main-only item dropped: {titles:?}"
    );
    assert!(
        titles.contains(&"BBB"),
        "worktree-only item missing: {titles:?}"
    );
}

#[test]
fn worktree_only_items_shown() {
    // AC-3: main has no entity files; worktree has ccc.md
    let root = unique_temp_dir("wt-only");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, None, None);
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt, Some("ccc.md"), Some(&entity_md("003", "CCC")));

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "CCC");
}

#[test]
fn active_folder_form_item_loads_from_index_md() {
    let root = unique_temp_dir("active-folder-form");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, None, None);
    let task_dir = wf.join("task-folder");
    fs::create_dir_all(&task_dir).expect("task dir");
    write_markdown(&task_dir.join("index.md"), &entity_md("064", "Folder Task"));
    write_markdown(&task_dir.join("notes.md"), &entity_md("065", "Nested Note"));

    let snapshot = load_workflow_dir(&wf, &root).expect("load");

    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Folder Task");
}

#[test]
fn worktree_only_folder_form_item_loads_from_index_md() {
    let root = unique_temp_dir("wt-folder-form");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, None, None);
    let wt_task_dir = root.join(".worktrees/wt-1/docs/wf/folder-task");
    fs::create_dir_all(&wt_task_dir).expect("worktree task dir");
    write_minimal_workflow(&root.join(".worktrees/wt-1/docs/wf"), None, None);
    write_markdown(
        &wt_task_dir.join("index.md"),
        &entity_md("064", "Worktree Folder Task"),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load");

    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Worktree Folder Task");
    assert!(
        snapshot.items[0]
            .worktree_source
            .as_ref()
            .is_some_and(|path| path.ends_with("folder-task/index.md")),
        "worktree-only folder item should keep its source path"
    );
}

#[test]
fn worktree_version_wins_on_hash_mismatch() {
    // AC-4: same slug in main and worktree with different content.
    // After the merged-view fix: FO-owned fields (title, status) come from main;
    // body comes from worktree.
    let root = unique_temp_dir("wt-wins");
    let wf = root.join("docs/wf");
    write_minimal_workflow(
        &wf,
        Some("task.md"),
        Some(&entity_md("010", "Main Version")),
    );
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(
        &wt,
        Some("task.md"),
        Some(&entity_md("010", "Worktree Version")),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    // FO-owned frontmatter (title) comes from main.
    assert_eq!(snapshot.items[0].title, "Main Version");
    // Body comes from worktree.
    assert!(
        snapshot.items[0].body.contains("Worktree Version body"),
        "body should come from worktree: {:?}",
        snapshot.items[0].body
    );
}

#[test]
fn no_regression_without_worktrees() {
    // AC-7: no .worktrees directory — behavior identical to before
    let root = unique_temp_dir("no-wt");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, Some("solo.md"), Some(&entity_md("001", "Solo")));
    // Confirm no .worktrees dir exists
    assert!(!root.join(".worktrees").exists());

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Solo");
}

/// Write an entity with a custom status and body for merge-view tests.
fn entity_md_with_status(id: &str, title: &str, status: &str, body: &str) -> String {
    format!("---\nid: \"{id}\"\ntitle: {title}\nstatus: {status}\n---\n\n{body}\n")
}

/// Write a minimal workflow README with both `design` and `done` states.
fn write_two_state_workflow(dir: &Path, entity_name: Option<&str>, entity_content: Option<&str>) {
    fs::create_dir_all(dir).expect("workflow dir");
    fs::write(
            dir.join("README.md"),
            "---\ncommissioned-by: spacedock@0.10.1\nstages:\n  states:\n    - name: design\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
        )
        .expect("write README");
    if let (Some(name), Some(content)) = (entity_name, entity_content) {
        write_markdown(&dir.join(name), content);
    }
}

#[test]
fn worktree_status_from_main() {
    // AC-1: when main has status=done and worktree has status=design,
    // the merged item displays the main-branch status (done).
    let root = unique_temp_dir("wt-status-main");
    let wf = root.join("docs/wf");
    write_two_state_workflow(
        &wf,
        Some("task.md"),
        Some(&entity_md_with_status(
            "030",
            "My Task",
            "done",
            "main body",
        )),
    );
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_two_state_workflow(
        &wt,
        Some("task.md"),
        Some(&entity_md_with_status(
            "030",
            "My Task",
            "design",
            "worktree body with stage report",
        )),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(
        snapshot.items[0].status, "done",
        "status should come from main, got: {:?}",
        snapshot.items[0].status
    );
}

#[test]
fn worktree_body_from_worktree() {
    // AC-2: when main and worktree differ, body comes from the worktree copy
    // (which may contain the latest stage report not yet merged to main).
    let root = unique_temp_dir("wt-body-wt");
    let wf = root.join("docs/wf");
    write_two_state_workflow(
        &wf,
        Some("task.md"),
        Some(&entity_md_with_status(
            "031",
            "My Task",
            "done",
            "main body",
        )),
    );
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_two_state_workflow(
        &wt,
        Some("task.md"),
        Some(&entity_md_with_status(
            "031",
            "My Task",
            "design",
            "worktree body with stage report",
        )),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert!(
        snapshot.items[0]
            .body
            .contains("worktree body with stage report"),
        "body should come from worktree, got: {:?}",
        snapshot.items[0].body
    );
}

#[test]
fn no_worktree_unchanged() {
    // AC-3: when no worktree is present, both frontmatter and body come from main unchanged.
    let root = unique_temp_dir("no-wt-unchanged");
    let wf = root.join("docs/wf");
    write_two_state_workflow(
        &wf,
        Some("task.md"),
        Some(&entity_md_with_status(
            "032",
            "Solo Task",
            "done",
            "main only body",
        )),
    );
    assert!(!root.join(".worktrees").exists());

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].status, "done");
    assert!(
        snapshot.items[0].body.contains("main only body"),
        "body should come from main, got: {:?}",
        snapshot.items[0].body
    );
}

#[test]
fn same_content_hash_keeps_main_item_path() {
    // AC-4 inverse: same content hash → main item is kept (either is fine per spec)
    let root = unique_temp_dir("same-hash");
    let content = entity_md("020", "Identical");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, Some("task.md"), Some(&content));
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt, Some("task.md"), Some(&content));

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Identical");
    // When hashes match, the main copy is retained.
    assert!(snapshot.items[0].path.starts_with(&wf));
    // Identical content: not flagged as worktree-sourced; no main_body.
    assert!(snapshot.items[0].worktree_source.is_none());
    assert!(snapshot.items[0].main_body.is_none());
}

// ---- Task 038: worktree-only marker and body-diff merge data ----

#[test]
fn worktree_only_item_has_worktree_source_tag() {
    // AC-1: worktree-only item carries `worktree_source = Some(wt_path)` so
    // the UI can render a marker distinguishing it from main-tracked rows.
    let root = unique_temp_dir("wt-only-tag");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, None, None);
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt, Some("only.md"), Some(&entity_md("050", "Only")));

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    let item = &snapshot.items[0];
    assert_eq!(item.title, "Only");
    let wt_source = item
        .worktree_source
        .as_ref()
        .expect("worktree-only item must carry worktree_source");
    assert!(
        wt_source.starts_with(root.join(".worktrees/wt-1")),
        "worktree_source should point inside the worktree: {wt_source:?}"
    );
    // Worktree-only: no main_body (nothing to diff against).
    assert!(item.main_body.is_none());
}

#[test]
fn archived_main_slug_suppresses_stale_worktree_copy() {
    let root = unique_temp_dir("archived-stale-wt");
    let wf = root.join("docs/wf");
    write_two_state_workflow(
        &wf,
        Some("task-059.md"),
        Some(&entity_md_with_status(
            "059",
            "Archive Move",
            "design",
            "main body",
        )),
    );
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_two_state_workflow(
        &wt,
        Some("task-059.md"),
        Some(&entity_md_with_status(
            "059",
            "Archive Move",
            "design",
            "stale worktree body",
        )),
    );
    fs::create_dir_all(wf.join("_archive")).expect("archive dir");
    fs::rename(
        wf.join("task-059.md"),
        wf.join("_archive").join("task-059.md"),
    )
    .expect("archive move");

    let snapshot = load_workflow_dir(&wf, &root).expect("load active snapshot");

    assert!(
        snapshot.items.iter().all(|item| item.id != "059"),
        "archived slug must not be resurrected from worktree copy: {:?}",
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );

    let allowed = stage_names(&wf);
    let archived = load_archived_items(&wf, &allowed, None).expect("load archive");
    assert!(
        archived.iter().any(|item| item.id == "059"),
        "moved task should be available from archived scope"
    );
}

#[test]
fn archived_folder_slug_suppresses_stale_worktree_copy() {
    let root = unique_temp_dir("archived-folder-stale-wt");
    let wf = root.join("docs/wf");
    write_two_state_workflow(&wf, None, None);
    let archived_dir = wf.join("_archive/task-folder");
    fs::create_dir_all(&archived_dir).expect("archive folder");
    write_markdown(
        &archived_dir.join("index.md"),
        &entity_md_with_status("064", "Archived Folder", "done", "archived body"),
    );

    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_two_state_workflow(
        &wt,
        Some("task-folder.md"),
        Some(&entity_md_with_status(
            "064",
            "Stale Folder",
            "design",
            "stale worktree body",
        )),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load active snapshot");

    assert!(
        snapshot.items.iter().all(|item| item.id != "064"),
        "archived folder slug must not be resurrected from worktree copy: {:?}",
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn active_worktree_overlay_ignores_stale_archived_siblings() {
    let root = unique_temp_dir("archived-siblings-stale-wt");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, None, None);
    write_markdown(
        &wf.join("0x0c.md"),
        r#"---
id: "0x0c"
title: Root Active
status: design
source: captain bug report
worktree: .worktrees/0x0c-xxxx
issue: "123"
pr: "456"
---

root active body
"#,
    );
    write_markdown(
        &wf.join("_archive").join("0x0a.md"),
        r#"---
id: "0x0a"
title: Archived A
status: done
---

root archived A body
"#,
    );
    write_markdown(
        &wf.join("_archive").join("0x0b.md"),
        r#"---
id: "0x0b"
title: Archived B
status: done
---

root archived B body
"#,
    );
    let wt = root.join(".worktrees/0x0c-xxxx/docs/wf");
    write_minimal_workflow(&wt, None, None);
    write_markdown(
        &wt.join("0x0a.md"),
        &entity_md_with_status("0x0a", "Stale A", "design", "stale A body"),
    );
    write_markdown(
        &wt.join("0x0b.md"),
        &entity_md_with_status("0x0b", "Stale B", "design", "stale B body"),
    );
    write_markdown(
        &wt.join("0x0c.md"),
        &entity_md_with_status("0x0c", "Worktree Active", "done", "worktree active body"),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load active snapshot");

    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["0x0c"],
        "stale archived siblings must not reappear in active scope"
    );
    let active = &snapshot.items[0];
    assert_eq!(active.title, "Root Active");
    assert_eq!(active.status, "design");
    assert_eq!(active.source.as_deref(), Some("captain bug report"));
    assert_eq!(active.worktree.as_deref(), Some(".worktrees/0x0c-xxxx"));
    assert_eq!(active.issue.as_deref(), Some("123"));
    assert_eq!(active.pr.as_deref(), Some("456"));
    assert!(
        active.body.contains("worktree active body"),
        "active body should come from worktree copy: {:?}",
        active.body
    );
    assert!(
        active
            .main_body
            .as_deref()
            .is_some_and(|body| body.contains("root active body")),
        "main_body should preserve root body for diffing: {:?}",
        active.main_body
    );
    assert!(
        active
            .worktree_source
            .as_ref()
            .is_some_and(|path| path.starts_with(&wt)),
        "active worktree overlay should record source path: {:?}",
        active.worktree_source
    );

    let allowed = stage_names(&wf);
    let archived = load_archived_items(&wf, &allowed, None).expect("load archive");
    assert_eq!(
        archived
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["0x0a", "0x0b"],
        "archived scope should stay anchored to root archive files"
    );
}

#[test]
fn malformed_archived_slug_still_suppresses_stale_worktree_copy() {
    let root = unique_temp_dir("malformed-archived-stale-wt");
    let wf = root.join("docs/wf");
    write_two_state_workflow(&wf, None, None);
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_two_state_workflow(
        &wt,
        Some("task-060.md"),
        Some(&entity_md_with_status(
            "060",
            "Malformed Archive Move",
            "design",
            "stale worktree body",
        )),
    );
    write_markdown(
        &wf.join("_archive").join("task-060.md"),
        "---\nid: [\n---\n\nmalformed archive body\n",
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load active snapshot");

    assert!(
        snapshot.items.iter().all(|item| item.id != "060"),
        "malformed archived slug must still suppress stale worktree copy: {:?}",
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );

    let allowed = stage_names(&wf);
    let (_archived, errors) =
        load_archived_items_with_errors(&wf, &allowed, None).expect("load archive with errors");
    assert_eq!(errors.len(), 1, "archive parse error remains archive-owned");
}

#[test]
fn worktree_divergent_keeps_main_frontmatter_and_records_main_body() {
    // AC-2: when main and worktree differ, frontmatter (status, title) comes
    // from main, body comes from worktree, and main_body retains the root
    // body so the preview can render a diff.
    let root = unique_temp_dir("wt-divergent");
    let wf = root.join("docs/wf");
    write_two_state_workflow(
        &wf,
        Some("task.md"),
        Some(&entity_md_with_status(
            "060",
            "Main Title",
            "done",
            "main body line",
        )),
    );
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_two_state_workflow(
        &wt,
        Some("task.md"),
        Some(&entity_md_with_status(
            "060",
            "Worktree Title",
            "design",
            "worktree body line",
        )),
    );

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    let item = &snapshot.items[0];
    assert_eq!(item.status, "done", "status from main");
    assert_eq!(item.title, "Main Title", "title from main");
    assert!(
        item.body.contains("worktree body line"),
        "body from worktree: {:?}",
        item.body
    );
    assert!(
        item.main_body
            .as_deref()
            .is_some_and(|b| b.contains("main body line")),
        "main_body should retain root body for diffing, got: {:?}",
        item.main_body
    );
    assert!(
        item.worktree_source.is_some(),
        "divergent merge must tag worktree_source"
    );
}

#[test]
fn claude_worktrees_dir_is_scanned_alongside_dot_worktrees() {
    // AC-1: items mirrored under `.claude/worktrees/*` are picked up the same
    // way as items under `.worktrees/*`.
    let root = unique_temp_dir("claude-wt");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, None, None);
    let wt = root.join(".claude/worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt, Some("c.md"), Some(&entity_md("070", "Claude WT")));

    let snapshot = load_workflow_dir(&wf, &root).expect("load");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Claude WT");
    assert!(snapshot.items[0].worktree_source.is_some());
}

#[test]
fn worktree_scan_handles_no_worktrees_missing_subdir_and_partial_overlap() {
    // AC-5: three sub-cases — no worktree dirs, worktree missing the workflow
    // subdir, and a worktree with only an unrelated subset of task files.
    // Each case must succeed without errors and the merged item list must
    // match the root-only baseline plus any genuinely new worktree items.

    // (a) no worktrees registered at all.
    let root_a = unique_temp_dir("ac5-none");
    let wf_a = root_a.join("docs/wf");
    write_minimal_workflow(&wf_a, Some("alpha.md"), Some(&entity_md("080", "Alpha")));
    let snap_a = load_workflow_dir(&wf_a, &root_a).expect("(a) load");
    assert_eq!(snap_a.items.len(), 1);
    assert_eq!(snap_a.items[0].title, "Alpha");
    assert!(snap_a.items[0].worktree_source.is_none());

    // (b) worktree exists but does not contain the workflow directory.
    let root_b = unique_temp_dir("ac5-missing");
    let wf_b = root_b.join("docs/wf");
    write_minimal_workflow(&wf_b, Some("beta.md"), Some(&entity_md("081", "Beta")));
    fs::create_dir_all(root_b.join(".worktrees/wt-1/unrelated"))
        .expect("worktree without workflow dir");
    let snap_b = load_workflow_dir(&wf_b, &root_b).expect("(b) load");
    assert_eq!(snap_b.items.len(), 1);
    assert_eq!(snap_b.items[0].title, "Beta");
    assert!(snap_b.items[0].worktree_source.is_none());

    // (c) worktree exists, has the workflow dir, but the worktree's set of
    // task files does not overlap with the root.
    let root_c = unique_temp_dir("ac5-partial");
    let wf_c = root_c.join("docs/wf");
    write_minimal_workflow(&wf_c, Some("gamma.md"), Some(&entity_md("082", "Gamma")));
    let wt_c = root_c.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt_c, Some("delta.md"), Some(&entity_md("083", "Delta")));
    let snap_c = load_workflow_dir(&wf_c, &root_c).expect("(c) load");
    let titles: Vec<&str> = snap_c.items.iter().map(|i| i.title.as_str()).collect();
    assert!(titles.contains(&"Gamma"));
    assert!(titles.contains(&"Delta"));
    // gamma stays main-sourced; delta is worktree-only.
    let gamma = snap_c
        .items
        .iter()
        .find(|i| i.title == "Gamma")
        .expect("gamma");
    let delta = snap_c
        .items
        .iter()
        .find(|i| i.title == "Delta")
        .expect("delta");
    assert!(gamma.worktree_source.is_none());
    assert!(delta.worktree_source.is_some());
}

#[test]
fn worktree_scan_does_not_mutate_files() {
    // AC-4: snapshot file mtimes across root + worktrees, run load, assert
    // mtimes unchanged and no new files were created.
    let root = unique_temp_dir("ac4-mtime");
    let wf = root.join("docs/wf");
    write_minimal_workflow(&wf, Some("task.md"), Some(&entity_md("090", "Stable")));
    let wt = root.join(".worktrees/wt-1/docs/wf");
    write_minimal_workflow(&wt, Some("task.md"), Some(&entity_md("090", "Stable WT")));
    let claude_wt = root.join(".claude/worktrees/wt-2/docs/wf");
    write_minimal_workflow(&claude_wt, Some("only.md"), Some(&entity_md("091", "Only")));

    fn snapshot_tree(dir: &Path) -> Vec<(PathBuf, SystemTime, u64)> {
        let mut out = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, SystemTime, u64)>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let meta = fs::metadata(&path).expect("metadata");
                if meta.is_dir() {
                    walk(&path, out);
                } else {
                    out.push((path, meta.modified().expect("mtime"), meta.len()));
                }
            }
        }
        walk(dir, &mut out);
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    let before = snapshot_tree(&root);
    let _ = load_workflow_dir(&wf, &root).expect("load");
    let after = snapshot_tree(&root);

    assert_eq!(
        before.len(),
        after.len(),
        "no files should be created or deleted: before={before:?} after={after:?}"
    );
    for (a, b) in before.iter().zip(after.iter()) {
        assert_eq!(a.0, b.0, "file path stable");
        assert_eq!(a.2, b.2, "file size stable for {:?}", a.0);
        assert_eq!(a.1, b.1, "file mtime stable for {:?}", a.0);
    }
}

// ---- Task 042: malformed-frontmatter entities are surfaced, not fatal ----

/// Reproduction shape modeled on `rd-013.md`: a recognized key with an
/// unquoted multi-line value where one of the continuation lines contains an
/// unquoted `:`. The first failure mode trips strict `serde_yaml` (the colon
/// inside an inline mapping value is rejected). The presence of indented
/// continuation lines additionally trips the flat-line fallback's
/// `line.starts_with(' ')` guard, so neither parser path can rescue the
/// entity. The loader therefore sees a `MalformedYaml` error — the exact
/// surface the captain's repro hits.
const MALFORMED_FRONTMATTER_BODY: &str = "---
id: 042
title: Bad Entity
status: design
diff_summary: build the candidate set using a disjunction: WHERE foo = 1
  continuation line with another: colon problem
---

Body
";

fn write_workflow_readme(dir: &Path) {
    fs::create_dir_all(dir).expect("workflow dir");
    fs::write(
        dir.join("README.md"),
        "---\ncommissioned-by: spacedock@0.10.1\nstages:\n  states:\n    - name: design\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
    )
    .expect("write README");
}

#[test]
fn load_workflow_dir_skips_malformed_entity_and_records_error() {
    // AC-1: N valid + 1 malformed -> N items loaded + 1 parse_error captured.
    let dir = unique_temp_dir("malformed-entity");
    write_workflow_readme(&dir);
    write_markdown(&dir.join("good-1.md"), &entity_md("001", "Good One"));
    write_markdown(&dir.join("good-2.md"), &entity_md("002", "Good Two"));
    write_markdown(&dir.join("good-3.md"), &entity_md("003", "Good Three"));
    write_markdown(&dir.join("bad.md"), MALFORMED_FRONTMATTER_BODY);

    let snapshot = load_workflow_dir(&dir, &dir).expect("load with malformed entity is non-fatal");
    assert_eq!(
        snapshot.items.len(),
        3,
        "all valid items must still load; got: {:?}",
        snapshot
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        ["001", "002", "003"]
    );
    assert_eq!(
        snapshot.parse_errors.len(),
        1,
        "malformed entity must surface exactly one parse_error"
    );
    let err = &snapshot.parse_errors[0];
    assert!(
        err.path.ends_with("bad.md"),
        "parse_error path should reference the malformed file: {:?}",
        err.path
    );
    assert!(
        err.message.contains("malformed YAML frontmatter"),
        "parse_error message should describe the YAML failure: {}",
        err.message
    );
    let line = err
        .line
        .expect("YAML line should be derivable for MalformedYaml");
    let column = err
        .column
        .expect("YAML column should be derivable for MalformedYaml");
    assert!(line > 0, "line should be > 0; got {line}");
    assert!(column > 0, "column should be > 0; got {column}");
}

#[test]
fn load_workflow_dir_all_malformed_entities_loads_empty_with_broken_rows() {
    // Deliberate softening per the plan: when every entity is malformed, the
    // workflow still loads with `items = []` and `parse_errors.len() == N`,
    // rather than bailing out. AC-4 only mandates the README-malformed case
    // stays fatal. This documents the soft path.
    let dir = unique_temp_dir("all-malformed");
    write_workflow_readme(&dir);
    write_markdown(&dir.join("bad-1.md"), MALFORMED_FRONTMATTER_BODY);
    write_markdown(&dir.join("bad-2.md"), MALFORMED_FRONTMATTER_BODY);

    let snapshot = load_workflow_dir(&dir, &dir)
        .expect("workflow with all-malformed entities should still load with broken rows");
    assert!(snapshot.items.is_empty());
    assert_eq!(snapshot.parse_errors.len(), 2);
}

#[test]
fn load_workflow_dir_malformed_readme_still_fatal() {
    // AC-4: a malformed workflow README is still a hard top-level error.
    let dir = unique_temp_dir("bad-readme");
    fs::create_dir_all(&dir).expect("dir");
    // README frontmatter that fails to parse as YAML.
    fs::write(
        dir.join("README.md"),
        "---\nstages: [unterminated\n---\n\n# Workflow\n",
    )
    .expect("README");
    write_markdown(&dir.join("good.md"), &entity_md("001", "Good"));

    let err =
        load_workflow_dir(&dir, &dir).expect_err("malformed README must produce a top-level error");
    let display = err.to_string();
    assert!(
        matches!(err, ParseError::MalformedYaml { .. }),
        "expected MalformedYaml, got {err:?}"
    );
    assert!(
        display.contains("README.md"),
        "error must reference README.md: {display}"
    );
}

// ---- Slug-identity tests (id-style: slug) ----

fn slug_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/slug-workflow")
}

/// AC-2: a slug-style workflow whose entity carries a blank `id:` resolves the
/// effective ID to the filename stem, which is the value the overview ID column
/// renders. Also proves AC-1: the blank-id entity loads as a real item with no
/// `parse_errors` instead of crashing or becoming a broken row.
#[test]
fn loads_slug_workflow_uses_filename_as_id() {
    let root = slug_fixture_root();
    let snapshot = load_workflow_dir(&root, &root).expect("slug workflow should load");

    assert_eq!(snapshot.definition.id_style.as_deref(), Some("slug"));
    assert!(
        snapshot.parse_errors.is_empty(),
        "blank-id slug entity must not record a parse error: {:?}",
        snapshot.parse_errors
    );

    let item = snapshot
        .items
        .iter()
        .find(|item| item.title == "Roadmap v5")
        .expect("roadmap-v5 entity should be present");
    assert_eq!(
        item.id, "roadmap-v5",
        "blank id with id-style: slug resolves to the filename stem"
    );

    // AC-2 render: the overview ID column is `format!(\"{:>4}\", item.id)`
    // (src/ui/list.rs). Assert the formatted value equals the slug exactly —
    // this confirms {:>4} neither pads nor truncates a wide slug.
    let id_column = format!("{:>4}", item.id);
    assert_eq!(
        id_column, "roadmap-v5",
        "ID column should equal the slug unchanged; got {id_column:?}"
    );
}

/// AC-1: explicit no-crash assertion — loading a slug workflow with a blank
/// `id:` returns `Ok` and surfaces no per-entity parse error.
#[test]
fn slug_workflow_blank_id_does_not_error() {
    let root = slug_fixture_root();
    let snapshot =
        load_workflow_dir(&root, &root).expect("slug workflow must not error on blank id");
    assert!(
        snapshot.parse_errors.is_empty(),
        "expected no parse errors, got {:?}",
        snapshot.parse_errors
    );
    assert!(
        snapshot.items.iter().any(|item| item.id == "roadmap-v5"),
        "blank-id slug entity should appear as a loaded item"
    );
}

/// AC-3: `id-style` is read from README frontmatter for both declared values.
/// The committed slug fixture exercises `slug`; a temp workflow exercises
/// `sequential`.
#[test]
fn id_style_read_from_readme_for_both_values() {
    let slug = parse_workflow_readme(&slug_fixture_root().join("README.md"))
        .expect("slug README should parse");
    assert_eq!(slug.id_style.as_deref(), Some("slug"));

    let dir = unique_temp_dir("id-style-sequential");
    fs::write(
        dir.join("README.md"),
        "---\ncommissioned-by: spacedock@0.19.8\nid-style: sequential\nstages:\n  states:\n    - name: design\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
    )
    .expect("write sequential README");
    let sequential =
        parse_workflow_readme(&dir.join("README.md")).expect("sequential README should parse");
    assert_eq!(sequential.id_style.as_deref(), Some("sequential"));
}

/// AC-4: blank-id tolerance is scoped to `id-style: slug` only. A sequential
/// (default `id-style`) workflow whose entity has a populated id keeps that id,
/// and one with a blank id still fails with `MissingRequiredField { field: "id" }`.
#[test]
fn sequential_workflow_id_behavior_unaffected() {
    // Populated numeric id round-trips.
    let with_id = unique_temp_dir("seq-with-id");
    write_minimal_workflow(&with_id, Some("task.md"), Some(&entity_md("042", "Task")));
    let snapshot = load_workflow_dir(&with_id, &with_id).expect("sequential workflow should load");
    let item = snapshot
        .items
        .iter()
        .find(|item| item.title == "Task")
        .expect("task entity present");
    assert_eq!(item.id, "042");

    // Blank id in a non-slug workflow still errors.
    let blank = write_temp_markdown(
        "blank-id.md",
        "---\nid:\ntitle: Blank Id\nstatus: design\n---\n\nBody\n",
    );
    let err = parse_work_item(&blank, &["design".to_string()], None)
        .expect_err("blank id without id-style: slug must error");
    assert!(
        matches!(err, ParseError::MissingRequiredField { field: "id", .. }),
        "expected MissingRequiredField for id, got {err:?}"
    );

    // Even when id-style is explicitly sequential, blank id errors.
    let err_seq = parse_work_item(&blank, &["design".to_string()], Some("sequential"))
        .expect_err("blank id with id-style: sequential must error");
    assert!(
        matches!(
            err_seq,
            ParseError::MissingRequiredField { field: "id", .. }
        ),
        "expected MissingRequiredField for id under sequential, got {err_seq:?}"
    );
}

/// Write a README declaring `state: <state_rel>` (or single-root when `None`)
/// with `design`/`done` stages, returning the definition dir.
fn write_split_root_readme(def_dir: &Path, state_rel: Option<&str>) {
    fs::create_dir_all(def_dir).expect("definition dir");
    let state_line = state_rel
        .map(|s| format!("state: {s}\n"))
        .unwrap_or_default();
    let readme = format!(
        "---\ncommissioned-by: spacedock@0.10.1\n{state_line}stages:\n  states:\n    - name: design\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n"
    );
    fs::write(def_dir.join("README.md"), readme).expect("write README");
}

/// AC-2/AC-3 (primary regression). A split-root workflow — README with
/// `state: state-sub`, active + `_archive/` entities under `state-sub/`, and NO
/// entity files beside the README — renders its active AND archived entities.
/// Against `main` (which scans the README dir) both lists are empty, so this
/// test fails there and passes once entity-dir resolution lands.
#[test]
fn split_root_loads_active_and_archived_from_state_checkout() {
    use crate::sources::WorkflowSources;

    let root = unique_temp_dir("split-root");
    let def = root.join("docs/wf");
    write_split_root_readme(&def, Some(".spacedock-state"));
    let state = def.join(".spacedock-state");

    // Active entity lives in the state checkout, not beside the README.
    write_markdown(
        &state.join("active-task.md"),
        &entity_md("001", "Active Task"),
    );
    // Archived entity lives under the state checkout's `_archive/`.
    write_markdown(
        &state.join("_archive").join("done-task.md"),
        &entity_md_with_status("002", "Done Task", "done", "done body"),
    );

    let definition = parse_workflow_readme(&def.join("README.md")).expect("parse README");
    assert_eq!(definition.state.as_deref(), Some(".spacedock-state"));
    assert_eq!(definition.root, def, "definition root stays the README dir");

    let snapshot = load_workflow_dir(&def, &root).expect("load split-root workflow");
    let active_titles: Vec<&str> = snapshot.items.iter().map(|i| i.title.as_str()).collect();
    assert!(
        active_titles.contains(&"Active Task"),
        "split-root active items must load from the state checkout; got {active_titles:?}"
    );

    let archive = WorkflowSources::load_archive(&def, &definition);
    assert!(
        archive.error.is_none(),
        "archive load errored: {:?}",
        archive.error
    );
    let archived_titles: Vec<&str> = archive.entities.iter().map(|e| e.title.as_str()).collect();
    assert!(
        archived_titles.contains(&"Done Task"),
        "split-root archived items must load from the state checkout's _archive/; got {archived_titles:?}"
    );
}

/// AC-4 (single-root guard). `state: $inline` is the explicit single-root
/// sentinel: entities beside the README still load, and behavior is identical
/// to omitting `state:` entirely.
#[test]
fn inline_state_keeps_single_root_behavior() {
    use crate::sources::WorkflowSources;

    let root = unique_temp_dir("inline-state");
    let def = root.join("docs/wf");
    write_split_root_readme(&def, Some("$inline"));

    // Entities beside the README, as in any single-root workflow.
    write_markdown(
        &def.join("active-task.md"),
        &entity_md("001", "Active Task"),
    );
    write_markdown(
        &def.join("_archive").join("done-task.md"),
        &entity_md_with_status("002", "Done Task", "done", "done body"),
    );

    let definition = parse_workflow_readme(&def.join("README.md")).expect("parse README");
    let snapshot = load_workflow_dir(&def, &root).expect("load $inline workflow");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].title, "Active Task");

    let archive = WorkflowSources::load_archive(&def, &definition);
    assert_eq!(archive.entities.len(), 1);
    assert_eq!(archive.entities[0].title, "Done Task");
}
