---
id: "062"
title: Diagnose history unavailable in metrics, activity, and timeline views
status: plan
source: captain diagnostic request 2026-06-12
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: reproduced headless commands plus core/headless regression tests and make lint
started: 2026-06-12T13:01:29Z
completed:
verdict:
score: 0.86
worktree:
issue:
pr:
---

Metrics, activity, and timeline keep showing:

```text
history unavailable: git log could not be read
```

The failure reproduces in the current Spacetop project with
`docs/spacetop-dev`, even though the repository has readable git history for the
workflow path. The diagnostic pass should find the actual failing boundary and
fix it, or make the unavailable reason precise enough to act on if the history
source is legitimately incomplete.

## Reproduction evidence

Observed from the repo root on 2026-06-12:

```bash
cargo run -p spacetop -- metrics --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- activity --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- timeline 056 --workflow-dir docs/spacetop-dev
```

Each command completed successfully at the process level but printed:

```text
history unavailable: git log could not be read
```

Sanity checks from the same checkout:

```bash
git rev-parse --is-shallow-repository
git log --oneline -- docs/spacetop-dev
git log --first-parent --reverse --date=unix --pretty=format:%H%x00%ct --name-status -M -- 'docs/spacetop-dev/**'
```

The repository reported `false` for shallow status, and both git-log checks
returned workflow history. That suggests the user-facing message may be masking
a downstream history-loader error, such as `git show` or metadata extraction,
not necessarily the initial `git log` command.

Plan-stage confirmation from the current checkout:

```bash
cargo run -p spacetop -- metrics --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- activity --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- timeline 056 --workflow-dir docs/spacetop-dev
git rev-parse --is-shallow-repository
git log --first-parent --reverse --date=unix --pretty=format:%H%x00%ct --name-status -M -- docs/spacetop-dev/**
git show fa66b5d24604018b0de727d9aad9106107063f70:docs/spacetop-dev/_archive/scaffold-rust-cli-project.md
```

The three headless commands still exit 0 and print the generic git-log
message. `git rev-parse --is-shallow-repository` prints `false`, and the
history log returns workflow touches. The first archived entity in that log can
be read with `git show`, but its historical frontmatter uses `id: 001` as an
unquoted scalar. The normal parser accepts that style through its flat
frontmatter fallback and preserves the lexical ID, but
`crates/spacetop-core/src/git_history.rs::frontmatter_metadata` currently reads
`id` through `serde_yaml::Value::as_str()`. That is the primary boundary to
probe first: `git log` and `git show` can succeed while metadata extraction
still returns `HistoryUnavailable::GitError(_)`, which renders as
`history unavailable: git log could not be read`.

## Scope

- Kind: bugfix
- Risk: medium
- Milestone: v1-maintenance
- Touches: git / index-query / headless CLI / UI history views
- Non-goals: changing Spacedock workflow markdown semantics, adding workflow
  write support, or filing a GitHub issue.

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- The root cause is identified with evidence.**
The implementation report names the exact failing operation and explains why the
current Spacetop project prints `history unavailable: git log could not be read`
despite readable workflow git history.
Verified by: implementation report cites the first failing history-loader
boundary and includes the current `docs/spacetop-dev` reproduction commands plus
the specific git probe or focused test that proves whether the failure is log,
blob read, or metadata extraction.

**AC-2 -- Headless metrics, activity, and timeline behave correctly for this repo.**
Running the reproduced commands against `docs/spacetop-dev` produces useful
history output when history is available, or a precise unavailable reason when it
is not.
Verified by: `cargo run -p spacetop -- metrics --workflow-dir docs/spacetop-dev`,
`cargo run -p spacetop -- activity --workflow-dir docs/spacetop-dev`, and
`cargo run -p spacetop -- timeline 056 --workflow-dir docs/spacetop-dev`, plus
headless tests that exercise text and JSON unavailable rendering for all three
commands.

**AC-3 -- TUI history views use the same corrected history result.**
The Metrics, Activity, and Timeline TUI pages stop showing the generic git-log
message for this reproducible case and stay consistent with headless output.
Verified by: Ratatui `TestBackend` assertions in `crates/spacetop/src/ui/tests.rs`
for Metrics, Activity, and Timeline using the same `HistoryUnavailable` variants
returned by the core/index path; manual TUI smoke only supplements those tests.

