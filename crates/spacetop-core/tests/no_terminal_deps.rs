//! Guard: spacetop-core must never link a terminal-UI crate. The headless/export
//! surface depends on this boundary. Uses `cargo tree` so transitive deps count.

use std::process::Command;

const FORBIDDEN: [&str; 4] = ["ratatui", "crossterm", "termimad", "ratskin"];

#[test]
fn core_dependency_tree_has_no_terminal_crates() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "spacetop-core",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run `cargo tree`");

    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tree = String::from_utf8_lossy(&output.stdout);
    let offenders: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|crate_name| {
            tree.lines().any(|line| {
                // `--prefix none` lines look like "ratatui v0.30.0".
                line.split_whitespace().next() == Some(crate_name)
            })
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "spacetop-core must not depend on terminal crates, found: {offenders:?}"
    );
}
