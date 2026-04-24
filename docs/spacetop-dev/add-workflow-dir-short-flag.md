---
id: 004
title: Add short `-w` alias for `--workflow-dir`
status: design
source: captain feedback after build-initial-tui-overview
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

Expose `-w` as a short alias for the existing `--workflow-dir` CLI flag so the common invocation (`spacetop -w docs/spacetop-dev`) is ergonomic. Scope is strictly the CLI surface; no behavior change.

## Acceptance criteria

_To be firmed up during design. Expected shape:_

**AC-1 -- `spacetop -w <path>` behaves identically to `spacetop --workflow-dir <path>`.**
Verified by: CLI parsing test / `cargo test` covers both spellings; `spacetop -h` documents the short form.