**AC-4 -- Regression coverage protects the history loader boundary.**
Tests cover the failing path at the lowest practical layer, including the case
where `git log` succeeds but later history processing fails.
Verified by: `crates/spacetop-core/src/git_history.rs` unit tests with
`RecordingGitRunner` and `crates/spacetop-core/tests/git_history_fixtures.rs`
fixture tests for accepted legacy entity IDs, actual log failures, `git show`
failures after a successful log, and missing/invalid historical metadata after a
successful log.

**AC-5 -- Spacetop remains read-only toward workflow markdown.**
The fix may read git history and workflow files, but it does not add any path
that mutates workflow markdown.
Verified by: `cargo test -p spacetop-core --test no_write_git_calls`, the existing
history read-command guard in `crates/spacetop-core/src/git_history.rs`, diff
review showing only `git rev-parse`, `git log`, and `git show` history reads,
and final `make lint`.

## Proof plan

- Lowest test layer: `spacetop-core` git-history/index tests for the failing
  loader boundary, plus focused headless CLI tests for unavailable/error
  rendering.
- Required command: `make lint`
- Manual check, if any: run the three reproduction commands against
  `docs/spacetop-dev`, then open the TUI Metrics, Activity, and Timeline views.
- Docs/policy update needed: only if the user-facing unavailable messages or
  history-view behavior changes.

## Implementation plan

### 1. Pin the loader boundary before changing behavior

Owned files:

- `crates/spacetop-core/src/git_history.rs`
- `crates/spacetop-core/tests/git_history_fixtures.rs`

Steps:

1. Add a failing core unit test named
   `history_source_accepts_legacy_numeric_entity_id`. Use
   `RecordingGitRunner` responses for:
   - `rev-parse --is-shallow-repository` -> `false`
   - `git log ... -- docs/workflow/**` -> one add touch for
     `docs/workflow/_archive/scaffold-rust-cli-project.md`
   - `git show <commit>:docs/workflow/_archive/scaffold-rust-cli-project.md`
     -> frontmatter containing `id: 001`, `status: done`, and a title
   Expected result: `GitHistorySource::load` succeeds, returns one event, and
   preserves `event.entity_id == "001"` rather than `"1"`.
2. Add a failing core unit test named
   `log_success_show_failure_reports_blob_unavailable`. Queue a successful
   shallow probe, a successful log touch, and a failing `git show`. Expected
   result: the error variant and user message identify the historical blob read,
   not `git log`.
3. Add a failing core unit test named
   `log_success_metadata_failure_reports_metadata_unavailable`. Queue a
   successful shallow probe, a successful log touch, and a successful `git show`
   blob whose frontmatter lacks `status`. Expected result: the error variant and
   user message identify metadata extraction, not `git log`.
4. Keep or update the existing log-failure test so a failed `git log` still
   renders exactly `history unavailable: git log could not be read`.
5. Run the focused checks before implementation:

```bash
cargo test -p spacetop-core git_history::tests::history_source_accepts_legacy_numeric_entity_id
cargo test -p spacetop-core git_history::tests::log_success_show_failure_reports_blob_unavailable
cargo test -p spacetop-core git_history::tests::log_success_metadata_failure_reports_metadata_unavailable
```

Expected before the fix: the new tests fail, proving the boundary.

### 2. Make history metadata parsing match accepted entity IDs

Owned files:

- `crates/spacetop-core/src/parser/frontmatter.rs`
- `crates/spacetop-core/src/parser/item.rs`
- `crates/spacetop-core/src/git_history.rs`

Steps:

1. Extract a small pure helper from the existing flat frontmatter fallback into
   `parser/frontmatter.rs`, for example
   `pub(crate) fn top_level_scalar(frontmatter: &str, field: &str) -> Option<String>`.
   It should preserve the raw scalar text for simple `key: value` lines,
   unquote single- or double-quoted scalars, ignore blank/comment lines, and
   reject indented continuation lines plus flow/block values (`[`, `{`, `|`,
   `>`, `&`, `*`) just like the current item fallback.
