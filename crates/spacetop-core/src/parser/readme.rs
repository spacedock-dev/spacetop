use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::domain::{StageDefinition, StageTransition, WorkflowDefinition};

use super::frontmatter::extract_frontmatter;
use super::{display_path, required, ParseError};

pub fn parse_workflow_readme(path: &Path) -> Result<WorkflowDefinition, ParseError> {
    let path_label = display_path(path);
    let contents = fs::read_to_string(path).map_err(|source| ParseError::ReadFile {
        path: path_label.clone(),
        source,
    })?;
    let (frontmatter, _) = extract_frontmatter(&contents, &path_label)?;
    let raw: RawWorkflowFrontmatter =
        serde_yaml::from_str(frontmatter).map_err(|source| ParseError::MalformedYaml {
            path: path_label.clone(),
            source,
        })?;

    let stage_block = raw.stages.ok_or(ParseError::MissingRequiredField {
        path: path_label,
        field: "stages",
    })?;
    let defaults = stage_block.defaults.unwrap_or_default();
    let mut stages = Vec::with_capacity(stage_block.states.len());
    for raw_stage in stage_block.states {
        let name = required(raw_stage.name, path, "stages.states.name")?;
        stages.push(StageDefinition {
            name,
            initial: raw_stage
                .initial
                .unwrap_or(defaults.initial.unwrap_or(false)),
            terminal: raw_stage
                .terminal
                .unwrap_or(defaults.terminal.unwrap_or(false)),
            gate: raw_stage.gate.unwrap_or(defaults.gate.unwrap_or(false)),
            fresh: raw_stage.fresh.unwrap_or(defaults.fresh.unwrap_or(false)),
            feedback_to: raw_stage.feedback_to.or(defaults.feedback_to.clone()),
            worktree: raw_stage
                .worktree
                .unwrap_or(defaults.worktree.unwrap_or(false)),
            concurrency: raw_stage.concurrency.or(defaults.concurrency),
        });
    }

    let stage_colors = crate::domain::assign_stage_colors(&stages);
    let stage_prose = parse_stage_prose(&contents);
    let transitions = stage_block
        .transitions
        .unwrap_or_default()
        .into_iter()
        .filter_map(|raw| match (raw.from, raw.to) {
            (Some(from), Some(to)) => Some(StageTransition {
                from,
                to,
                label: raw.label,
            }),
            _ => None,
        })
        .collect();
    Ok(WorkflowDefinition {
        root: path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        stages,
        id_style: raw.id_style,
        entity_type: raw.entity_type,
        entity_label: raw.entity_label,
        entity_label_plural: raw.entity_label_plural,
        stage_colors,
        stage_prose,
        transitions,
    })
}

