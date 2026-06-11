# SpaceTop v2 - Phase P5: Headless CLI + Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add headless CLI subcommands over the core query API for scripting and JSON export while preserving the current no-argument TUI launch behavior.

**Architecture:** The existing `spacetop` binary remains the only binary. Headless subcommands resolve one workflow, load `WorkflowIndex`, and print either compact text or JSON. The optional third crate is not introduced in P5 unless a measured build-time problem is documented; the baseline remains the two-crate P0 workspace.

**Tech Stack:** Rust 2021, `clap`, `spacetop-core`, `serde`, new `serde_json` dependency in the bin crate only, existing config loading.

---

## Prerequisites

- P0 through P4 are merged.
- Core query/index/metrics/timeline/activity APIs are serializable.
- Config loading is available in core.

## Hard constraints

- Do not expose a subcommand variant until its handler is implemented and dispatched in the same task.
- Headless commands resolve exactly one workflow before loading an index.
- Headless commands honor P4 config defaults unless a CLI flag explicitly overrides them.
- History commands use an injectable git runner in tests so shallow clone, non-git, and generic git-error outputs are stable.

## CLI surface

Existing behavior stays:

```bash
spacetop
spacetop --workflow-dir docs/spacetop-dev
```

New headless commands:

```bash
spacetop list --workflow-dir docs/spacetop-dev
spacetop list --workflow-dir docs/spacetop-dev --status verify --text sync --json
spacetop timeline 050 --workflow-dir docs/spacetop-dev --json
spacetop metrics --workflow-dir docs/spacetop-dev --json
spacetop activity --workflow-dir docs/spacetop-dev --json
spacetop export --workflow-dir docs/spacetop-dev --json
```

Headless commands require exactly one workflow. If `--workflow-dir` is omitted and discovery finds zero or multiple workflows, they exit non-zero with a stable stderr hint.

## File map

- Modify: `crates/spacetop/Cargo.toml`
- Modify: `crates/spacetop/src/cli.rs`
- Modify: `crates/spacetop/src/lib.rs`
- Create: `crates/spacetop/src/headless.rs`
- Modify: `crates/spacetop-core/src/index.rs` if export model helpers are needed
- Modify: `README.md`

---

## Task 1: Add CLI subcommand definitions

**Files:**
- Modify: `crates/spacetop/src/cli.rs`

- [ ] **Step 1: Write CLI parse tests**

Add tests:

```rust
#[test]
fn parses_list_subcommand_with_filters() {
    let cli = Cli::parse_from([
        "spacetop",
        "list",
        "--workflow-dir",
        "docs/spacetop-dev",
        "--status",
        "verify",
        "--text",
        "sync",
        "--json",
    ]);
    match cli.command {
        Some(Command::List(args)) => {
            assert_eq!(args.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
            assert_eq!(args.status.as_deref(), Some("verify"));
            assert_eq!(args.text.as_deref(), Some("sync"));
            assert!(args.json);
        }
        other => panic!("expected list command, got {other:?}"),
    }
}

#[test]
fn no_subcommand_still_launches_tui_shape() {
    let cli = Cli::parse_from(["spacetop", "--workflow-dir", "docs/spacetop-dev"]);
    assert!(cli.command.is_none());
    assert_eq!(cli.workflow_dir, Some(PathBuf::from("docs/spacetop-dev")));
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop cli::tests::parses_list_subcommand_with_filters`

Expected: FAIL because subcommands do not exist.

- [ ] **Step 3: Implement clap types**

In `cli.rs`, add:

```rust
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    List(ListArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct ListArgs {
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub text: Option<String>,
    #[arg(long)]
    pub scope: Option<ListScopeArg>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ListScopeArg {
    Active,
    Archived,
    All,
}
```

Extend `Cli`:

