use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "spacetop",
    version,
    about = "Inspect Spacedock workflow state from the terminal.",
    long_about = "Spacetop is a read-only terminal UI for browsing Spacedock workflow state files."
)]
pub struct Cli {
    /// Path to a Spacedock workflow directory. When omitted, SpaceTop
    /// discovers workflows under the current git root.
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    List(ListArgs),
    Timeline(TimelineArgs),
    Metrics(WorkflowOutputArgs),
    Activity(WorkflowOutputArgs),
    Export(WorkflowOutputArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub text: Option<String>,
    #[arg(long)]
    pub scope: Option<ListScopeArg>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListScopeArg {
    Active,
    Archived,
    All,
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowOutputArgs {
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TimelineArgs {
    pub entity_id: String,
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::{CommandFactory, Parser};
    use std::path::PathBuf;

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_output_uses_workspace_package_version() {
        let version = Cli::command().render_version().to_string();

        assert!(
            version.contains(env!("CARGO_PKG_VERSION")),
            "version output `{version}` did not contain Cargo package version `{}`",
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn parses_workflow_dir() {
        let cli = Cli::parse_from(["spacetop", "--workflow-dir", "docs/spacetop-dev"]);

        assert_eq!(cli.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
    }

    #[test]
    fn parses_list_subcommand_with_filters() {
        let cli = Cli::parse_from([
            "spacetop",
            "list",
            "--workflow-dir",
            "docs/spacetop-dev",
            "--status",
            "verify",
            "--text",
            "sync",
            "--json",
        ]);

        match cli.command {
            Some(Command::List(args)) => {
                assert_eq!(args.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
                assert_eq!(args.status.as_deref(), Some("verify"));
                assert_eq!(args.text.as_deref(), Some("sync"));
                assert!(args.json);
            }
            other => panic!("expected list command, got {other:?}"),
        }
    }

    #[test]
    fn parses_top_level_workflow_dir_before_headless_subcommand() {
        let cli = Cli::parse_from([
            "spacetop",
            "--workflow-dir",
            "docs/spacetop-dev",
            "list",
            "--json",
        ]);

        assert_eq!(cli.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
        match cli.command {
            Some(Command::List(args)) => {
                assert!(args.workflow_dir.is_none());
                assert!(args.json);
            }
            other => panic!("expected list command, got {other:?}"),
        }
    }

    #[test]
    fn no_subcommand_still_launches_tui_shape() {
        let cli = Cli::parse_from(["spacetop", "--workflow-dir", "docs/spacetop-dev"]);

        assert!(cli.command.is_none());
        assert_eq!(cli.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
    }

    #[test]
    fn parses_timeline_subcommand() {
        let cli = Cli::parse_from([
            "spacetop",
            "timeline",
            "050",
            "--workflow-dir",
            "docs/spacetop-dev",
            "--json",
        ]);

        match cli.command {
            Some(Command::Timeline(args)) => {
                assert_eq!(args.entity_id, "050");
                assert_eq!(args.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
                assert!(args.json);
            }
            other => panic!("expected timeline command, got {other:?}"),
        }
    }

    #[test]
    fn parses_metrics_subcommand() {
        let cli = Cli::parse_from([
            "spacetop",
            "metrics",
            "--workflow-dir",
            "docs/spacetop-dev",
            "--json",
        ]);

        match cli.command {
            Some(Command::Metrics(args)) => {
                assert_eq!(args.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
                assert!(args.json);
            }
            other => panic!("expected metrics command, got {other:?}"),
        }
    }

    #[test]
    fn parses_activity_subcommand() {
        let cli = Cli::parse_from([
            "spacetop",
            "activity",
            "--workflow-dir",
            "docs/spacetop-dev",
            "--json",
        ]);

        match cli.command {
            Some(Command::Activity(args)) => {
                assert_eq!(args.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
                assert!(args.json);
            }
            other => panic!("expected activity command, got {other:?}"),
        }
    }

    #[test]
    fn parses_export_subcommand() {
        let cli = Cli::parse_from([
            "spacetop",
            "export",
            "--workflow-dir",
            "docs/spacetop-dev",
            "--json",
        ]);

        match cli.command {
            Some(Command::Export(args)) => {
                assert_eq!(args.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
                assert!(args.json);
            }
            other => panic!("expected export command, got {other:?}"),
        }
    }

    #[test]
    fn defaults_workflow_dir_to_none() {
        let cli = Cli::parse_from(["spacetop"]);

        assert!(cli.workflow_dir.is_none());
    }

    #[test]
    fn parses_short_w_alias() {
        let cli = Cli::parse_from(["spacetop", "-w", "docs/spacetop-dev"]);

        assert_eq!(cli.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
    }

    #[test]
    fn help_output_surfaces_both_spellings() {
        let help = Cli::command().render_help().to_string();

        assert!(
            help.contains("-w"),
            "help output missing short flag `-w`:\n{help}"
        );
        assert!(
            help.contains("--workflow-dir"),
            "help output missing long flag `--workflow-dir`:\n{help}"
        );
    }
}
