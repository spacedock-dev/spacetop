use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use spacetop_core::config::{self, ConfigLoad, ConfigWarning, DefaultScope, DefaultSort};
use spacetop_core::discovery;
use spacetop_core::git::{GitRunner, StdGitRunner};
use spacetop_core::index::{ActivityEvent, Metrics, StageEvent, WorkflowIndex};
use spacetop_core::query::{EntityQuery, EntitySort, HistoryUnavailable, QueryScope};
use spacetop_core::sources::WorkflowSources;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlessWorkflow {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub workflow_rel: String,
}

pub fn resolve_workflow_arg(
    workflow_dir: Option<PathBuf>,
    cwd: &Path,
) -> anyhow::Result<HeadlessWorkflow> {
    let requested = match workflow_dir {
        Some(path) if path.is_absolute() => path,
        Some(path) => cwd.join(path),
        None => cwd.to_path_buf(),
    }
    .canonicalize()
    .with_context(|| "failed to resolve workflow path")?;

    let workflows = discovery::discover_workflows(&requested)
        .with_context(|| format!("failed to scan {}", requested.display()))?;
    if workflows.len() != 1 {
        anyhow::bail!("headless command requires exactly one workflow; pass --workflow-dir <path>");
    }

    let workflow_dir = workflows[0].root.clone();
    let repo_root = discovery::resolve_scan_root(&workflow_dir);
    let workflow_rel = workflow_dir
        .strip_prefix(&repo_root)
        .map(path_to_git_rel)
        .unwrap_or_else(|_| workflow_dir.to_string_lossy().into_owned());

    Ok(HeadlessWorkflow {
        workflow_dir,
        repo_root,
        workflow_rel,
    })
}

pub fn run_command(command: crate::cli::Command) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let stderr = io::stderr();
    run_command_with_io(command, &mut stdout.lock(), &mut stderr.lock())
}

fn run_command_with_io(
    command: crate::cli::Command,
    out: &mut impl Write,
    err: &mut impl Write,
) -> anyhow::Result<()> {
    let config_load = load_headless_config();
    if !command_outputs_json(&command) {
        write_config_warnings(err, &config_load.warnings)?;
    }

    match command {
        crate::cli::Command::List(args) => run_list(args, &config_load.config, out),
        crate::cli::Command::Timeline(args) => run_timeline(args, &StdGitRunner, out),
        crate::cli::Command::Metrics(args) => run_metrics(args, &StdGitRunner, out),
        crate::cli::Command::Activity(args) => run_activity(args, &StdGitRunner, out),
    }
}

pub fn run_list(
    args: crate::cli::ListArgs,
    config: &spacetop_core::config::SpacetopConfig,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let resolved = resolve_workflow_arg(args.workflow_dir, &cwd)?;
    let mut index =
        WorkflowIndex::load(&resolved.workflow_dir, &resolved.repo_root).with_context(|| {
            format!(
                "failed to load workflow {}",
                resolved.workflow_dir.display()
            )
        })?;
    let scope = list_scope(args.scope, config.defaults.scope);
    if matches!(scope, QueryScope::Archived | QueryScope::All) {
        let archive = WorkflowSources::load_archive(&resolved.workflow_dir, index.definition());
        index = index.with_archive(archive);
    }

    let entities = index.query(EntityQuery {
        scope,
        status: args.status,
        text: args.text,
        sort: list_sort(config.defaults.sort),
        ..EntityQuery::default()
    });

    if args.json {
        serde_json::to_writer_pretty(&mut *out, &entities)?;
        writeln!(out)?;
    } else {
        for entity in entities {
            writeln!(out, "{}\t{}\t{}", entity.id, entity.status, entity.title)?;
        }
    }
    Ok(())
}

pub fn run_timeline<R: GitRunner>(
    args: crate::cli::TimelineArgs,
    runner: &R,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let json = args.json;
    let entity_id = args.entity_id.clone();
    let index = load_index_with_history(args.workflow_dir, runner)?;
    match index.timeline(&entity_id) {
        Ok(events) if json => write_json(out, &events),
        Ok(events) => write_timeline_text(out, &events),
        Err(reason) => write_unavailable(out, json, &reason),
    }
}

pub fn run_metrics<R: GitRunner>(
    args: crate::cli::WorkflowOutputArgs,
    runner: &R,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let json = args.json;
    let index = load_index_with_history(args.workflow_dir, runner)?;
    match index.metrics() {
        Ok(metrics) if json => write_json(out, &metrics),
        Ok(metrics) => write_metrics_text(out, &metrics),
        Err(reason) => write_unavailable(out, json, &reason),
    }
}

