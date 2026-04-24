pub mod app;
pub mod cli;
pub mod domain;
pub mod parser;
pub mod ui;

use std::io;
use std::time::Duration;

use anyhow::Context;
use app::App;
use cli::Cli;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let app = App::load(cli.workflow_dir.clone()).with_context(|| {
        format!(
            "failed to load workflow directory {}",
            cli.workflow_dir.display()
        )
    })?;
    run_terminal(app)
}

fn run_terminal(mut app: App) -> anyhow::Result<()> {
    enable_raw_mode().context("failed to enable terminal raw mode")?;
    let _restore = TerminalRestore;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;

    loop {
        terminal
            .draw(|frame| ui::render(frame, &app))
            .context("failed to draw terminal UI")?;

        if app.should_quit() {
            break;
        }

        if event::poll(Duration::from_millis(250)).context("failed to poll terminal events")? {
            if let Event::Key(key) = event::read().context("failed to read terminal event")? {
                app.handle_key(key);
            }
        }
    }

    terminal
        .show_cursor()
        .context("failed to restore terminal cursor")?;

    Ok(())
}

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
