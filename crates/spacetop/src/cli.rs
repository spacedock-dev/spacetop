use std::path::PathBuf;

use clap::Parser;

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
}

#[cfg(test)]
mod tests {
    use super::Cli;
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