pub fn run_activity<R: GitRunner>(
    args: crate::cli::WorkflowOutputArgs,
    runner: &R,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let json = args.json;
    let index = load_index_with_history(args.workflow_dir, runner)?;
    match index.activity(None) {
        Ok(events) if json => write_json(out, &events),
        Ok(events) => write_activity_text(out, &events),
        Err(reason) => write_unavailable(out, json, &reason),
    }
}

fn load_index_with_history<R: GitRunner>(
    workflow_dir: Option<PathBuf>,
    runner: &R,
) -> anyhow::Result<WorkflowIndex> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let resolved = resolve_workflow_arg(workflow_dir, &cwd)?;
    WorkflowIndex::load_with_history(
        &resolved.workflow_dir,
        &resolved.repo_root,
        &resolved.workflow_rel,
        runner,
    )
    .with_context(|| {
        format!(
            "failed to load workflow {}",
            resolved.workflow_dir.display()
        )
    })
}

#[derive(serde::Serialize)]
struct UnavailableOutput<'a> {
    unavailable: &'a str,
}

fn write_unavailable(
    out: &mut impl Write,
    json: bool,
    reason: &HistoryUnavailable,
) -> anyhow::Result<()> {
    let message = reason.user_message();
    if json {
        write_json(
            out,
            &UnavailableOutput {
                unavailable: message,
            },
        )
    } else {
        writeln!(out, "{message}")?;
        Ok(())
    }
}

fn write_json<T: serde::Serialize>(out: &mut impl Write, value: &T) -> anyhow::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)?;
    Ok(())
}

fn write_timeline_text(out: &mut impl Write, events: &[StageEvent]) -> anyhow::Result<()> {
    if events.is_empty() {
        writeln!(out, "no timeline events")?;
        return Ok(());
    }
    for event in events {
        writeln!(
            out,
            "{}\t{}\t{}\t{}",
            event.at.0,
            event.entity_id,
            transition_label(event.from.as_deref(), &event.to),
            event.commit.0
        )?;
    }
    Ok(())
}

fn write_metrics_text(out: &mut impl Write, metrics: &Metrics) -> anyhow::Result<()> {
    writeln!(out, "completed_entities\t{}", metrics.completed_entities)?;
    writeln!(
        out,
        "throughput_completed\t{}",
        metrics.throughput_completed
    )?;
    write_i64_map(out, "stage_dwell_seconds", &metrics.stage_dwell_seconds)?;
    write_i64_map(out, "cycle_time_seconds", &metrics.cycle_time_seconds)?;
    write_usize_map(out, "wip_by_stage", &metrics.wip_by_stage)?;
    Ok(())
}

fn write_activity_text(out: &mut impl Write, events: &[ActivityEvent]) -> anyhow::Result<()> {
    if events.is_empty() {
        writeln!(out, "no activity events")?;
        return Ok(());
    }
    for activity in events {
        let event = &activity.event;
        writeln!(
            out,
            "{}\t{}\t{}\t{}",
            event.at.0,
            activity.entity_id,
            transition_label(event.from.as_deref(), &event.to),
            event.commit.0
        )?;
    }
    Ok(())
}

fn write_i64_map(
    out: &mut impl Write,
    label: &str,
    values: &std::collections::HashMap<String, i64>,
) -> anyhow::Result<()> {
    let mut rows: Vec<_> = values.iter().collect();
    rows.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in rows {
        writeln!(out, "{label}.{key}\t{value}")?;
    }
    Ok(())
}

fn write_usize_map(
    out: &mut impl Write,
    label: &str,
    values: &std::collections::HashMap<String, usize>,
) -> anyhow::Result<()> {
    let mut rows: Vec<_> = values.iter().collect();
    rows.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in rows {
        writeln!(out, "{label}.{key}\t{value}")?;
    }
    Ok(())
}

fn transition_label(from: Option<&str>, to: &str) -> String {
    match from {
        Some(from) => format!("{from}->{to}"),
        None => format!("(new)->{to}"),
    }
}

fn list_scope(
    cli_scope: Option<crate::cli::ListScopeArg>,
    default_scope: DefaultScope,
) -> QueryScope {
    match cli_scope {
        Some(crate::cli::ListScopeArg::Active) => QueryScope::Active,
        Some(crate::cli::ListScopeArg::Archived) => QueryScope::Archived,
        Some(crate::cli::ListScopeArg::All) => QueryScope::All,
        None => match default_scope {
            DefaultScope::Active => QueryScope::Active,
            DefaultScope::Archived => QueryScope::Archived,
        },
    }
}

