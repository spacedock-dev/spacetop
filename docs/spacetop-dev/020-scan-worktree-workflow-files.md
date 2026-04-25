---
id: "020"
title: Scan worktree folders for workflow task files
status: plan
source: captain
started: 2026-04-25T16:43:26Z
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

## Stage Report: design

- DONE: The current scan logic is located and its entry point / data structures are identified.
  Entry point is `parser::load_workflow_dir` (`src/parser.rs:191`). It reads `*.md` files from the workflow dir (skipping `README.md`) via `fs::read_dir` and returns a `WorkflowSnapshot { definition: WorkflowDefinition, items: Vec<WorkItem> }`. The `.worktrees` skip is in `discovery::PRUNED_DIR_NAMES` (`src/discovery.rs:29-38`), which prunes `.worktrees` from workflow-discovery walks; item scanning itself is plain `fs::read_dir` without worktree awareness. `OverviewState::reload` (`src/app.rs:169`) is the watcher-driven call site.

- DONE: The SHA-1 merge strategy (main ∪ worktrees, worktree wins on hash mismatch) is concretely specified with the relevant types and function signatures.
  A new function `merge_worktree_items` should be added to `src/parser.rs` with the signature:
  `fn merge_worktree_items(main_items: Vec<WorkItem>, worktree_items: Vec<WorkItem>) -> Vec<WorkItem>`
  The merge key is the file stem (slug) — same logic as `slug_of` in `src/app.rs:47`. Build a `HashMap<String, WorkItem>` from main items, then for each worktree item compute `sha1(fs::read(path))` and compare against `sha1(fs::read(main_path))`; if digests differ or no main copy exists, the worktree item wins. The `[sha1]` crate (crates.io, pure Rust, no_std compatible) is the recommended choice — it exposes `sha1::Sha1::digest(&[u8]) -> [u8; 20]`. No `sha2`/`ring`/`openssl` needed. Worktree path discovery: iterate `<repo_root>/.worktrees/*/`, each entry is a worktree root; the parallel workflow path is `<worktree_root>/<workflow_relative_path>/` where `workflow_relative_path` is the workflow dir path relative to `repo_root`.

- DONE: Parser/TUI constraints are named so the plan stage can proceed immediately — specifically which Rust SHA-1 crate to use and how worktree paths are discovered.
  Crate: `sha1 = "0.10"` (the `sha1` crate on crates.io, part of the RustCrypto family). API: `use sha1::{Sha1, Digest}; let hash: [u8; 20] = Sha1::digest(&file_bytes).into();`. Worktree path mapping: given `repo_root` and `workflow_dir` (absolute, canonical), the relative path is `workflow_dir.strip_prefix(repo_root)` → `rel`; each worktree scan path is `repo_root/.worktrees/<name>/<rel>/`. The `OverviewState::reload` call site needs `repo_root` passed in (currently not available — it must be threaded from `resolve_scan_root` through `OverviewState` or derived lazily from `workflow_dir` by walking up to `.git`). TUI constraint: `WorkItem.path` must remain the worktree-local absolute path so the file-watcher can watch the correct file; `reload_from_snapshot` slug-matching in `app.rs:122` is unaffected since it uses `file_stem`, not full path.

### Summary

The current active-item scan entry point is `parser::load_workflow_dir`, called by `OverviewState::reload`. The `.worktrees` skip lives in workflow-discovery (`discovery.rs`), not item scanning — so item scanning must be augmented, not the discovery prune list. The SHA-1 merge strategy uses the `sha1 = "0.10"` crate with `Sha1::digest(&bytes)` for a 20-byte digest comparison; worktree paths are resolved as `<repo_root>/.worktrees/<name>/<workflow_rel>/`. The one structural constraint is that `repo_root` must be available at reload time, either stored in `OverviewState` or re-derived from `workflow_dir`.
