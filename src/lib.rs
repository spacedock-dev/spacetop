pub mod app;
pub mod cli;
pub mod domain;
pub mod ui;

use app::App;
use cli::Cli;

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let _app = App::new(cli.workflow_dir);

    Ok(())
}
