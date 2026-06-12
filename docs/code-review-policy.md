# Spacetop Code Review Policy

**Owner:** CTO policy for human and AI reviewers
**Applies to:** all pull requests, local branch reviews, and agent review passes
**Last reviewed:** 2026-06-12

This is the canonical code review policy for Spacetop. `AGENTS.md` remains the
mandatory repo entrypoint and highest repo contract. If this file conflicts with
`AGENTS.md`, follow `AGENTS.md` and fix this file.

All review surfaces must load this file instead of maintaining separate review
rules:

- Codex reads `AGENTS.md`, which requires this file for review work.
- Claude Code reads `CLAUDE.md`, which imports this file.
- GitHub Copilot reads `.github/copilot-instructions.md`, which points code
  review to this file.

## Review Goal

Reviews protect the Spacetop product contract: a read-first Rust TUI for
inspecting Spacedock markdown workflow state. A review is not a style pass. It
should identify concrete defects, regressions, missing verification, and policy
violations that could make the tool unsafe, confusing, or harder to maintain.

## Required Reviewer Stance

- Lead with findings, ordered by severity.
- Ground each finding in the current diff, file path, and line number.
- Explain concrete impact: what breaks, what risk grows, or what user behavior
  changes.
- Do not report speculative issues. If evidence is missing, say what evidence is
  missing and why it matters.
- If there are no findings, say that clearly and name any test gaps or residual
  risk.
- For PR reviews, end with a direct judgment: approve, comment only, or request
  changes.

## Severity

- **Critical:** data loss, unsafe workflow-state mutation, broad git writes,
  security issue, or a change that prevents normal startup.
- **High:** user-visible regression, parser/index corruption, broken documented
  command, invalid config/session path behavior, or missing guardrail around a
  write path.
- **Medium:** maintainability or testability issue that will likely cause future
  bugs, missing tests for changed behavior, stale docs for behavior changes, or
  architecture drift across module boundaries.
- **Low:** localized clarity, naming, or documentation issue that does not block
  merge unless it repeats across the change.

Request changes for Critical and High findings. Request changes for Medium
findings when they affect behavior, safety, or test coverage. Low findings are
usually comments unless they reveal a repeated pattern.

## Mandatory Review Checks

Check the areas touched by the change, not every rule mechanically.

- **Read-only contract:** Spacetop must not mutate Spacedock workflow markdown by
  default. The only approved workflow-adjacent write path is explicit sync using
  `git pull --ff-only`.
- **Git safety:** reject broader git writes, implicit commits, branch mutation,
  or shell-outs that can alter workflow state outside the audited sync path.
- **Config/session safety:** config and session files may only be read or written
  under absolute XDG/HOME-derived user paths, never workflow directories.
- **Domain before UI:** workflow schema, parser, index, query, and state facts
  must be typed before rendering. UI code must not infer schema rules from raw
  strings or vectors.
- **Core boundary:** `spacetop-core` must stay terminal-free. Terminal crates and
  Ratatui rendering belong in the `spacetop` crate.
- **Parser contracts:** preserve README discovery, status validation, archive
  handling, active item loading rules, and worktree merge behavior unless the
  task explicitly changes them.
- **UI contracts:** preserve documented keyboard behavior, footer/help text, ASCII
  graph fallback, narrow-terminal behavior, and test-pinned strings unless the
  diff updates tests and docs together.
- **Error handling:** production paths should return meaningful errors instead of
  hiding parser failures or using `unwrap`, `expect`, or `panic!`.
- **Dependencies:** new dependencies need a clear correctness or complexity
  reason. Prefer existing crates and standard Rust APIs.
- **Docs:** behavior, command, architecture, config/session, or policy changes
  must update nearby docs in the same PR.

## Verification Expectations

Reviewers should verify the author supplied appropriate evidence. Run checks
when practical for local reviews; for hosted reviews, flag missing evidence.

- Rust code changes require `cargo fmt`, `cargo test`, and `make lint` unless the
  PR clearly explains why a narrower check is sufficient.
- `make lint` means `cargo clippy --all-targets --all-features -- -D warnings`.
- Parser behavior belongs in parser tests.
- App/input behavior belongs in app tests.
- Rendering behavior should use Ratatui `TestBackend` assertions before manual
  terminal checks.
- Git sync changes require `git_sync` tests and the
  `crates/spacetop-core/tests/no_write_git_calls.rs` guardrail to remain valid.
- Core boundary changes require
  `crates/spacetop-core/tests/no_terminal_deps.rs` to remain valid.
- Watcher backend changes may need `cargo test -- --ignored` for the real
  `notify` smoke test.
- Documentation-only changes do not require Rust checks unless they change
  commands, examples, or behavior claims.

## Review Output Format

Use this shape unless the requested review surface requires a different format:

```markdown
Findings
- High - path/to/file.rs:123 - Concrete impact and why this must change.
- Medium - path/to/test.rs:45 - Missing coverage or policy drift.

Open Questions
- Any uncertainty that changes the approval decision.

Judgment
Request changes / Comment only / Approve.
```

Keep summaries secondary. Do not bury findings under praise or broad commentary.