```rust
#[command(subcommand)]
pub command: Option<Command>,
```

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop cli::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/cli.rs
git commit -m "feat(cli): define headless subcommands"
```

---

## Task 2: Add headless workflow resolver

**Files:**
- Create: `crates/spacetop/src/headless.rs`
- Modify: `crates/spacetop/src/lib.rs`

- [ ] **Step 1: Write resolver tests**

Create `crates/spacetop/src/headless.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn explicit_workflow_path_canonicalizes_and_resolves_direct_workflow() {
        let path = PathBuf::from("docs/spacetop-dev");
        let cwd = std::env::current_dir().expect("cwd");
        let resolved = resolve_workflow_arg(Some(path.clone()), &cwd)
            .expect("resolve");
        assert!(resolved.workflow_dir.ends_with("docs/spacetop-dev"));
        assert!(resolved.repo_root.is_absolute());
    }

    #[test]
    fn explicit_scan_root_must_discover_exactly_one_workflow() {
        let root = fixture_repo_with_one_workflow();
        let resolved = resolve_workflow_arg(Some(root.path().to_path_buf()), root.path())
            .expect("resolve");
        assert!(resolved.workflow_dir.ends_with("docs/workflow"));
    }

    #[test]
    fn omitted_path_rejects_zero_or_multiple_workflows() {
        let empty = tempfile::tempdir().expect("tempdir");
        let err = resolve_workflow_arg(None, empty.path()).unwrap_err().to_string();
        assert!(err.contains("headless command requires exactly one workflow"));

        let root = fixture_repo_with_two_workflows();
        let err = resolve_workflow_arg(None, root.path()).unwrap_err().to_string();
        assert!(err.contains("headless command requires exactly one workflow"));
    }
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop headless::tests`

Expected: FAIL because resolver types do not exist.

- [ ] **Step 3: Implement resolver shell**

Add:

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlessWorkflow {
    pub workflow_dir: PathBuf,
    pub repo_root: PathBuf,
    pub workflow_rel: String,
}

pub fn resolve_workflow_arg(
    workflow_dir: Option<PathBuf>,
    cwd: &Path,
) -> anyhow::Result<HeadlessWorkflow> {
    let requested = workflow_dir
        .map(|path| if path.is_absolute() { path } else { cwd.join(path) })
        .unwrap_or_else(|| cwd.to_path_buf())
        .canonicalize()?;
    let workflows = spacetop_core::discovery::discover_workflows(&requested)?;
    if workflows.len() != 1 {
        anyhow::bail!("headless command requires exactly one workflow; pass --workflow-dir");
    }
    let workflow_dir = workflows[0].root.clone();
    let repo_root = spacetop_core::discovery::resolve_scan_root(&workflow_dir);
    let workflow_rel = workflow_dir
        .strip_prefix(&repo_root)
        .unwrap_or(&workflow_dir)
        .to_string_lossy()
        .into_owned();
    Ok(HeadlessWorkflow {
        workflow_dir,
        repo_root,
        workflow_rel,
    })
}
```

Direct workflow paths, explicit repo/root paths, and omitted paths all run core discovery from the canonical requested path and require exactly one discovered workflow. Discovery includes the root path itself, so a direct workflow directory still resolves without a new `is_workflow_dir` API. Do not open the TUI picker for headless commands.

- [ ] **Step 4: Export module and verify**

In `lib.rs`, add:

```rust
mod headless;
```

Run: `cargo test -p spacetop headless::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/lib.rs crates/spacetop/src/headless.rs
git commit -m "feat(cli): resolve one workflow for headless commands"
```

---

## Task 3: Implement `list`

**Files:**
- Modify: `crates/spacetop/src/headless.rs`
- Modify: `crates/spacetop/src/lib.rs`
- Modify: `crates/spacetop/Cargo.toml`

- [ ] **Step 1: Add dependency**

In `crates/spacetop/Cargo.toml`, add:

```toml
serde_json = "1"
```

Keep it in the bin crate unless core needs JSON-specific helpers.

- [ ] **Step 2: Write list command test**

Add a test:

```rust
#[test]
fn list_json_outputs_entities() {
    let workflow = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/slug-workflow");
    let mut out = Vec::new();
    run_list(
        ListArgs {
            workflow_dir: Some(workflow),
            status: None,
            text: Some("roadmap".to_string()),
            scope: None,
            json: true,
        },
        &spacetop_core::config::SpacetopConfig::default(),
        &mut out,
    )
    .expect("list");
    let body = String::from_utf8(out).expect("utf8");
    assert!(body.contains("\"id\""));
    assert!(body.contains("roadmap-v5"));
}
```

