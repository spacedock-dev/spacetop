---
id: 005
title: Auto-discover Spacedock workflows under `docs/`
status: design
source: captain feedback after build-initial-tui-overview
started: 2026-04-24T16:06:48Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

When `spacetop` is launched without `--workflow-dir` from a repo root, it should scan `docs/` for Spacedock workflow directories (identified by a README with `commissioned-by:` frontmatter, or equivalent signal) and present them to the user. A single repo can host multiple workflows; the TUI must let the user pick which to open when more than one is found.

Open design questions to resolve in the `design` stage:

- What is the canonical signal for "this directory is a Spacedock workflow"? (README frontmatter key, a sentinel file, presence of `_mods/`, etc.)
- Does the scan recurse under `docs/`, or only inspect immediate subdirectories?
- Zero-workflow and single-workflow fallbacks (error out? drop to an empty overview? auto-open the one hit?).
- Should an explicit `--workflow-dir` still short-circuit discovery? (Expected: yes.)
- How does the picker interact with the existing overview — a separate screen, a side panel, or a startup prompt?

## Acceptance criteria

_To be firmed up during design. Expected shape:_

**AC-1 -- Running `spacetop` at a repo root with zero `--workflow-dir` scans `docs/` and lists discovered workflows.**
Verified by: integration test against a fixture repo containing multiple workflow dirs under `docs/`.

**AC-2 -- Explicit `--workflow-dir` / `-w` still opens that workflow directly, bypassing discovery.**
Verified by: CLI test.

**AC-3 -- Multi-workflow and zero-workflow cases have defined UX with no panics.**
Verified by: fixture-driven tests for both paths.
