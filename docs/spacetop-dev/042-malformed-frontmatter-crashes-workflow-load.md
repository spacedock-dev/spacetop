---
id: "042"
title: One malformed-frontmatter entity crashes the entire workflow load
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

Running `spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/` exits 1 with no TUI shown:

```
Error: failed to load workflow directory /Users/kent/Dev/InfuseAI/GitHub/dataagentbench/docs/research

Caused by:
    0: /Users/kent/Dev/InfuseAI/GitHub/dataagentbench/docs/research/rd-013.md: malformed YAML frontmatter: mapping values are not allowed in this context at line 7 column 137
    1: mapping values are not allowed in this context at line 7 column 137
```

The offending entity (`rd-013.md`) has an unquoted multi-line `diff_summary:` value containing colons, which `serde_yaml` rejects. The error itself is technically correct, but a single bad entity should not be allowed to take down the whole workflow view — the captain cannot inspect any of the other 12+ valid entities in that workflow until the bad file is hand-fixed.

spacetop is a read-only viewer. The current behavior is closer to a strict compiler than a viewer: it bails on the first parse error and returns nothing. A viewer's job is to show what it can and surface what it can't, not to refuse to start.

## Background

- Loader entry point: `src/discovery.rs` / `src/parser.rs` (see `CLAUDE.md` module layout). The current load path treats any parse failure as fatal and propagates it up to `lib.rs::run_terminal` / `decide_app`.
- The error message names the file, line, and column — that's good — but offers no remediation hint (e.g., "wrap values containing `:` in quotes" or "add `>-` for multi-line").
- The TUI never starts, so the captain has no in-app affordance for navigating to the broken entity, viewing its raw frontmatter, or skipping it.
- Stable user-facing strings are pinned by tests (per `CLAUDE.md`) — any new behavior here needs paired test updates.

## Acceptance criteria

**AC-1 — One malformed entity does not prevent the rest of the workflow from loading.**
Verified by: a `cargo test` that constructs a workflow directory with N valid entity files and one entity file whose frontmatter is malformed (e.g., an unquoted value containing `:`), invokes the loader, and asserts the returned workflow contains the N valid entities. The malformed entity must be reported (see AC-2) but must not abort the load.

**AC-2 — Malformed entities surface as in-app errors, not as a process-level crash.**
Verified by: the same test (or a sibling test) asserts the loader returns or records a structured per-entity error for the malformed file — containing the file path, the underlying YAML error, and (where derivable) the line/column — that the TUI can render alongside the valid entities. Acceptable surfaces include: a synthetic "broken entity" placeholder in the item list with the error in its preview pane, a separate errors panel, or an overlay; the design stage picks one and justifies it.

**AC-3 — `spacetop -w {path}` exits 0 and starts the TUI even when one entity has malformed frontmatter.**
Verified by: an integration test in `tests/` that runs `decide_app` (or equivalent) against a fixture workflow with one malformed entity and asserts a launch decision (no error return) consistent with the existing zero-workflows / valid-workflow tests.

**AC-4 — The existing strict-failure path remains available for genuinely unusable inputs.**
Verified by: tests that cover "no entities parse" or "workflow `README.md` is itself malformed" still produce a clear top-level error and a non-zero exit, matching today's behavior. The fix only relaxes the per-entity case, not the entire-workflow case.

**AC-5 — `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy `-D warnings`) and `cargo test` from the repo root, both green.

## Reproduction (for design / implement reference)

```bash
spacetop -w /Users/kent/dev/InfuseAI/GitHub/dataagentbench/docs/research/
# exits 1; Error: failed to load workflow directory ... rd-013.md: malformed YAML frontmatter
```

The minimal fixture for the test suite is two files: one valid entity (anything from the existing `tests/` fixtures), one entity with a frontmatter line like `diff_summary: build the candidate set using a disjunction: WHERE foo` (the unquoted post-colon segment is what breaks YAML).
