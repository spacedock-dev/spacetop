use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use spacetop_core::config::{self, ConfigLoad, ConfigWarning, DefaultScope, DefaultSort};
use spacetop_core::discovery;
use spacetop_core::index::WorkflowIndex;
use spacetop_core::query::{EntityQuery, EntitySort, QueryScope};
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
        anyhow::bail!(
            "headless command requires exactly one workflow; pass --workflow-dir <path>"
        );
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
    }
}

pub fn run_list(
    args: crate::cli::ListArgs,
    config: &spacetop_core::config::SpacetopConfig,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let resolved = resolve_workflow_arg(args.workflow_dir, &cwd)?;
    let mut index = WorkflowIndex::load(&resolved.workflow_dir, &resolved.repo_root)
        .with_context(|| format!("failed to load workflow {}", resolved.workflow_dir.display()))?;
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

        assert_eq!(resolved.workflow_dir, path.canonicalize().expect("canonical"));
        assert_eq!(resolved.repo_root, repo.path().canonicalize().expect("repo"));
        assert_eq!(resolved.workflow_rel, "docs/workflow");
    }

    #[test]
    fn explicit_scan_root_must_discover_exactly_one_workflow() {
        let repo = fixture_repo_with_one_workflow();

        let resolved =
            resolve_workflow_arg(Some(repo.path().to_path_buf()), repo.path()).expect("resolve");

        assert!(resolved.workflow_dir.ends_with("docs/workflow"));
        assert_eq!(resolved.repo_root, repo.path().canonicalize().expect("repo"));
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
}