- [ ] **Step 3: Run and verify it fails**

Run: `cargo test -p spacetop headless::tests::list_json_outputs_entities`

Expected: FAIL because `run_list` does not exist.

- [ ] **Step 4: Implement list**

Add:

```rust
pub fn run_list(
    args: crate::cli::ListArgs,
    config: &spacetop_core::config::SpacetopConfig,
    out: &mut impl std::io::Write,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let resolved = resolve_workflow_arg(args.workflow_dir, &cwd)?;
    let mut index = spacetop_core::index::WorkflowIndex::load(
        &resolved.workflow_dir,
        &resolved.repo_root,
    )?;
    let scope = match args.scope {
        Some(crate::cli::ListScopeArg::Active) => spacetop_core::query::QueryScope::Active,
        Some(crate::cli::ListScopeArg::Archived) => spacetop_core::query::QueryScope::Archived,
        Some(crate::cli::ListScopeArg::All) => spacetop_core::query::QueryScope::All,
        None => match config.defaults.scope {
            spacetop_core::config::DefaultScope::Active => {
                spacetop_core::query::QueryScope::Active
            }
            spacetop_core::config::DefaultScope::Archived => {
                spacetop_core::query::QueryScope::Archived
            }
        },
    };
    if matches!(
        scope,
        spacetop_core::query::QueryScope::Archived | spacetop_core::query::QueryScope::All
    ) {
        let archive = spacetop_core::sources::WorkflowSources::load_archive(
            &resolved.workflow_dir,
            index.definition(),
        );
        index = index.with_archive(archive);
    }
    let sort = match config.defaults.sort {
        spacetop_core::config::DefaultSort::Id => spacetop_core::query::EntitySort::Id,
        spacetop_core::config::DefaultSort::Status => spacetop_core::query::EntitySort::Status,
    };
    let entities = index.query(spacetop_core::query::EntityQuery {
        scope,
        status: args.status,
        text: args.text,
        sort,
        ..spacetop_core::query::EntityQuery::default()
    });
    if args.json {
        serde_json::to_writer_pretty(out, &entities)?;
        writeln!(out)?;
    } else {
        for entity in entities {
            writeln!(out, "{}\t{}\t{}", entity.id, entity.status, entity.title)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Dispatch from `run`**

In `lib.rs::run`, before TUI decision:

```rust
if let Some(command) = cli.command.clone() {
    return headless::run_command(command);
}
```

`run_command` should load P4 config once with `load_config_with_warnings(&StdEnv)`, print config warnings to stderr only in text mode, and dispatch only the implemented `list` branch in this task. Add the remaining `Command` enum variants, match arms, and handlers in Task 4 and Task 5 when their handlers are implemented, so no shipped command path returns a placeholder error.

- [ ] **Step 6: Verify and commit**

Run: `cargo test -p spacetop headless::tests::list_json_outputs_entities`

Expected: PASS.

```bash
git add crates/spacetop/Cargo.toml crates/spacetop/src/headless.rs crates/spacetop/src/lib.rs
git commit -m "feat(cli): add headless list command"
```

---

## Task 4: Implement timeline, metrics, and activity commands

**Files:**
- Modify: `crates/spacetop/src/cli.rs`
- Modify: `crates/spacetop/src/headless.rs`

- [ ] **Step 1: Add CLI variants and parse tests**

In `cli.rs`, extend `Command` only now:

```rust
pub enum Command {
    List(ListArgs),
    Timeline(TimelineArgs),
    Metrics(WorkflowOutputArgs),
    Activity(WorkflowOutputArgs),
}
```

Add `TimelineArgs`:

```rust
#[derive(Debug, Clone, clap::Args)]
pub struct WorkflowOutputArgs {
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, clap::Args)]
pub struct TimelineArgs {
    pub entity_id: String,
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}
```

Add parse tests for `timeline 050 --json`, `metrics --json`, and `activity --json`.

- [ ] **Step 2: Write unavailable tests**

Add tests that run each command through an injectable `GitRunner` and assert exact JSON/text responses for:

- `HistoryUnavailable::ShallowClone`
- `HistoryUnavailable::NotGitRepository`
- `HistoryUnavailable::GitError("fatal: bad object".to_string())`

Use `HistoryUnavailable::user_message()` in assertions. Do not depend on the user's actual git checkout.

- [ ] **Step 3: Implement shared JSON helper**

Add:

```rust
#[derive(serde::Serialize)]
struct UnavailableOutput<'a> {
    unavailable: &'a str,
}

