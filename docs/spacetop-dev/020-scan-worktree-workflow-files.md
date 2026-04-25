---
id: "020"
title: Scan worktree folders for workflow task files
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

SpaceTop currently skips `.worktrees/` when scanning for workflow task files. When a Spacedock implement agent is active, all task file writes land in the worktree branch — the main branch entity file is not updated until the PR merges. This means the TUI shows stale task state during active development.

The fix is to merge the main branch scan with worktree scans using the following precedence rules:

1. **Task only in main** — display it as-is.
2. **Task only in a worktree** — display it (new tasks filed inside a worktree, or tasks whose entity file only exists there).
3. **Same task ID in both** — compare content via SHA-1 hash (not full text). If hashes differ, use the worktree's version (it is more recent). If hashes match, either copy is fine.

SHA-1 is used for change detection only — no cryptographic guarantees needed. The goal is a fast, allocation-light check that avoids reading full file contents for every task on every refresh.

## Acceptance criteria

**AC-1 -- Worktree task files are included in the TUI task list.**
When a `.worktrees/` directory exists under the project root and contains workflow entity files, those files appear in the task list.
Verified by: integration test or fixture that places a task file in a mock worktree path and confirms the scanner returns it.

**AC-2 -- Tasks present only in main are not dropped.**
Entity files on the main branch that have no corresponding worktree copy are still shown.
Verified by: test with a mixed set (some entities main-only, some worktree-only, some in both).

**AC-3 -- Tasks present only in a worktree are shown.**
Entity files found in a worktree but absent from the main branch scan appear in the task list.
Verified by: same mixed-set test as AC-2.

**AC-4 -- When the same task ID exists in both, the worktree version wins.**
If a task entity file (same slug/ID) exists in both the main branch path and a worktree path, and their SHA-1 hashes differ, the worktree copy is returned.
Verified by: unit test that provides two copies of the same entity with different content and asserts the worktree copy is selected.

**AC-5 -- SHA-1 hash is used for content comparison, not full text.**
The merge logic hashes file content (e.g. `sha1(file_bytes)`) and compares digests — it does not read and compare full strings.
Verified by: code review — the merge function uses a hash type, not string equality on the full body.

**AC-6 -- Scanning multiple worktrees is supported.**
If multiple `.worktrees/<name>/` directories exist (parallel agents working on different tasks), all are scanned and merged.
Verified by: test with two worktree directories each containing a distinct entity file.

**AC-7 -- No regression on repos without worktrees.**
When `.worktrees/` does not exist or is empty, the scanner behaves identically to today.
Verified by: existing tests continue to pass unchanged.
