---
id: "043"
title: Stage-heading parser drops prose when the README uses `### `stage` (qualifier)` form
status: design
source: captain
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

When the captain opens `spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/` and presses `D` to show the workflow README detail, the per-stage description blocks are all empty even though the README clearly defines them under a `## Stages` section.

## Reproduction

```bash
spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/
# press D to open the workflow README detail view
# every stage row shows an empty description
```

The README at that path uses stage headings of the form:

```
### `pending`
### `scoping` (lead only, worktree)
### `ideate` (question only, no worktree)
### `review` (hypothesis only, gate, fresh)
### `smoke` (hypothesis only, worktree)
### `run` (hypothesis only, worktree)
### `analyze` (hypothesis only, fresh, no worktree)
### `promote` (hypothesis only, gate, fresh)
### `expanded` / `ideated` / `done` / `rejected`
```

The only headings whose prose actually populates the per-stage map are `### \`pending\`` and (effectively) the combined `### \`expanded\` / \`ideated\` / \`done\` / \`rejected\`` line — and the latter populates the wrong key. Every stage that has a parenthetical qualifier after the backticked name (`scoping`, `ideate`, `review`, `smoke`, `run`, `analyze`, `promote`) loses its prose entirely.

## Diagnosis

`src/parser/readme.rs::stage_heading_name` (line 130) strips the `### ` prefix, then runs `trim_matches('\`')` on the remainder. That only strips backticks from the **outer** edges. For a heading like `\`scoping\` (lead only, worktree)`:

- after `strip_prefix("### ")` → `\`scoping\` (lead only, worktree)`
- after `trim()` → unchanged
- after `trim_matches('\`')` → `scoping\` (lead only, worktree)` — only the leading backtick is removed because the trailing char is `)`, not a backtick

The resulting string `scoping\` (lead only, worktree)` is stored as the key in the prose `HashMap<String, String>`. Later when the renderer looks up `definition.stage_prose.get("scoping")` to render the detail view, it finds nothing — the key on file is the full noisy string. So the rendered detail pane shows the stage name from frontmatter but no description body.

The README contract for spacetop is implicit and lenient elsewhere (frontmatter accepts varied shapes, the YAML parser is permissive), so the heading parser's strictness about "name must be the entire trimmed-and-unticked content" is the outlier. Other Spacedock workflows in real use (notably the bigger research/hypothesis-flavoured workflows) routinely add a parenthetical qualifier on stage headings as documentation, and the parser should accept that.

## Acceptance criteria

**AC-1 — Stage prose populates for the canonical research-workflow heading form.**
Verified by: a unit test in `src/parser/readme.rs::tests` that feeds the parser a `## Stages` block with headings of the form `### \`scoping\` (lead only, worktree)`, `### \`review\` (hypothesis only, gate, fresh)`, and a few siblings — all matching the frontmatter `stages.states` names — and asserts the returned `HashMap<String, String>` has an entry for each plain stage name (`"scoping"`, `"review"`, …) whose value is the prose body that followed that heading.

**AC-2 — Stage prose still populates for the simple `### \`stage\`` form.**
Verified by: the existing `parse_workflow_readme_populates_stage_prose` test (or its equivalent) continues to pass unchanged. The fix must be additive — accepting the qualifier-suffixed form without breaking the strict form.

**AC-3 — Slash-joined heading like `### \`expanded\` / \`ideated\` / \`done\` / \`rejected\`` is recognized for every named stage in the list.**
Verified by: a unit test that asserts each of the four names becomes a key in the returned map, and they all share the same prose body (the prose under that combined heading applies to all four terminal stages). If the design stage decides this case is out of scope for the fix, the test instead asserts that at least one of the four names is keyed (today: none are) and the entity body's design notes explicitly defer the multi-stage form to a follow-up task.

**AC-4 — The TUI detail view (D-key) renders the matched prose under each stage when running against the research workflow.**
Verified by: an integration test or a rendering test in `src/ui/` that loads the research workflow fixture (a trimmed copy in `tests/fixtures/` is fine), opens the workflow detail view, and asserts the rendered buffer contains a recognisable substring from each stage's prose body (e.g. for `scoping`, a phrase that appears in the real README's `scoping` paragraph). At a minimum, assert non-empty rendered prose for `scoping`, `review`, `smoke`, `analyze`, and `promote`.

**AC-5 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green; pre-existing parser tests untouched or updated in lockstep with the new heading-shape coverage.

## Suggested fix sketch (for the design / implement stages)

The parser needs to extract the first backtick-wrapped token from the heading and treat that as the stage name, ignoring any trailing prose. Pseudocode:

```rust
fn stage_heading_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("### ")?;
    let trimmed = rest.trim();
    // Prefer backtick-delimited token: `name`
    if let Some(name) = first_backtick_token(trimmed) {
        return Some(name);
    }
    // Fallback: whole trimmed line, only if it has no inline backticks at all
    if !trimmed.contains('`') {
        return Some(trimmed);
    }
    None
}
```

For AC-3, the parser may need to scan for *all* backtick tokens on the heading line and emit the same prose body under each one — the design stage should weigh in on whether to handle the multi-stage combined heading or punt.