2. Reuse that helper in `parser/item.rs::parse_flat_work_item_frontmatter` so
   the normal parser and history loader share the same lexical scalar behavior.
3. Update `git_history.rs::frontmatter_metadata` to read only `id` and `status`
   from the historical blob frontmatter, preserving lexical IDs such as `001`.
   Do not call `parse_work_item`: history must not require a current title field
   or validate historical statuses against the current README stage list.
4. Return a precise metadata error when either `id` or `status` is missing or
   malformed after extraction.
5. Run:

```bash
cargo test -p spacetop-core git_history::tests::history_source_accepts_legacy_numeric_entity_id
cargo test -p spacetop-core parser::tests::parses_flat_frontmatter_with_unquoted_colon_in_title
```

Expected after the fix: both pass, and the parser fallback still handles
unquoted colon titles.

### 3. Split unavailable reasons by failed operation

Owned files:

- `crates/spacetop-core/src/query.rs`
- `crates/spacetop-core/src/git_history.rs`
- `crates/spacetop-core/src/index.rs`
- `crates/spacetop/src/headless.rs`
- `crates/spacetop/src/ui/metrics.rs`
- `crates/spacetop/src/ui/activity.rs`
- `crates/spacetop/src/ui/timeline.rs`

Steps:

1. Replace the single catch-all `HistoryUnavailable::GitError(String)` surface
   with operation-specific variants. Recommended shape:
   - `GitLogError(String)` -> `history unavailable: git log could not be read`
   - `GitBlobError { path: String, message: String }` -> historical blob read
     failed
   - `MetadataError { path: String, message: String }` -> historical entity
     metadata could not be parsed
2. Change `HistoryUnavailable::user_message()` from `&str` to `String` only if
   dynamic path/detail text is needed. Keep stable prefix strings short and
   test-pinned.
3. Map errors at the exact operation boundary:
   - `ensure_not_shallow` keeps `NotGitRepository`, `ShallowClone`, or a
     rev-parse-specific git error if needed.
   - the `git log` command maps only to `GitLogError`.
   - `blob_metadata` maps `git show` process failures to `GitBlobError`.
   - `frontmatter_metadata` failures map to `MetadataError`.
4. Update `WorkflowIndex` tests that assert exact unavailable propagation; the
   index should remain a pass-through and must not rewrite reasons.
5. Run:

```bash
cargo test -p spacetop-core git_history
cargo test -p spacetop-core index::tests::history_methods_surface_exact_unavailable_reason
```

### 4. Cover headless metrics, activity, and timeline

Owned file:

- `crates/spacetop/src/headless.rs`

Steps:

1. Replace the current single-response `TestGitRunner` in `headless.rs` tests
   with a small queued test runner so one test can simulate `rev-parse`, `log`,
   and one or more `show` calls in order.
2. Add a text and JSON assertion named
   `history_commands_emit_downstream_metadata_unavailable`. It should cover
   `run_timeline`, `run_metrics`, and `run_activity`, and assert that all three
   commands render the metadata-specific unavailable message instead of the
   git-log message.
3. Add a success assertion named
   `history_commands_emit_events_for_legacy_numeric_id`. It should queue a
   successful history load from a blob with `id: 001`; expected output:
   - `timeline 001` prints at least one row for `001`
   - `activity` prints at least one row for `001`
   - `metrics` prints `completed_entities` and does not print an unavailable
     message
4. Keep the existing shallow, non-git, and true log-error tests.
5. Run:

```bash
cargo test -p spacetop headless::tests::history_commands_emit_downstream_metadata_unavailable
cargo test -p spacetop headless::tests::history_commands_emit_events_for_legacy_numeric_id
```

### 5. Cover TUI history views through shared app/index state

Owned files:

- `crates/spacetop/src/app/tests.rs`
- `crates/spacetop/src/ui/tests.rs`

Steps:

1. Extend `apply_history_result_surfaces_exact_unavailable_reason` or add a
   sibling test that applies the new metadata-specific unavailable variant and
   asserts `timeline`, `metrics`, and `activity` all return that exact variant.