fn write_json<T: serde::Serialize>(out: &mut impl std::io::Write, value: &T) -> anyhow::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)?;
    Ok(())
}
```

- [ ] **Step 4: Implement `run_timeline`**

Load `WorkflowIndex` with history using a generic `R: GitRunner`, call `timeline(entity_id)`, and output either events or:

```json
{ "unavailable": "history unavailable: shallow clone" }
```

using `HistoryUnavailable::user_message()`.

- [ ] **Step 5: Implement `run_metrics`**

Call `index.metrics()` and output metrics or unavailable JSON/text.

- [ ] **Step 6: Implement `run_activity`**

Call `index.activity(None)` and output newest-first events or unavailable JSON/text.

- [ ] **Step 7: Dispatch, verify, and commit**

Update `run_command` to dispatch `Timeline`, `Metrics`, and `Activity` with `StdGitRunner`. Keep `List` on the non-history load path.

Run: `cargo test -p spacetop headless::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/cli.rs crates/spacetop/src/headless.rs
git commit -m "feat(cli): add headless history and metrics commands"
```

---

## Task 5: Implement full JSON export

**Files:**
- Modify: `crates/spacetop/src/cli.rs`
- Modify: `crates/spacetop/src/headless.rs`

- [ ] **Step 1: Add export CLI variant and parse test**

In `cli.rs`, add `Export(WorkflowOutputArgs)` to `Command` only in this task. Add a parse test for:

```bash
spacetop export --workflow-dir docs/spacetop-dev --json
```

- [ ] **Step 2: Write export test**

Add:

```rust
#[test]
fn export_json_contains_definition_and_entities() {
    let workflow = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/slug-workflow");
    let mut out = Vec::new();
    run_export(
        WorkflowOutputArgs {
            workflow_dir: Some(workflow),
            json: true,
        },
        &mut out,
    )
    .expect("export");
    let body = String::from_utf8(out).expect("utf8");
    assert!(body.contains("\"definition\""));
    assert!(body.contains("\"entities\""));
}
```

- [ ] **Step 3: Run and verify it fails**

Run: `cargo test -p spacetop headless::tests::export_json_contains_definition_and_entities`

Expected: FAIL because `run_export` does not exist.

- [ ] **Step 4: Add export shape**

Add:

```rust
#[derive(serde::Serialize)]
struct ExportOutput {
    definition: spacetop_core::domain::WorkflowDefinition,
    entities: Vec<spacetop_core::domain::Entity>,
    archived_entities: Vec<spacetop_core::domain::Entity>,
}
```

P1 already requires `WorkflowDefinition`, `Entity`, and nested core DTOs to derive `Serialize`. Add a P5 regression test that serializes `ExportOutput`; if it fails, fix the missing core derive at the source and add the smallest core serialization test that proves the nested type is covered.

- [ ] **Step 5: Implement export**

`run_export` loads the active index, attaches archive data explicitly, and writes:

```rust
let archive = spacetop_core::sources::WorkflowSources::load_archive(
    &resolved.workflow_dir,
    index.definition(),
);
let index = index.with_archive(archive);

