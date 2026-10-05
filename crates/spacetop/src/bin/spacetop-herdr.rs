use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Open the invoking Herdr project's Spacetop sidecar")]
struct Cli {
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand)]
enum Action {
    Open,
    Pane,
}

fn main() {
    let action = Cli::parse().action;
    let keep_error_visible = matches!(action, Action::Pane);
    let result = match action {
        Action::Open => spacetop::herdr_sidecar::open(),
        Action::Pane => spacetop::herdr_sidecar::pane(),
    };
    if let Err(error) = result {
        eprintln!("spacetop-herdr: {error:#}");
        use std::io::IsTerminal;
        if keep_error_visible && std::io::stdin().is_terminal() {
            eprintln!("Press Enter to close this inspector, then correct the error and reopen it.");
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
        }
        std::process::exit(1);
    }
}
