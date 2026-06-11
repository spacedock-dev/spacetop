# SpaceTop v2 — Design

**Status:** Approved design, ready for implementation planning
**Date:** 2026-06-11
**Author:** brainstorming session (captain: Kent)

## Summary

SpaceTop v2 is a major-version **internals rebuild** that keeps the product and
safety contract unchanged — SpaceTop stays a **read-only** TUI for browsing
Spacedock workflow state — while replacing the architecture so it can support a
new class of capabilities the current single-snapshot design cannot.

The driver is **ambition**, not pain: a set of new read-only capabilities
(time/history, metrics, search + command palette, activity feed, entity
relationships, theming) all depend on two foundations the app lacks today —
**git history as a data source** and an **indexed, queryable core that is
reusable without a terminal**. v2 builds that core and rebuilds the TUI on top
of it.

### Decisions locked in this session

| Question | Decision |
|----------|----------|
| Product direction | Stay **read-only**; rebuild internals (contract unchanged) |
| Driver | **Ambition** — unlock new capabilities |
| Headline capabilities | Time & history · Metrics & analytics · Search + command palette |
| Depth capabilities | Live activity feed · Entity relationships · Theming & config · Richer worktree/PR view |
| Scale target | **Medium** — hundreds of entities, growing archives → incremental reload matters |
| Surface | **TUI + headless/export** — core reusable behind a clean API |
| Migration path | **B** — Cargo workspace + fresh git-aware core, executed strangler-style |
| Git-history depth | **Key timestamps only** (status-change commits), seam left open for full replay |

## Goals

1. A **git-aware, indexed core** (`spacetop-core`) with a query API that the
   TUI, the CLI, and JSON export all consume — and that has **zero terminal
   dependencies**.
2. Read git **history** (status-change events per entity) to power dwell-time,
   cycle-time, "last changed", timelines, and an activity feed.
3. **Incremental** index updates (`apply_change(path)`) instead of full re-parse
   on every filesystem event.
4. A **rebuilt TUI** that consumes the query API, with `App`/`OverviewState`
   decoupled and the `Cell`-based render→input layout back-channel removed.
5. An **async event loop** so git operations and history ingestion never block
   the main thread.
6. New views — command palette + search, timeline, metrics, activity feed — each
   a thin consumer of one query method.
7. **Config / theming / session persistence**, respected by both TUI and
   headless surfaces.
8. A **strangler migration** that keeps a working, shipping app at every phase.

## Non-goals (explicit YAGNI cuts for v2)

- **No write operations.** The read-only contract is unchanged. The only
  sanctioned write remains `git pull --ff-only` (the `Y` sync action). The
  `no_write_git_calls` guardrail moves into core and is extended to cover
  git-history (only `log`/`rev-list`/`show`).
- **No portfolio view.** Not selected. The index naturally spans workflows, so a
  portfolio/cross-workflow dashboard stays cheap to add later — but it is out of
  scope for v2.
- **No full git replay.** Key timestamps only (when each entity entered each
  stage). A seam is left so full per-field replay can be added later without
  reshaping the index.
- **No real search engine** (e.g. tantivy). In-memory substring/fuzzy matching
  over the index is sufficient for hundreds of entities.
- **No async for its own sake.** Async is introduced only where it removes
  main-thread blocking (git ops, history ingestion). The editor still suspends
  the TUI synchronously — that is correct behavior.
- **No greenfield rewrite.** Battle-tested logic (worktree SHA-1 merge, YAML
  flat-parse fallback, watcher debounce/relevance filtering, OSC 7, symlink-cycle
  -safe discovery, sentry wiring) is **ported**, not rewritten.

---

## Architecture

### Workspace structure

Today SpaceTop is a single binary crate. v2 restructures it into a Cargo
workspace where the core boundary is **physical** (compiler-enforced), not a
convention.

```
spacetop/                      # workspace root
├── Cargo.toml                 # [workspace] members
├── crates/
│   ├── spacetop-core/         # zero terminal deps — the spine
│   ├── spacetop-tui/          # ratatui/crossterm — consumes core
│   └── spacetop/              # bin: clap CLI → launch TUI or run headless
├── docs/
└── tests/                     # workspace-level integration tests (optional)
```

**The load-bearing rule:** `spacetop-core` must not depend on `ratatui`,
`crossterm`, `termimad`, `ratskin`, or any terminal crate. This is what makes
"reusable headless" enforced rather than aspirational. A CI check (or a simple
`cargo tree` assertion test) guards the rule.

Crate responsibilities:

