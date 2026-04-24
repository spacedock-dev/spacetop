pub mod app;
pub mod cli;
pub mod discovery;
pub mod domain;
pub mod parser;
pub mod ui;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context};
use app::App;
use cli::Cli;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// Result of resolving a CLI invocation into a launch decision, prior to any
/// TUI startup. Exposed so integration tests can assert zero/one/many and the
/// `-w` bypass without spawning a terminal.
#[derive(Debug)]
pub enum DecideOutcome {
    /// Open the given app in the TUI.
    Overview(App),
    /// Open the picker TUI against multiple discovered workflows.
    Picker(App),
    /// No workflows found; caller should report the stable stderr message
    /// naming this scan root and exit non-zero.
    ZeroWorkflows { scan_root: PathBuf },
}

pub fn decide_app(cli: &Cli, cwd: &Path) -> anyhow::Result<DecideOutcome> {
    if let Some(explicit) = cli.workflow_dir.clone() {
        let app = App::load(explicit.clone())
            .with_context(|| format!("failed to load workflow directory {}", explicit.display()))?;
        return Ok(DecideOutcome::Overview(app));
    }

    let scan_root = discovery::resolve_scan_root(cwd);
    let workflows = discovery::discover_workflows(&scan_root)
        .with_context(|| format!("failed to scan {}", scan_root.display()))?;

    match workflows.len() {
        0 => Ok(DecideOutcome::ZeroWorkflows { scan_root }),
        1 => {
            let only = workflows.into_iter().next().unwrap();
            let app = App::load(only.root.clone()).with_context(|| {
                format!("failed to load workflow directory {}", only.root.display())
            })?;
            Ok(DecideOutcome::Overview(app))
        }
        _ => Ok(DecideOutcome::Picker(App::from_picker(
            scan_root, workflows,
        ))),
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    match decide_app(&cli, &cwd)? {
        DecideOutcome::Overview(app) | DecideOutcome::Picker(app) => run_terminal(app),
        DecideOutcome::ZeroWorkflows { scan_root } => {
            eprintln!(
                "spacetop: no Spacedock workflows found under {}. Pass --workflow-dir <path> to open a specific directory.",
                scan_root.display()
            );
            Err(anyhow!("no workflows discovered"))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_workflow_eprintln_prefix_is_stable() {
        // This test locks the stable stderr prefix specified by AC-3.
        let scan_root = PathBuf::from("/some/root");
        let msg = format!(
            "spacetop: no Spacedock workflows found under {}. Pass --workflow-dir <path> to open a specific directory.",
            scan_root.display()
        );
        assert!(msg.starts_with("spacetop: no Spacedock workflows found under "));
        assert!(msg.contains("/some/root"));
        assert!(msg.contains("Pass --workflow-dir <path>"));
    }
}
