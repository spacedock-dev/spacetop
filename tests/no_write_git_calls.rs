//! AC-5: Spacetop's sync feature must NOT introduce any code path that
//! mutates the workflow tree beyond what `git pull --ff-only` itself
//! writes. This guardrail test walks the `src/` tree and asserts that no
//! production source file references the disallowed write subcommands
//! (`push`, `commit`, `checkout`) anywhere near a git invocation, while
//! `--ff-only` does appear exactly once (in the sync helper).
//!
//! This is a static-source assertion, not a runtime behavior test; it
//! keeps the read-only safety property enforceable in CI.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

fn src_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("rs")
        })
        .map(|e| e.into_path())
        .collect()
}

/// Strip line comments so a documentation reference to a disallowed
/// subcommand (e.g. "no `git push`...") doesn't trip the grep.
fn strip_line_comments(s: &str) -> String {
    s.lines()
        .map(|line| line.split("//").next().unwrap_or("").to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// True when `haystack` contains any of `needles` outside of line
/// comments. We deliberately keep the check coarse — string-literal
/// quotes around the subcommand are the realistic surface area, and any
/// future expansion (a Command::arg call) would still hit a literal.
fn contains_any_outside_comments(haystack: &str, needles: &[&str]) -> Option<&'static str> {
    static_assert_disallowed(needles);
    let stripped = strip_line_comments(haystack);
    for n in needles {
        if stripped.contains(n) {
            // Re-resolve the matched needle to a 'static reference so we
            // can return it. The `needles` slice carries 'static literals.
            for s in DISALLOWED {
                if s == n {
                    return Some(s);
                }
            }
        }
    }
    None
}

const DISALLOWED: &[&str] = &["\"push\"", "\"commit\"", "\"checkout\""];

fn static_assert_disallowed(needles: &[&str]) {
    for n in needles {
        assert!(
            DISALLOWED.contains(n),
            "needle {n:?} must be listed in DISALLOWED so it round-trips to a 'static reference"
        );
    }
}

#[test]
fn src_tree_does_not_reference_disallowed_git_write_subcommands() {
    let files = rust_files(&src_root());
    assert!(
        !files.is_empty(),
        "expected to find some .rs files under src/"
    );
    let mut offenders: Vec<(PathBuf, &'static str)> = Vec::new();
    for path in &files {
        let body = std::fs::read_to_string(path).expect("read source file");
        if let Some(hit) = contains_any_outside_comments(&body, DISALLOWED) {
            offenders.push((path.clone(), hit));
        }
    }
    assert!(
        offenders.is_empty(),
        "AC-5: disallowed git write subcommands appear in source: {offenders:?}"
    );
}

#[test]
fn src_tree_references_ff_only_exactly_once() {
    let files = rust_files(&src_root());
    let mut count = 0usize;
    let mut hits: Vec<PathBuf> = Vec::new();
    for path in &files {
        let body = std::fs::read_to_string(path).expect("read source file");
        let body = strip_line_comments(&body);
        let n = body.matches("--ff-only").count();
        if n > 0 {
            count += n;
            hits.push(path.clone());
        }
    }
    assert_eq!(
        count, 1,
        "expected exactly one --ff-only reference in src/, found {count} in {hits:?}"
    );
}