- **`spacetop-core`** (lib): domain model, parser, git-history, worktree scan/
  merge, the `WorkflowIndex`, the query API, metrics, config, filesystem watcher,
  git sync. Pure logic.
- **`spacetop-tui`** (lib): rendering, async event loop, input handling, view
  state, theming application. Depends on `spacetop-core`.
- **`spacetop`** (bin): clap entry point. With no args / `-w` it launches the TUI
  (depends on `spacetop-tui` + `spacetop-core`). Headless subcommands depend on
  `spacetop-core` only. Sentry init lives here (release builds).

### Core spine (new subsystems)

#### Sources (read-only inputs)

- **`WorkingTreeSource`** — port of today's parser: scan a workflow dir, split
  frontmatter, parse entities, capture per-entity parse errors. Largely as-is.
- **`WorktreeSource`** — port of today's `.worktrees/*` and `.claude/worktrees/*`
  scan + SHA-1 content merge.
- **`GitHistorySource`** *(new)* — reads `git log` for the workflow directory and
  extracts **status-change events**: for each entity, the commits where its
  `status:` frontmatter field changed, with commit timestamps. Read-only
  (`log`/`rev-list`/`show` only). Degrades gracefully when git is unavailable.

#### Model

- **`Entity`** — evolution of today's `WorkItem` (path, id, title, status,
  source, started/completed, verdict, score, worktree, issue, pr, body, worktree
  provenance fields).
- **`WorkflowDefinition`**, **`StageDefinition`**, **`StageTransition`** — ported,
  with oklch stage colors and stage prose.
- **`StageEvent`** *(new)* — `{ entity_id, from: Option<String>, to: String, at:
  CommitTime, commit: CommitId }`. The atom of history. Derived by
  `GitHistorySource`.

#### `WorkflowIndex`

Built from the sources; the single in-memory structure the query API reads from.
Holds:

- entities keyed by id and by slug,
- entities grouped by stage,
- a lightweight **text index** (substring/fuzzy over id + title + body),
- a **relationship graph** (blocks / blocked-by / references — parsed from
  frontmatter fields and/or body links),
- a **per-entity timeline** (the `StageEvent`s for that entity).

**Incremental updates:** the index exposes `apply_change(path)` to re-parse a
single entity file and patch the affected index structures, rather than
rebuilding the whole index on every watcher tick. Full `build()` is used for
first load and re-discovery; `apply_change` for steady-state edits.

#### Query API (the headless-reusable surface)

All outputs are `serde`-serializable so the same calls back the TUI and JSON
export. Indicative shape (exact signatures settled in the P1 plan):

- `query(filter) -> Vec<&Entity>` — by status, field predicate, or text match.
- `metrics(workflow) -> Metrics` — stage dwell-time, cycle-time, WIP per stage,
  throughput; all derived from `StageEvent`s.
- `timeline(entity) -> Vec<StageEvent>` — ordered history for one entity.
- `activity(since) -> Vec<ActivityEvent>` — recent changes across workflows.
- `related(entity) -> Vec<(Relation, &Entity)>` — relationship-graph neighbors.

The TUI and CLI call **only** this API — never index internals.

#### Config

Loaded from a config file (location TBD in P4 plan — likely XDG config dir, with
a workflow-local override). Carries theme, keybindings, default sort/scope, and
per-workflow preferences. Session persistence (selected entity + scope per
workflow) is written back so selection survives restarts. Config lives in core so
headless surfaces honor the same settings.

### TUI layer (rebuilt on top)

- **Decouple `App` from `OverviewState`.** A dedicated **layout pass** computes
  pane geometry once per frame; both render and input read it. This removes the
  `Cell<usize>` interior-mutability back-channel currently used to pass layout
  info from render to input handling.
- **Async event loop.** Replace the 100ms poll with a select over: the crossterm
  event stream, watcher refresh signals, and completion of background jobs (git
  pull, history ingestion). Long-running git work runs off-thread; the UI thread
  stays responsive. The editor handoff still **suspends** the TUI synchronously.
- **New views**, each a thin consumer of one query method: command palette +
  search overlay, timeline view, metrics view, activity feed. Theming reads
  colors and keybindings from `Config`.

### CLI / headless

The `spacetop` binary keeps current launch behavior (no args → discover and open
TUI; `-w <path>` → open that workflow). v2 adds headless subcommands over the
core query API — indicatively `list`, `metrics`, `export --json` (exact surface
defined in the P5 plan). These depend on `spacetop-core` only and emit
serde-serialized output for scripting.

