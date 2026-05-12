use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    load_archived_items, load_workflow_dir, parse_work_item, parse_workflow_readme, ParseError,
};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/spacetop-dev")
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
        ["design", "plan", "implement", "review", "done"]
    );

    let design = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "design")
        .expect("design stage should exist");
    assert!(design.initial);
    assert!(!design.terminal);
    assert_eq!(design.concurrency, Some(2));

    let implement = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "implement")
        .expect("implement stage should exist");
    assert!(implement.worktree);
    assert_eq!(implement.concurrency, Some(2));

    let review = workflow
        .stages
        .iter()
        .find(|stage| stage.name == "review")
        .expect("review stage should exist");
    assert!(review.gate);
    assert!(review.fresh);
    assert_eq!(review.feedback_to.as_deref(), Some("implement"));

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
    let item = parse_work_item(&path, &allowed_statuses).expect("work item should parse");

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
    fs::copy(fixture_root().join("README.md"), root.join("README.md"))
        .expect("README fixture should copy");
    write_markdown(
        &root.join("active.md"),
        r#"---
id: "001"
title: Active
status: design
---

Active body.
"#,
    );
    write_markdown(
        &root.join("_mods/ignored.md"),
        r#"---
id: "002"
title: Ignored Mod
status: design
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
    let error = parse_work_item(&path, &["design".to_string()])
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
    let error = parse_work_item(&path, &["design".to_string(), "done".to_string()])
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
    let error = parse_work_item(&path, &["design".to_string()])
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
    let root = fixture_root();
    let allowed = stage_names(&root);
    let items = load_archived_items(&root, &allowed).expect("archive should load");

    assert!(items.len() >= 3, "expected at least 3 archived entries");
    let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
    assert!(titles.contains(&"Scaffold Rust CLI Project"));
    assert!(titles.contains(&"Parse Spacedock Workflow Files"));
    assert!(titles.contains(&"Build Initial TUI Overview"));
    assert!(items.iter().all(|item| item.status == "done"));
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

    let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
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

    let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Folder Entity");
}

#[test]
fn load_archived_items_missing_archive_dir_is_empty_ok() {
    let dir = unique_temp_dir("missing");
    let items = load_archived_items(&dir, &["done".to_string()]).expect("should be Ok");
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

    let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
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
<<<<<<< HEAD
status: validation
=======
status: done
>>>>>>> branch
---

Body
"#,
    );

    let items = load_archived_items(&dir, &["done".to_string()]).expect("archive load");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Good");
}

#[cfg(unix)]
#[test]
fn load_archived_items_returns_io_errors_instead_of_silently_skipping_them() {
    let dir = unique_temp_dir("archive-io-error");
    let archive = dir.join("_archive");
    fs::create_dir_all(&archive).expect("archive dir");
    std::os::unix::fs::symlink(archive.join("missing-target.md"), archive.join("broken.md"))
        .expect("symlink");

    let err =
        load_archived_items(&dir, &["done".to_string()]).expect_err("archive load should fail");
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
    let error = parse_work_item(&path, &["design".to_string()])
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
    write_minimal_workflow(
        &wt,
        Some("task.md"),
        Some(&entity_md("090", "Stable WT")),
    );
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
                    out.push((
                        path,
                        meta.modified().expect("mtime"),
                        meta.len(),
                    ));
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