/// Pure prose extractor for the per-stage `### {stage}` blocks under
/// the README's `## Stages` section. The input is the full README text
/// (YAML frontmatter is stripped here using `split_frontmatter`); the
/// output is a name → raw markdown body map. The body is the raw
/// substring between the `### {stage}` line and the next line that
/// begins with `#`, `##`, or `###` followed by whitespace (i.e. a
/// heading of equal-or-higher level).
///
/// Behavior:
/// - No `fs::*`, no `&Path`. Pure `(&str) -> HashMap`.
/// - The stage name is the heading text trimmed of whitespace AND any
///   surrounding backticks (so `### \`design\`` becomes key `"design"`).
/// - The body is stored verbatim, preserving newlines, with no
///   normalisation or rewriting. Trailing newlines that immediately
///   precede the next heading are trimmed so the body's final visible
///   text isn't followed by an empty paragraph.
/// - Stage names appearing in prose but not in frontmatter are silently
///   retained in the returned map. The renderer ignores them.
/// - If the input has no frontmatter the whole document is scanned.
pub(crate) fn parse_stage_prose(readme_contents: &str) -> HashMap<String, String> {
    use super::frontmatter::{split_frontmatter, SplitFrontmatter};

    let body = match split_frontmatter(readme_contents) {
        Some(SplitFrontmatter::Ok { body, .. }) => body,
        _ => readme_contents,
    };

    let mut out: HashMap<String, String> = HashMap::new();
    let mut current: Option<(Vec<String>, String)> = None;

    for line in body.lines() {
        if let Some(names) = stage_heading_names(line) {
            // Close the previous block if any.
            if let Some((names, mut prose)) = current.take() {
                trim_trailing_blank_lines(&mut prose);
                for name in names {
                    out.insert(name, prose.clone());
                }
            }
            current = Some((
                names.into_iter().map(str::to_string).collect(),
                String::new(),
            ));
            continue;
        }
        // A heading of any level (h1/h2/h3+) closes the current stage.
        // We already handled `### …` above as a stage heading; any other
        // `#`-prefixed heading closes whatever stage we were collecting.
        if is_any_heading(line) {
            if let Some((names, mut prose)) = current.take() {
                trim_trailing_blank_lines(&mut prose);
                for name in names {
                    out.insert(name, prose.clone());
                }
            }
            continue;
        }
        if let Some((_, prose)) = current.as_mut() {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    if let Some((names, mut prose)) = current.take() {
        trim_trailing_blank_lines(&mut prose);
        for name in names {
            out.insert(name, prose.clone());
        }
    }
    out
}

/// Returns `Some(names)` when `line` is an `### {names}` ATX heading,
/// where `names` is the list of backtick-wrapped tokens found on the
/// heading line (in source order). If the heading has no backticks at
/// all, the whole trimmed remainder is returned as a single element —
/// preserving the legacy `### plan` form.
///
/// Examples:
/// - `` ### `scoping` (lead only, worktree) `` → `Some(vec!["scoping"])`
/// - `` ### `expanded` / `ideated` / `done` / `rejected` `` →
///   `Some(vec!["expanded", "ideated", "done", "rejected"])`
/// - `### plan` → `Some(vec!["plan"])`
/// - `### \`unterminated` → `None` (malformed input, dropped silently)
fn stage_heading_names(line: &str) -> Option<Vec<&str>> {
    let rest = line.strip_prefix("### ")?;
    // Reject anything that opens another heading marker after the level-3
    // prefix (e.g. `#### foo` would have been caught earlier; this is
    // defence in depth).
    if rest.starts_with('#') {
        return None;
    }
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains('`') {
        // Walk byte indices and collect every backtick-delimited token.
        let bytes = trimmed.as_bytes();
        let mut names: Vec<&str> = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'`' {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b'`' {
                    j += 1;
                }
                if j >= bytes.len() {
                    // Unterminated backtick — malformed heading, drop it.
                    return None;
                }
                names.push(&trimmed[start..j]);
                i = j + 1;
            } else {
                i += 1;
            }
        }
        if names.is_empty() {
            None
        } else {
            Some(names)
        }
    } else {
        // No backticks at all: fall back to the entire trimmed line as a
        // single name. Preserves AC-2 for the legacy `### plan` form.
        Some(vec![trimmed])
    }
}

/// Returns true if the line looks like an ATX markdown heading at any
/// level (`#`, `##`, `###`, …) — i.e. starts with one or more `#`
/// characters followed by a space.
fn is_any_heading(line: &str) -> bool {
    let mut chars = line.chars();
    let mut saw_hash = false;
    for c in chars.by_ref() {
        if c == '#' {
            saw_hash = true;
            continue;
        }
        return saw_hash && c == ' ';
    }
    false
}

fn trim_trailing_blank_lines(s: &mut String) {
    while s.ends_with("\n\n") {
        s.pop();
    }
    // Drop the single trailing newline so re-serialisation matches the
    // input's final line layout when the body ends with a newline.
    if s.ends_with('\n') {
        s.pop();
    }
}

#[derive(Debug, Deserialize)]
struct RawWorkflowFrontmatter {
    #[serde(rename = "id-style")]
    id_style: Option<String>,
    #[serde(rename = "entity-type")]
    entity_type: Option<String>,
    #[serde(rename = "entity-label")]
    entity_label: Option<String>,
    #[serde(rename = "entity-label-plural")]
    entity_label_plural: Option<String>,
    stages: Option<RawStageBlock>,
}

#[derive(Debug, Deserialize)]
struct RawStageBlock {
    defaults: Option<RawStageDefaults>,
    states: Vec<RawStage>,
    transitions: Option<Vec<RawTransition>>,
}

#[derive(Debug, Deserialize)]
struct RawTransition {
    from: Option<String>,
    to: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawStageDefaults {
    initial: Option<bool>,
    terminal: Option<bool>,
    gate: Option<bool>,
    fresh: Option<bool>,
    #[serde(rename = "feedback-to")]
    feedback_to: Option<String>,
    worktree: Option<bool>,
    concurrency: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawStage {
    name: Option<String>,
    initial: Option<bool>,
    terminal: Option<bool>,
    gate: Option<bool>,
    fresh: Option<bool>,
    #[serde(rename = "feedback-to")]
    feedback_to: Option<String>,
    worktree: Option<bool>,
    concurrency: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// AC-3: extractor returns one entry per `### {stage}` block with
    /// body bytes byte-equal to the source between the heading and the
    /// next equal-or-higher heading (minus the trailing newline normaliser).
    #[test]
    fn prose_extracts_stage_body_verbatim() {
        let readme = "---\nstages:\n  states:\n    - name: alpha\n---\n\
            # Title\n\
            ## Stages\n\
            ### alpha\n\
            alpha-body-line-1\n\
            alpha-body-line-2\n\
            \n\
            ### beta\n\
            beta-body-line\n";
        let out = parse_stage_prose(readme);
        assert_eq!(
            out.get("alpha").map(String::as_str),
            Some("alpha-body-line-1\nalpha-body-line-2")
        );
        assert_eq!(out.get("beta").map(String::as_str), Some("beta-body-line"));
    }

    /// AC-3: a frontmatter-declared stage with no `### {stage}` prose
    /// block returns no entry — and produces no panic.
    #[test]
    fn prose_missing_block_is_silent() {
        let readme = "---\nstages:\n  states:\n    - name: alpha\n---\n\n# Title\n\n## Stages\n\n(no stage blocks here)\n";
        let out = parse_stage_prose(readme);
        assert!(out.is_empty(), "expected no prose entries, got: {out:?}");
    }

    /// AC-3: a prose block whose name does not match any frontmatter
    /// stage is retained in the returned map (the renderer ignores it).
    /// No panic, no error.
    #[test]
    fn prose_unknown_stage_is_retained_or_ignored() {
        let readme = "---\nstages:\n  states:\n    - name: alpha\n---\n\
            ## Stages\n\
            ### ghost\n\
            ghost-body\n";
        let out = parse_stage_prose(readme);
        assert_eq!(out.get("ghost").map(String::as_str), Some("ghost-body"));
    }

    /// AC-3: load the real workflow README and verify the `plan` stage
    /// body contains the substring "Approved design notes" (which is
    /// inside the Inputs bullet of the `plan` stage).
    #[test]
    fn prose_extracts_real_readme_plan_stage() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
        let contents = std::fs::read_to_string(root.join("README.md")).expect("read README");
        let out = parse_stage_prose(&contents);
        let plan = out.get("plan").expect("plan stage prose must be extracted");
        assert!(
            plan.contains("Approved design notes"),
            "plan prose missing 'Approved design notes'; got: {plan}"
        );
        // The other stages also have prose.
        for stage in ["design", "implement", "review", "done"] {
            assert!(
                out.contains_key(stage),
                "missing prose for stage {stage}; map has: {:?}",
                out.keys().collect::<Vec<_>>()
            );
        }
    }

    /// AC-4: the prose extractor's signature is structurally pure —
    /// it takes only `&str` and returns a `HashMap`. The function body
    /// also does no `fs::*` work. This test is the compile-time guard:
    /// if the signature drifts (e.g. someone adds a `&Path`), this
    /// trait-object coercion fails to compile.
    #[test]
    fn parse_stage_prose_signature_is_pure() {
        let f: fn(&str) -> HashMap<String, String> = parse_stage_prose;
        let _ = f(""); // exercise the coerced signature
    }

    /// AC-1: stage headings of the qualifier-suffixed form
    /// `### \`scoping\` (lead only, worktree)` populate the prose map
    /// under the plain stage name, not the noisy full-trim string.
    #[test]
    fn prose_extracts_qualifier_suffixed_headings() {
        let readme = "---\nstages:\n  states:\n    - name: scoping\n    - name: review\n    - name: smoke\n---\n\
            ## Stages\n\
            ### `scoping` (lead only, worktree)\n\
            scoping-body\n\
            \n\
            ### `review` (hypothesis only, gate, fresh)\n\
            review-body\n\
            \n\
            ### `smoke` (hypothesis only, worktree)\n\
            smoke-body\n";
        let out = parse_stage_prose(readme);
        assert_eq!(out.get("scoping").map(String::as_str), Some("scoping-body"));
        assert_eq!(out.get("review").map(String::as_str), Some("review-body"));
        assert_eq!(out.get("smoke").map(String::as_str), Some("smoke-body"));
        // Lock the trim contract: the noisy keys must not appear.
        assert!(
            !out.contains_key("scoping` (lead only, worktree)"),
            "noisy key should not be inserted; got map: {out:?}"
        );
    }

    /// AC-3: a slash-joined heading like
    /// `### \`expanded\` / \`ideated\` / \`done\` / \`rejected\``
    /// inserts the same prose body under each named stage.
    #[test]
    fn prose_extracts_slash_joined_terminal_stages() {
        let readme = "---\nstages:\n  states:\n    - name: expanded\n    - name: ideated\n    - name: done\n    - name: rejected\n---\n\
            ## Stages\n\
            ### `expanded` / `ideated` / `done` / `rejected`\n\
            terminal-shared-body\n";
        let out = parse_stage_prose(readme);
        for name in ["expanded", "ideated", "done", "rejected"] {
            assert_eq!(
                out.get(name).map(String::as_str),
                Some("terminal-shared-body"),
                "stage {name} should map to shared body; got {out:?}"
            );
        }
        // All four entries must share byte-identical bodies.
        let body = out.get("expanded").cloned().expect("expanded body");
        for name in ["ideated", "done", "rejected"] {
            assert_eq!(
                out.get(name),
                Some(&body),
                "{name} body should equal expanded body"
            );
        }
    }

    /// AC-1: a `stages.transitions:` block is parsed into the returned
    /// `WorkflowDefinition.transitions` vec in declaration order.
    #[test]
    fn transitions_block_is_parsed_into_definition() {
        let tmp = tempdir_path("transitions_block");
        std::fs::create_dir_all(&tmp).expect("mkdir tmp");
        let path = tmp.join("README.md");
        let readme = "---\nstages:\n  states:\n    - name: pending\n      initial: true\n    - name: scoping\n    - name: expanded\n      terminal: true\n    - name: review\n      gate: true\n    - name: smoke\n    - name: analyze\n    - name: promote\n    - name: done\n      terminal: true\n    - name: rejected\n      terminal: true\n  transitions:\n    - from: pending\n      to: scoping\n    - from: scoping\n      to: expanded\n    - from: review\n      to: rejected\n      label: reject\n    - from: smoke\n      to: rejected\n      label: reject\n    - from: analyze\n      to: rejected\n      label: reject\n    - from: promote\n      to: done\n---\n";
        std::fs::write(&path, readme).expect("write readme");

        let wf = parse_workflow_readme(&path).expect("parse");
        assert_eq!(wf.transitions.len(), 6);
        let pairs: Vec<(&str, &str)> = wf
            .transitions
            .iter()
            .map(|t| (t.from.as_str(), t.to.as_str()))
            .collect();
        assert!(pairs.contains(&("pending", "scoping")));
        assert!(pairs.contains(&("scoping", "expanded")));
        assert!(pairs.contains(&("review", "rejected")));
        assert!(pairs.contains(&("smoke", "rejected")));
        assert!(pairs.contains(&("analyze", "rejected")));
        assert!(pairs.contains(&("promote", "done")));
        // Label round-trips when present.
        let labeled: Vec<_> = wf
            .transitions
            .iter()
            .filter(|t| t.label.as_deref() == Some("reject"))
            .collect();
        assert_eq!(labeled.len(), 3);
    }

    /// AC-4 (parser layer): a frontmatter without a `transitions:` key leaves
    /// the parsed vec empty so `effective_transitions()` falls back to the
    /// implicit linear chain.
    #[test]
    fn missing_transitions_block_leaves_empty_vec() {
        let tmp = tempdir_path("transitions_missing");
        std::fs::create_dir_all(&tmp).expect("mkdir tmp");
        let path = tmp.join("README.md");
        let readme = "---\nstages:\n  states:\n    - name: design\n      initial: true\n    - name: plan\n    - name: done\n      terminal: true\n---\n";
        std::fs::write(&path, readme).expect("write readme");

        let wf = parse_workflow_readme(&path).expect("parse");
        assert!(
            wf.transitions.is_empty(),
            "expected empty transitions vec when block is absent; got {:?}",
            wf.transitions
        );
    }

    /// AC-1: labels round-trip — `Some` when declared, `None` when omitted.
    #[test]
    fn transitions_block_with_labels_round_trips() {
        let tmp = tempdir_path("transitions_labels");
        std::fs::create_dir_all(&tmp).expect("mkdir tmp");
        let path = tmp.join("README.md");
        let readme = "---\nstages:\n  states:\n    - name: a\n      initial: true\n    - name: b\n    - name: c\n      terminal: true\n  transitions:\n    - from: a\n      to: b\n      label: advance\n    - from: b\n      to: c\n---\n";
        std::fs::write(&path, readme).expect("write readme");

        let wf = parse_workflow_readme(&path).expect("parse");
        assert_eq!(wf.transitions.len(), 2);
        assert_eq!(wf.transitions[0].label.as_deref(), Some("advance"));
        assert_eq!(wf.transitions[1].label, None);
    }

    /// AC-2/AC-3 fixture: the dataagentbench research-style stages.transitions
    /// block produces edges into all four terminal stages with the right
    /// predecessor multiplicities (3 sources for `rejected`).
    #[test]
    fn parse_workflow_readme_research_fixture_terminal_edges() {
        let tmp = tempdir_path("transitions_research");
        std::fs::create_dir_all(&tmp).expect("mkdir tmp");
        let path = tmp.join("README.md");
        let readme = "---\nstages:\n  states:\n    - name: pending\n      initial: true\n    - name: scoping\n    - name: ideate\n    - name: review\n      gate: true\n    - name: smoke\n    - name: run\n    - name: analyze\n    - name: promote\n    - name: expanded\n      terminal: true\n    - name: ideated\n      terminal: true\n    - name: done\n      terminal: true\n    - name: rejected\n      terminal: true\n  transitions:\n    - from: pending\n      to: scoping\n    - from: scoping\n      to: ideate\n    - from: scoping\n      to: expanded\n    - from: ideate\n      to: review\n    - from: ideate\n      to: ideated\n    - from: review\n      to: smoke\n    - from: review\n      to: rejected\n      label: reject\n    - from: smoke\n      to: run\n    - from: smoke\n      to: rejected\n      label: reject\n    - from: run\n      to: analyze\n    - from: analyze\n      to: promote\n    - from: analyze\n      to: rejected\n      label: reject\n    - from: promote\n      to: done\n---\n";
        std::fs::write(&path, readme).expect("write readme");

        let wf = parse_workflow_readme(&path).expect("parse");
        // Count incoming edges per terminal stage.
        let inbound = |target: &str| -> Vec<String> {
            wf.transitions
                .iter()
                .filter(|t| t.to == target)
                .map(|t| t.from.clone())
                .collect()
        };
        assert_eq!(inbound("expanded"), vec!["scoping".to_string()]);
        assert_eq!(inbound("ideated"), vec!["ideate".to_string()]);
        assert_eq!(inbound("done"), vec!["promote".to_string()]);
        let rejected_sources = inbound("rejected");
        assert_eq!(rejected_sources.len(), 3, "got {:?}", rejected_sources);
        for src in ["review", "smoke", "analyze"] {
            assert!(
                rejected_sources.iter().any(|s| s == src),
                "rejected missing source {src}; got {rejected_sources:?}"
            );
        }
    }

    fn tempdir_path(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("spacetop-parser-{label}-{nanos}"))
    }

    /// `parse_workflow_readme` wires the prose extractor into the
    /// returned `WorkflowDefinition`. The real workflow README must
    /// carry prose for every frontmatter-declared stage.
    #[test]
    fn parse_workflow_readme_populates_stage_prose() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spacetop-dev");
        let wf = parse_workflow_readme(&root.join("README.md")).expect("parse");
        for stage in &wf.stages {
            assert!(
                wf.stage_prose.contains_key(&stage.name),
                "stage {} should have prose populated",
                stage.name
            );
        }
        let plan = wf.stage_prose.get("plan").expect("plan prose");
        assert!(plan.contains("Approved design notes"));
    }
}
