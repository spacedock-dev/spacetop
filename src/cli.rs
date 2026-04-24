use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "spacetop",
    about = "Inspect Spacedock workflow state from the terminal.",
    long_about = "SpaceTop is a read-only terminal UI for browsing Spacedock workflow state files."
)]
pub struct Cli {
    /// Path to a Spacedock workflow directory.
    #[arg(long, value_name = "PATH", default_value = ".")]
    pub workflow_dir: PathBuf,
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
    fn parses_workflow_dir() {
        let cli = Cli::parse_from(["spacetop", "--workflow-dir", "docs/spacetop-dev"]);

        assert_eq!(cli.workflow_dir, PathBuf::from("docs/spacetop-dev"));
    }

    #[test]
    fn defaults_workflow_dir_to_current_directory() {
        let cli = Cli::parse_from(["spacetop"]);

        assert_eq!(cli.workflow_dir, PathBuf::from("."));
    }
}
