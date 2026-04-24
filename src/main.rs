use clap::Parser;
use spacetop::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    spacetop::run(cli)
}