ExportOutput {
    definition: index.definition().clone(),
    entities: index.query(EntityQuery::default()),
    archived_entities: index.query(EntityQuery {
        scope: QueryScope::Archived,
        ..EntityQuery::default()
    }),
}
```

For `export`, require `--json`; if omitted, return an error:

```text
spacetop export requires --json
```

- [ ] **Step 6: Dispatch, verify, and commit**

Update `run_command` to dispatch `Export` only after `run_export` passes tests.

Run: `cargo test -p spacetop headless::tests::export_json_contains_definition_and_entities`

Expected: PASS.

```bash
git add crates/spacetop/src/cli.rs crates/spacetop/src/headless.rs crates/spacetop-core/src/domain/mod.rs
git commit -m "feat(cli): add JSON export command"
```

---

## Task 6: Pin resolver and config-default behavior

**Files:**
- Modify: `crates/spacetop/src/headless.rs`

- [ ] **Step 1: Keep resolver ambiguity tests in the main resolver task**

Task 2 already implemented the resolver rule. Confirm it includes tests using temp directories with zero and two workflow READMEs. Assert errors contain:

```text
headless command requires exactly one workflow
```

- [ ] **Step 2: Add config-default list tests**

Add tests proving:

- config default `scope = archived` applies when `list` has no `--scope`
- `list --scope active` overrides config default `scope = archived`
- config default sort applies when no CLI sort flag exists

If a future sort flag is added, its CLI value must override the config default.

- [ ] **Step 3: Verify and commit**

Run: `cargo test -p spacetop headless::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/headless.rs
git commit -m "test(cli): pin headless resolver and config defaults"
```

---

## Task 7: Re-evaluate third crate split and document decision

**Files:**
- Modify: `docs/development-policy.md`
- Modify: `README.md`

- [ ] **Step 1: Measure build impact and artifact need**

Run:

```bash
cargo clean
time cargo build -p spacetop
```

Also record the release binary size if P5 is fired in a release-prep context:

```bash
cargo build -p spacetop --release
ls -lh target/release/spacetop
```

Record the elapsed wall time and size in the P5 workflow entity review notes when this phase is fired.

- [ ] **Step 2: Apply Decision Tabs**

Use the Decision Tabs format from `docs/development-policy.md`:

Recommended: keep the P0 two-crate workspace for P5.

Pros:

- no extra workspace topology during CLI delivery
- keeps TUI and headless dispatch in one binary, matching the user-facing command
- `spacetop-core` remains terminal-free and reusable

Cons:

- headless-only builds still compile terminal dependencies
- a future packaging target may need a smaller non-TUI artifact

Only introduce a third crate in P5 if at least one threshold is met and documented:

- terminal/UI dependencies account for more than 30% of a clean `cargo build -p spacetop` wall time
- release artifact size is blocking a real distribution target
- a downstream consumer needs a TUI-free binary artifact during this phase

If none of those thresholds is met, keep the two-crate layout and document the deferral.

- [ ] **Step 3: Document no-split decision**

Add to `docs/development-policy.md`:

```markdown
The P5 headless CLI remains in the `spacetop` binary crate. `spacetop-core` is still terminal-free; a separate `spacetop-tui` crate is deferred until a measured build or artifact-size problem justifies it.
```

- [ ] **Step 4: Update README CLI examples**

Add command examples for `list`, `timeline`, `metrics`, `activity`, and `export --json`.

- [ ] **Step 5: Full verification**

Run:

```bash
cargo test --workspace
make lint
cargo run -p spacetop -- list --workflow-dir docs/spacetop-dev --json
cargo run -p spacetop -- export --workflow-dir docs/spacetop-dev --json
```

Expected: tests/lint PASS; CLI commands print JSON and exit 0.

- [ ] **Step 6: Commit**

```bash
git add README.md docs/development-policy.md
git commit -m "docs: document headless CLI surface and crate split decision"
```

## Definition of done (P5)

- [ ] Existing TUI launch behavior is unchanged.
- [ ] `list` supports status/text/scope filters, P4 config defaults, and JSON/text output.
- [ ] `timeline`, `metrics`, and `activity` output data or stable unavailable responses.
- [ ] `export --json` emits definition, active entities, and archived entities.
- [ ] Headless commands reject zero/multiple workflow discovery.
- [ ] The third-crate split is explicitly deferred or justified by Decision Tabs plus measurement thresholds.
- [ ] `cargo test --workspace` passes.
- [ ] `make lint` passes.