fn list_sort(default_sort: DefaultSort) -> EntitySort {
    match default_sort {
        DefaultSort::Id => EntitySort::Id,
        DefaultSort::Status => EntitySort::Status,
    }
}

fn load_headless_config() -> ConfigLoad {
    match config::load_config_with_warnings(&config::StdEnv) {
        Ok(load) => load,
        Err(err) => ConfigLoad {
            config: spacetop_core::config::SpacetopConfig::default(),
            warnings: vec![ConfigWarning {
                message: format!("failed to load config: {err}"),
            }],
        },
    }
}

fn command_outputs_json(command: &crate::cli::Command) -> bool {
    match command {
        crate::cli::Command::List(args) => args.json,
        crate::cli::Command::Timeline(args) => args.json,
        crate::cli::Command::Metrics(args) => args.json,
        crate::cli::Command::Activity(args) => args.json,
    }
}

fn write_config_warnings(out: &mut impl Write, warnings: &[ConfigWarning]) -> io::Result<()> {
    for warning in warnings {
        writeln!(out, "spacetop: {}", warning.message)?;
    }
    Ok(())
}

fn path_to_git_rel(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn explicit_workflow_path_canonicalizes_and_resolves_direct_workflow() {
        let repo = fixture_repo_with_one_workflow();
        let path = repo.path().join("docs/workflow");

        let resolved = resolve_workflow_arg(Some(path.clone()), repo.path()).expect("resolve");

        assert_eq!(
            resolved.workflow_dir,
            path.canonicalize().expect("canonical")
        );
        assert_eq!(
            resolved.repo_root,
            repo.path().canonicalize().expect("repo")
        );
        assert_eq!(resolved.workflow_rel, "docs/workflow");
    }

    #[test]
    fn explicit_scan_root_must_discover_exactly_one_workflow() {
        let repo = fixture_repo_with_one_workflow();

        let resolved =
            resolve_workflow_arg(Some(repo.path().to_path_buf()), repo.path()).expect("resolve");

        assert!(resolved.workflow_dir.ends_with("docs/workflow"));
        assert_eq!(
            resolved.repo_root,
            repo.path().canonicalize().expect("repo")
        );
        assert_eq!(resolved.workflow_rel, "docs/workflow");
    }

    #[test]
    fn omitted_path_rejects_zero_or_multiple_workflows() {
        let empty = tempfile::tempdir().expect("tempdir");
        let err = resolve_workflow_arg(None, empty.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("headless command requires exactly one workflow"));

        let repo = fixture_repo_with_two_workflows();
        let err = resolve_workflow_arg(None, repo.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("headless command requires exactly one workflow"));
    }

    #[test]
    fn list_json_outputs_entities() {
        let workflow =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/slug-workflow");
        let mut out = Vec::new();

        run_list(
            crate::cli::ListArgs {
                workflow_dir: Some(workflow),
                status: None,
                text: Some("roadmap".to_string()),
                scope: None,
                json: true,
            },
            &spacetop_core::config::SpacetopConfig::default(),
            &mut out,
        )
        .expect("list");

        let body = String::from_utf8(out).expect("utf8");
        assert!(body.contains("\"id\""));
        assert!(body.contains("roadmap-v5"));
    }

    #[test]
    fn history_commands_emit_shallow_clone_unavailable() {
        assert_history_unavailable(
            git_ok("true\n"),
            spacetop_core::query::HistoryUnavailable::ShallowClone.user_message(),
        );
    }

    #[test]
    fn history_commands_emit_not_git_unavailable() {
        assert_history_unavailable(
            git_err(128, "fatal: not a git repository\n"),
            spacetop_core::query::HistoryUnavailable::NotGitRepository.user_message(),
        );
    }

    #[test]
    fn history_commands_emit_git_error_unavailable() {
        assert_history_unavailable(
            git_err(128, "fatal: bad object\n"),
            spacetop_core::query::HistoryUnavailable::GitError("fatal: bad object\n".to_string())
                .user_message(),
        );
    }

    fn assert_history_unavailable(response: spacetop_core::git::GitCmdResult, message: &str) {
        assert_history_unavailable_json(response.clone(), message);
        assert_history_unavailable_text(response, message);
    }

    fn assert_history_unavailable_json(response: spacetop_core::git::GitCmdResult, message: &str) {
        let expected = format!("{{\n  \"unavailable\": \"{message}\"\n}}\n");

        assert_eq!(
            run_timeline_body(response.clone(), true),
            expected,
            "timeline JSON unavailable output"
        );
        assert_eq!(
            run_metrics_body(response.clone(), true),
            expected,
            "metrics JSON unavailable output"
        );
        assert_eq!(
            run_activity_body(response, true),
            expected,
            "activity JSON unavailable output"
        );
    }

    fn assert_history_unavailable_text(response: spacetop_core::git::GitCmdResult, message: &str) {
        let expected = format!("{message}\n");

        assert_eq!(
            run_timeline_body(response.clone(), false),
            expected,
            "timeline text unavailable output"
        );
        assert_eq!(
            run_metrics_body(response.clone(), false),
            expected,
            "metrics text unavailable output"
        );
        assert_eq!(
            run_activity_body(response, false),
            expected,
            "activity text unavailable output"
        );
    }

    fn run_timeline_body(response: spacetop_core::git::GitCmdResult, json: bool) -> String {
        let fixture = fixture_repo_with_one_workflow();
        write_entity(&fixture.path().join("docs/workflow/001.md"));
        let runner = TestGitRunner::new(response);
        let mut out = Vec::new();

        run_timeline(
            crate::cli::TimelineArgs {
                entity_id: "001".to_string(),
                workflow_dir: Some(fixture.path().join("docs/workflow")),
                json,
            },
            &runner,
            &mut out,
        )
        .expect("timeline");

        String::from_utf8(out).expect("utf8")
    }

    fn run_metrics_body(response: spacetop_core::git::GitCmdResult, json: bool) -> String {
        let fixture = fixture_repo_with_one_workflow();
        write_entity(&fixture.path().join("docs/workflow/001.md"));
        let runner = TestGitRunner::new(response);
        let mut out = Vec::new();

        run_metrics(
            crate::cli::WorkflowOutputArgs {
                workflow_dir: Some(fixture.path().join("docs/workflow")),
                json,
            },
            &runner,
            &mut out,
        )
        .expect("metrics");

        String::from_utf8(out).expect("utf8")
    }

    fn run_activity_body(response: spacetop_core::git::GitCmdResult, json: bool) -> String {
        let fixture = fixture_repo_with_one_workflow();
        write_entity(&fixture.path().join("docs/workflow/001.md"));
        let runner = TestGitRunner::new(response);
        let mut out = Vec::new();

        run_activity(
            crate::cli::WorkflowOutputArgs {
                workflow_dir: Some(fixture.path().join("docs/workflow")),
                json,
            },
            &runner,
            &mut out,
        )
        .expect("activity");

        String::from_utf8(out).expect("utf8")
    }

    fn fixture_repo_with_one_workflow() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".git")).expect("git dir");
        write_workflow(&repo.path().join("docs/workflow"));
        repo
    }

    fn fixture_repo_with_two_workflows() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(repo.path().join(".git")).expect("git dir");
        write_workflow(&repo.path().join("docs/alpha"));
        write_workflow(&repo.path().join("docs/beta"));
        repo
    }

    fn write_workflow(dir: &Path) {
        std::fs::create_dir_all(dir).expect("workflow dir");
        std::fs::write(
            dir.join("README.md"),
            "---\ncommissioned-by: spacedock@test\nstages:\n  states:\n    - name: plan\n      initial: true\n    - name: done\n      terminal: true\n---\n\n# Workflow\n",
        )
        .expect("write readme");
    }

    fn write_entity(path: &Path) {
        std::fs::write(
            path,
            "---\nid: \"001\"\ntitle: First\nstatus: plan\n---\n\nbody\n",
        )
        .expect("write entity");
    }

    struct TestGitRunner {
        response: spacetop_core::git::GitCmdResult,
    }

    impl TestGitRunner {
        fn new(response: spacetop_core::git::GitCmdResult) -> Self {
            Self { response }
        }
    }

    impl spacetop_core::git::GitRunner for TestGitRunner {
        fn run(
            &self,
            _repo_root: &Path,
            _args: &[&str],
        ) -> std::io::Result<spacetop_core::git::GitCmdResult> {
            Ok(self.response.clone())
        }
    }

    fn git_ok(stdout: &str) -> spacetop_core::git::GitCmdResult {
        spacetop_core::git::GitCmdResult {
            status: exit_status(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    fn git_err(code: i32, stderr: &str) -> spacetop_core::git::GitCmdResult {
        spacetop_core::git::GitCmdResult {
            status: exit_status(code),
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }

    #[cfg(unix)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;

        std::process::ExitStatus::from_raw(code << 8)
    }
}
