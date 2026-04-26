---
id: "021"
title: "spacetop panics with IO error when workflow directory does not exist"
status: design
source: bug report
started:
completed:
verdict:
score: 0.8
worktree:
issue:
pr:
---

`spacetop` crashes with an unhelpful discovery IO error when run from a directory where the expected workflow path does not exist on disk. The user sees a raw OS error (2: No such file or directory) rather than a clear message about what path was scanned and what to do next.

## Error log

```
➜  recce-gtm git:(main) ✗ ~/.cargo/bin/spacetop
Error: failed to scan /Users/clkao/git/recce-gtm

Caused by:
    0: discovery IO error: No such file or directory (os error 2)
    1: No such file or directory (os error 2)
```

## Context

- `spacetop` is invoked with no arguments from `/Users/clkao/git/recce-gtm`.
- The error occurs during the workflow-directory discovery scan, before any UI is rendered.
- OS error 2 means the target path does not exist, but the message does not say which sub-path triggered the failure.

## Acceptance criteria

**AC-1 -- Graceful message when no workflow directory is found.**
When `spacetop` is run from a directory with no detectable workflow, it exits with a clear human-readable message (e.g. "No Spacedock workflow found in `<path>`. Run `spacedock commission` to create one.") and a non-zero exit code, rather than an unformatted `anyhow` chain.
Verified by: running `spacetop` from an empty temp directory and checking stderr output and exit code.

**AC-2 -- No raw OS error numbers in user-facing output.**
The final user-visible message must not contain `os error 2` or any raw errno string.
Verified by: grep on stderr output of the test scenario above.
