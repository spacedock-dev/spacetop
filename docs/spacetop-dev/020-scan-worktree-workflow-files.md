---
id: "020"
title: Scan worktree folders for workflow task files
status: review
source: captain
started: 2026-04-25T16:43:26Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-020-scan-worktree-workflow-files
issue:
pr: #9
mod-block: merge:pr-merge
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

## Implementation Plan

### Step 1 — Add `sha1` dependency to `Cargo.toml`

File: `Cargo.toml`

Add under `[dependencies]`:

```toml
sha1 = { version = "0.10", features = ["oid"] }
```

The `oid` feature is optional but the base `sha1 = "0.10"` entry is sufficient. The RustCrypto `Digest` trait is included via `sha1`'s re-export; no separate `digest` crate entry is required for the `Sha1::digest(&[u8])` → `[u8; 20]` API.

Verify: `cargo check --all-targets 2>&1 | grep -E "^error"` — must be empty.

### Step 2 — Add `repo_root` to `OverviewState`

File: `src/app.rs`

**Design decision:** derive `repo_root` lazily at construction from `workflow_dir` using `discovery::resolve_scan_root`. This avoids threading a new parameter through every call site in `src/lib.rs` (six locations) and keeps all `OverviewState::load` / `::empty` / `::from_snapshot` signatures unchanged.

Changes:
- Add `pub repo_root: PathBuf` field to the `OverviewState` struct (after `workflow_dir`).
- In `OverviewState::empty(workflow_dir)`: set `repo_root: discovery::resolve_scan_root(&workflow_dir)`.
- In `OverviewState::load(workflow_dir)`: the existing code calls `load_workflow_dir(&workflow_dir)` then `Self::from_snapshot(workflow_dir, snapshot)` — so `from_snapshot` is the initializer.
- In `OverviewState::from_snapshot(workflow_dir, snapshot)`: add `repo_root: discovery::resolve_scan_root(&workflow_dir)` to the struct literal.

`resolve_scan_root` is a cheap upward `.git` walk (at most ~20 `Path::join` + `Path::exists` calls). Calling it once at construction is fine.

Verify: `cargo check --all-targets 2>&1 | grep -E "^error"` — must be empty.

### Step 3 — Implement `scan_worktrees` and `merge_worktree_items` in `src/parser.rs`

File: `src/parser.rs`

Add two new functions (module-internal, not `pub`):

**`fn scan_worktrees(repo_root: &Path, workflow_rel: &Path, allowed_statuses: &[String]) -> Vec<WorkItem>`**