2. Extend the Ratatui tests for `timeline`, `metrics`, and `activity` so each
   view renders the metadata-specific unavailable message from the index. This
   is enough for the TUI because all three renderers call
   `session.active_state().index()` and do not load history themselves.
3. Do not add terminal-only tests for this behavior. Manual TUI smoke is useful
   after the automated tests, but the acceptance proof should be the app/index
   and Ratatui `TestBackend` tests.
4. Run:

```bash
cargo test -p spacetop app::tests::apply_history_result_surfaces_exact_unavailable_reason
cargo test -p spacetop ui::tests::timeline_view_renders_unavailable_loading_empty_and_events
cargo test -p spacetop ui::tests::metrics_view_renders_unavailable_and_populated_metrics
cargo test -p spacetop ui::tests::activity_view_renders_unavailable_and_newest_events_first
```

### 6. Reproduce against the real workflow and decide docs impact

Owned files:

- `README.md` only if documented output strings or history behavior changed
- `docs/spacetop-dev/062-diagnose-history-unavailable-headless-views.md` for
  the implement stage report only

Steps:

1. Run the exact reproduction commands:

```bash
cargo run -p spacetop -- metrics --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- activity --workflow-dir docs/spacetop-dev
cargo run -p spacetop -- timeline 056 --workflow-dir docs/spacetop-dev
```

Expected after the likely numeric-ID fix: the commands produce history-derived
output, not an unavailable message. If a later historical blob is legitimately
unreadable or malformed, the command may print a precise blob/metadata
unavailable reason, but it must not say `git log could not be read` unless the
log command actually failed.

2. Check docs for pinned user-facing strings:

```bash
rg -n "history unavailable|git log could not be read|metrics|activity|timeline" README.md docs
```

Update `README.md` or nearby docs only if they describe the old unavailable
message or behavior. Do not rewrite unrelated Spacedock workflow files.

3. Run the required completion gate:

```bash
cargo test -p spacetop-core --test no_write_git_calls
cargo test
make lint
```

4. Optional manual smoke after tests pass: open the TUI on `docs/spacetop-dev`,
select Metrics (`M`), Activity (`A`), and Timeline (`T`) with preview closed,
and confirm those views match the corrected headless history result.

## Read-only safety boundary

The implementation may add read-only git probes under
`crates/spacetop-core/src/git_history.rs`, but it must not add workflow markdown
writes, git write subcommands, or config/session writes inside workflow
directories. The approved git history command set remains read-only:
`rev-parse`, `log`, `show`, and any existing read-only `rev-list` usage. The
existing `Y` sync path remains the only workflow-adjacent write path and must
stay limited to `git pull --ff-only`.

## Stage Report: plan

- DONE: Identify the exact history-loading boundaries to probe, including
  downstream failures after successful `git log`.
  The plan names the path from `headless::run_*` and TUI `history_worker` into
  `WorkflowIndex::load_with_history`, `GitHistorySource::ensure_not_shallow`,
  the `git log` command, `parse_touches`, per-touch `git show`, and
  `frontmatter_metadata`. Current plan-stage evidence shows the first likely
  downstream failure is accepted legacy frontmatter (`id: 001`) that `git show`
  reads successfully but the history metadata extractor does not accept.
- DONE: Specify focused regression coverage for metrics, activity, timeline,
  and TUI history views at the lowest practical layers.
  The implementation plan adds core `GitHistorySource` tests, real git fixture
  coverage, headless text/JSON tests for all three commands, app propagation
  tests, and Ratatui render assertions for Metrics, Activity, and Timeline.
- DONE: Explain read-only safety, reproduction commands, `make lint`, and docs
  impact.
  The plan lists the exact reproduction commands, requires
  `cargo test -p spacetop-core --test no_write_git_calls`, `cargo test`, and
  `make lint`, and limits docs edits to pinned history output or behavior.

### Summary

Planned the bugfix as a core history-loader correction, not a UI workaround. The
implementer should first prove whether the current failure is the legacy numeric
ID metadata boundary, then split unavailable reasons so successful `git log`
cannot be reported as a log failure when `git show` or metadata extraction is the
actual problem. The test plan covers core, headless, app propagation, and
Ratatui views while preserving Spacetop's read-only workflow contract.