### Data flow

```
working tree ─┐
git log ──────┤→ Sources → WorkflowIndex ──→ Query API ──┬─→ TUI views
worktrees ────┘                  ↑                        └─→ CLI / JSON export
                  watcher signal → apply_change(path)   (incremental)
```

### Error handling

- **Per-entity parse errors** keep surfacing as synthetic "broken" rows in the
  list — and this is **extended to the archive scope** (a current gap where
  archive parse errors are silently dropped).
- **Git-history failures degrade gracefully.** No git repo, a shallow clone, or
  an unparseable log → history-derived features (timeline, metrics, activity)
  render an "unavailable" state with a hint, mirroring today's `SyncAvailability`
  pattern. Never a crash.
- **Read-only guardrail** moves into `spacetop-core` and is extended: the test
  asserts git-history only ever invokes read-only subcommands
  (`log`/`rev-list`/`show`), alongside the existing assertion that sync only runs
  `pull --ff-only`.

### Testing

- **Core is pure logic** → comprehensive unit + integration tests with no
  terminal. The trait-seam discipline already present (`GitRunner`, `EditorEnv`,
  `EditorLauncher`) is now physically enabled by the crate boundary.
- **`GitHistorySource`** → fixture repositories built with `tempfile` + real
  `git init` and scripted commits (extends the existing `git_sync_e2e` pattern).
- **Query API** → golden tests over fixture workflows.
- **TUI** → keep `TestBackend` render tests and `find_text`/list-pane-scoped
  assertions.

---

## Migration phasing (strangler)

Each phase ships independently; the app stays working throughout. P0–P2 build the
spine and are specified in depth; P3–P5 layer capabilities and are sketched here
to be detailed when we reach them.

### P0 — Workspace restructure *(behavior-identical)*

Stand up the Cargo workspace and move existing code into `spacetop-core`,
`spacetop-tui`, and the `spacetop` bin **without changing behavior**. All current
tests pass unchanged (relocated as needed). Add the "core has no terminal deps"
CI guard. This is pure mechanical restructuring — the riskiest part is import
churn, so it lands first and isolated.

**Done when:** workspace builds, `make lint` clean, full existing test suite
green, core depends on no terminal crate.

### P1 — Index + Query API over existing sources *(no new features)*

Introduce `WorkflowIndex` and the query API built from the existing
working-tree/worktree sources (no history yet). Migrate the list and preview to
read from the query API instead of iterating `Vec<WorkItem>` directly. Implement
`apply_change(path)` and wire the watcher to it for incremental reload.

**Done when:** the TUI renders entirely off the query API; incremental reload
patches the index instead of full rebuild; query API has golden tests.

### P2 — Git-history source *(key timestamps)*

Add `GitHistorySource` producing `StageEvent`s (status-change commits with
timestamps), fold them into the index as per-entity timelines, and add the
graceful-degradation path. No UI surface yet beyond what naturally fits (e.g. a
"last changed" value); the data is now available for P3.

**Done when:** `timeline(entity)` returns correct ordered events on fixture
repos; history failures degrade to "unavailable"; read-only guardrail covers the
new git calls.

### P3 — Capabilities *(sketch)*

Build on the spine: search + command palette overlay, metrics view (dwell/cycle/
WIP/throughput), timeline view, activity feed, and entity-relationship
navigation. Each is a consumer of one query method. Detailed per-feature plans to
be written when P2 lands and the query API's real shape is known.

### P4 — Config / theming / persistence *(sketch)*

Config file (theme, keybindings, default sort/scope), session persistence
(selection/scope per workflow), honored by both TUI and headless.

### P5 — CLI headless + export *(sketch)*

Headless subcommands (`list`, `metrics`, `export --json`, …) over the core query
API, with serde-serialized output for scripting.

---

## Open questions (to resolve in the relevant phase plan)

- **Git-history full-replay seam (P2):** confirm the `StageEvent` shape leaves
  room to add per-field change events later without reshaping the index.
- **Config location & format (P4):** XDG config dir vs workflow-local vs both;
  TOML vs YAML (YAML already in the dependency set).
- **CLI subcommand surface (P5):** exact command names, flags, and JSON schema.
- **Relationship source (P3):** which frontmatter fields / body link conventions
  define `blocks`/`blocked-by`/`references`.
- **Async runtime choice (P1/P3):** full `tokio` vs a lighter
  `std::sync::mpsc` + thread select. Decide based on what the off-thread git work
  actually needs; prefer the lightest option that removes blocking.