Logic:
1. Compute the worktrees dir: `let wt_dir = repo_root.join(".worktrees");`
2. If `wt_dir` does not exist, return `Vec::new()` immediately (satisfies AC-7).
3. `fs::read_dir(&wt_dir)` — iterate entries; for each entry that is a directory:
   - Compute candidate path: `entry.path().join(workflow_rel)`.
   - If the candidate directory does not exist, skip (not all worktrees mirror every workflow).
   - Call the existing item-scan inner loop (same logic as `load_workflow_dir`'s `fs::read_dir` block) to collect `*.md` files (skip `README.md`), sort them, and parse each via `parse_work_item(&item_path, allowed_statuses)`.
   - Accumulate all successfully parsed items; silently skip items with parse errors (same lenient policy as existing code).
4. Return the combined `Vec<WorkItem>` from all worktrees.

**`fn merge_worktree_items(main_items: Vec<WorkItem>, worktree_items: Vec<WorkItem>) -> Vec<WorkItem>`**

Logic:
1. `use sha1::{Sha1, Digest};`
2. Build `let mut index: HashMap<String, WorkItem>` from `main_items`, keyed by file stem (`path.file_stem().unwrap_or_default().to_string_lossy()`).
3. For each `wt_item` in `worktree_items`:
   - Compute `slug = file_stem(wt_item.path)`.
   - If slug is not in `index` → insert (AC-3: worktree-only items appear).
   - If slug is in `index`:
     - Read both file byte buffers: `fs::read(&wt_item.path)` and `fs::read(&main_item.path)`. On any IO error, take the worktree version (conservative).
     - Compute `Sha1::digest(&wt_bytes)` and `Sha1::digest(&main_bytes)`.
     - If digests differ → overwrite with `wt_item` (AC-4: worktree wins on mismatch).
     - If digests match → keep either (index already holds main; no change needed).
4. Drain `index` into a `Vec<WorkItem>`, then sort by `item.path` to get a stable order consistent with `load_workflow_dir`'s existing `item_paths.sort()`.
5. Return the vec.

The function reads file bytes for comparison (satisfies AC-5: SHA-1 used, not string equality).

Verify: `cargo check --all-targets 2>&1 | grep -E "^error"` — must be empty.

### Step 4 — Augment `load_workflow_dir` to accept and use `repo_root`

File: `src/parser.rs`

Change the signature of `load_workflow_dir`:

```rust
pub fn load_workflow_dir(path: &Path, repo_root: &Path) -> Result<WorkflowSnapshot, ParseError>
```

Inside the function, after building `items` from the main-branch scan:

```rust
let workflow_rel = path.strip_prefix(repo_root).unwrap_or(path);
let worktree_items = scan_worktrees(repo_root, workflow_rel, &allowed_statuses);
let items = merge_worktree_items(items, worktree_items);
```

Then `return Ok(WorkflowSnapshot { definition, items });` as before.

Update the two call sites:
- `OverviewState::load` in `src/app.rs`: `load_workflow_dir(&workflow_dir, &discovery::resolve_scan_root(&workflow_dir))` — but since `from_snapshot` / `empty` already compute `repo_root`, extract it or pass it inline. The simplest approach: `OverviewState::load` computes `repo_root` first, then passes it.
- `OverviewState::reload` in `src/app.rs`: `load_workflow_dir(&self.workflow_dir, &self.repo_root)`.

Parser tests that call `load_workflow_dir` directly must be updated to pass a `repo_root`. For tests that don't exercise worktree behavior, pass the `workflow_dir` itself as `repo_root` (no `.worktrees` sibling → `scan_worktrees` returns `Vec::new()` → behavior identical to today, satisfying AC-7).

Verify: `cargo test --lib -- parser 2>&1 | tail -5` — all parser unit tests pass.

### Step 5 — Write tests in `src/parser.rs`

File: `src/parser.rs`, `mod tests` block

Add a `write_minimal_workflow(dir, stage_name)` helper that writes a `README.md` (commissioning frontmatter + single stage) and zero or more entity `.md` files.

Tests to add:

**`worktree_items_included` (AC-1, AC-6)**
- Create temp root, write one workflow at `root/docs/wf/`, two worktrees at `root/.worktrees/wt-a/docs/wf/` and `root/.worktrees/wt-b/docs/wf/` each with a distinct entity file not in main.
- Assert `load_workflow_dir("root/docs/wf", "root")` returns all entities (main + wt-a + wt-b).

**`main_only_items_preserved` (AC-2)**
- Main has entity `aaa.md`, worktree has entity `bbb.md`.
- Assert both appear in result.

**`worktree_only_items_shown` (AC-3)**
- Main has no entity files; worktree has `ccc.md`.
- Assert result contains `ccc.md`'s item.

**`worktree_version_wins_on_hash_mismatch` (AC-4)**
- Same entity slug in main and worktree with different body text.
- Assert the returned item's body matches the worktree copy.

**`sha1_used_not_string_equality` (AC-5)**
- Code-review check: `merge_worktree_items` source contains `Sha1::digest` and does not contain direct `==` comparison on `body` field. This is enforced at review time; no runtime test needed. Document with a comment in the function.

**`no_regression_without_worktrees` (AC-7)**
- Temp root with workflow dir but no `.worktrees` subdirectory.
- Assert `load_workflow_dir` returns the same items as the pre-existing behavior (just the main items).

All six runtime tests run without a TUI session; `cargo test --lib -- parser` is sufficient.

Verify: `cargo test --lib -- parser 2>&1 | tail -10` — 6 new tests + all prior parser tests pass.

### Step 6 — Full test suite green

Run `cargo test 2>&1 | tail -15` — all tests pass (includes discovery, app, and lib integration tests). No regressions.

### Cargo commands summary

| Step | Command |
|------|---------|
| 1 (dep) | `cargo check --all-targets` |
| 2 (repo_root field) | `cargo check --all-targets` |
| 3-4 (parser impl) | `cargo check --all-targets` |
| 5 (new tests) | `cargo test --lib -- parser` |
| 6 (full suite) | `cargo test` |

### Module / file ownership

| File | Changes |
|------|---------|
| `Cargo.toml` | add `sha1 = "0.10"` |
| `src/parser.rs` | add `scan_worktrees`, `merge_worktree_items`; update `load_workflow_dir` signature and body; update tests |
| `src/app.rs` | add `repo_root: PathBuf` to `OverviewState`; update `empty`, `from_snapshot`, `load`, `reload` |
| `src/lib.rs` | no changes needed (all call sites go through `OverviewState::load` which self-derives `repo_root`) |

## Stage Report: plan

- DONE: Steps are ordered correctly: sha1 dep → repo_root threading → merge_worktree_items → augment reload → tests.
  Plan steps 1–6 follow that exact sequence; each step's output is a prerequisite for the next.

- DONE: Every step names the exact file, function, and cargo command needed.
  Steps 1–6 each list file paths, function signatures, and `cargo check`/`cargo test` commands with exact flags.

- DONE: Test strategy covers all seven ACs without requiring a live TUI session.
  Six named unit tests in `mod tests` of `src/parser.rs` cover AC-1 through AC-4, AC-6, and AC-7; AC-5 is satisfied by code review (SHA-1 usage is structural, not runtime-observable) with a source comment.

### Summary

The plan sequences five implementation steps — sha1 dep, repo_root field addition, scan_worktrees/merge_worktree_items, load_workflow_dir signature update, and unit tests — each producing a `cargo check`-clean artifact before the next step begins. `repo_root` is derived lazily at `OverviewState` construction via the existing `resolve_scan_root` helper, avoiding any signature changes to `lib.rs` call sites. All seven ACs are exercised by tempfile-based unit tests in `src/parser.rs` with no TUI dependency.
