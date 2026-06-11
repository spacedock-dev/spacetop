# SpaceTop v2 — Design

**Status:** Design under review — revised after adversarial review (see Review history)
**Date:** 2026-06-11
**Author:** brainstorming session (captain: Kent)

## Summary

SpaceTop v2 is a major-version **internals rebuild** that keeps the product and
safety contract unchanged — SpaceTop stays a **read-only** TUI for browsing
Spacedock workflow state — while replacing the architecture so it can support a
new class of capabilities the current single-snapshot design cannot.

The driver is **ambition**, not pain: a set of new read-only capabilities
(time/history, metrics, search + command palette, activity feed, richer
worktree/PR view, theming) all depend on two foundations the app lacks today —
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
| Scale target | **Medium** — hundreds of entities, growing archives |
| Surface | **TUI + headless/export** — core reusable behind a clean API |
| Migration path | **B** — Cargo workspace + fresh git-aware core, executed strangler-style |
| Git-history depth | **Key timestamps only** (status-change commits + archive-move), seam left open for full replay |

### Review adjustments (after adversarial review — see Review history for evidence)

The brainstorming decisions above are preserved, but ground-truth review against
the actual repo changed **how** several are executed:

- **Git history (A):** the terminal `done` transition is **never written to
  entity frontmatter** — entities are archived at `status: review` (verified:
  `048`, `049`). Done-ness is derived from the **archive-move rename**, not a
  frontmatter value. History derivation is redefined to diff the **frontmatter
  region only** of **entity files only** (avoiding body/README decoys), uses
  `--first-parent` traversal, and **hard-detects shallow clones**.
- **Entity relationships (E):** **no** `blocks`/`blocked-by`/`references` field
  exists in any entity or in the Spacedock schema, and there are no body-link
  conventions. The entity-to-entity dependency graph is **cut**. "Relationships"
  is re-scoped to the cross-references that actually exist: `issue`/`pr` links
  (entity → external) and `feedback-to` **stage** transitions (workflow-level).
- **Query API returns (B):** returns **owned snapshots / stable ids**, not
  `&Entity` borrows — borrows cannot survive across TUI frames while the index is
  rebuilt. Cloning is trivial at this scale (~48 entities, <1 MB total).
- **Incremental indexing (C):** **cut for v2.** Full index rebuild on each
  (debounced) watcher tick; the corpus rebuilds in single-digit ms. `apply_change`
  is added later only if a benchmark shows rebuild >16 ms.
- **Async runtime (D):** **cut.** No `tokio`. Off-thread git work uses a
  background thread + `mpsc` result channel folded into the existing poll loop —
  the exact pattern the watcher already uses.

## Goals

1. A **git-aware, indexed core** (`spacetop-core`) with a query API that the
   TUI, the CLI, and JSON export all consume — and that has **zero terminal
   dependencies**.
2. Read git **history** (status-change events + archive-move, per entity) to
   power dwell-time, cycle-time, "last changed", timelines, and an activity feed
   — using the rigorous derivation in the GitHistorySource section, not a naive
   `status:` grep.
3. A **rebuilt TUI** that consumes the query API, with `App`/`OverviewState`
   decoupled and the Cell-based render→input layout back-channel removed.
4. **Off-thread git work** (pull, history ingestion) via a background thread +
   result channel, so the UI thread never blocks. No async runtime.
5. New views — command palette + search, timeline, metrics, activity feed,
   richer worktree/PR view — each a thin consumer of one query method.
6. **Config / theming / session persistence**, respected by both TUI and
   headless surfaces.
7. A **strangler migration** that keeps a working, shipping app at every phase.

## Non-goals (explicit YAGNI cuts for v2)

- **No write operations.** The read-only contract is unchanged. The only
  sanctioned write remains `git pull --ff-only` (the `Y` sync action). The
  `no_write_git_calls` guardrail moves into core and is extended to cover
  git-history (only `log`/`rev-list`/`show`).
- **No async runtime.** A synchronous poll loop plus background worker threads +
  `mpsc` channels covers every blocking case (the watcher already proves the
  pattern). `tokio`/`async`/`.await` are explicitly out.
- **No incremental index.** Full rebuild per change for v2; revisit only behind a
  benchmark.
- **No entity-to-entity dependency graph.** No source data exists. Re-scoped to
  `issue`/`pr` and `feedback-to` (see Review adjustments). A real dependency
  graph would require a Spacedock schema extension — out of scope.
- **No portfolio view, and no multi-workflow index.** v2 loads **one workflow at
  a time** (`-w` or the discovered/selected workflow). `activity()` is scoped to
  the loaded workflow. A cross-workflow portfolio is a future capability that
  would require aggregating per-workflow indices.
- **No full git replay.** Key timestamps only (stage entry times + archive-move).
  A seam is left so full per-field replay can be added later without reshaping
  the index.
- **No real search engine** (e.g. tantivy). In-memory substring/fuzzy matching
  over the index is sufficient for hundreds of entities.
- **No greenfield rewrite.** Battle-tested logic (worktree SHA-1 merge, YAML
  flat-parse fallback, watcher debounce/relevance filtering, OSC 7, symlink-cycle
  -safe discovery, sentry wiring) is **ported**, not rewritten.

---

## Architecture

### Workspace structure

Today SpaceTop is a single binary crate. v2 restructures it so the **core/non-core
boundary is physical** (compiler-enforced), not a convention.

**Baseline: two crates.**

```
spacetop/                      # workspace root
├── Cargo.toml                 # [workspace] members
├── crates/
│   ├── spacetop-core/         # zero terminal deps — the spine (lib)
│   └── spacetop/              # bin: TUI + CLI; depends on spacetop-core
├── docs/
└── tests/                     # workspace-level integration tests
```

The boundary that earns its keep is **`spacetop-core` must not depend on
`ratatui`, `crossterm`, `termimad`, `ratskin`, or any terminal crate** — guarded
by a `cargo tree`/dependency assertion test in CI. That single rule makes the
headless surface real.

**Optional third crate (deferred):** split the TUI into `spacetop-tui` only if a
concrete need appears — namely, the headless CLI must *build* without compiling
ratatui (faster headless build / smaller headless artifact). For one developer +
agents this is otherwise ceremony, so it is **not** in the baseline; revisit at
P5 when the CLI surface is real.

Crate responsibilities (baseline two-crate layout):

- **`spacetop-core`** (lib): domain model, parser, git-history, worktree scan/
  merge, the `WorkflowIndex`, the query API, metrics, config, filesystem watcher,
  git sync. Pure logic, no terminal deps.
- **`spacetop`** (bin): clap entry point + the TUI (rendering, poll loop, input,
  view state, theming application). With no args / `-w` it launches the TUI;
  headless subcommands call only `spacetop-core`. Sentry init lives here (release
  builds).

### Core spine (new subsystems)

#### Sources (read-only inputs)

These are **new named seams to be created** by porting today's free functions
(`load_workflow_dir`, `scan_worktrees`, `merge_worktree_items`) behind a common
shape — they do not exist as types today.

- **`WorkingTreeSource`** — port of today's parser: scan a workflow dir, split
  frontmatter, parse entities, capture per-entity parse errors. Largely as-is.
- **`WorktreeSource`** — port of today's `.worktrees/*` and `.claude/worktrees/*`
  scan + SHA-1 content merge (`sha1` crate, already a dependency).
- **`GitHistorySource`** *(new — see rigorous derivation below)*.

#### GitHistorySource (the riskiest subsystem — derivation spec)

Goal: per-entity `StageEvent`s with trustworthy timestamps, given that the repo's
real commit patterns are noisy. Derivation rules (each addresses a verified
failure mode):

1. **Entity files only.** Enumerate commits that touch entity file paths
   (`<workflow>/*.md`, `<workflow>/_archive/**`, folder-entity `index.md`). Ignore
   the README and all non-entity files.
2. **Frontmatter-region diff, not pickaxe.** At each touching commit, read the
   blob and parse **only the YAML frontmatter** to read `status:`. A change in the
   frontmatter `status` value between a commit and its parent is a `StageEvent`.
   This avoids false hits from `status:` strings in entity bodies / ACs / fixtures
   and from the README's own `status: design` template line.
3. **Terminal `done` from the archive move.** Entities are archived at a
   non-terminal `status:` (verified: `048`/`049` archived at `status: review`).
   Synthesize the `→ done` transition from the **rename into `_archive/`**
   (detected with `-M`), timestamped at the move commit — not from a frontmatter
   value. This matches the existing `archived_done_count` workaround in
   `src/app/overview.rs`.
4. **`--first-parent` traversal.** Status advances are first-parent main-line FO
   commits; implementation/body edits arrive via merges. Traverse `--first-parent`
   for stage timing and **document that pre-merge worktree-branch timing is
   invisible** — acceptable for "key timestamps."
5. **Rename stitching.** Use `-M` for rename detection. `git log --follow` cannot
   track an entity across *multiple* renames (flat→`index.md` promotion, slug
   rename, then archive move) — `--follow` allows only one pathspec. Stitch the
   timeline by walking commit-level rename records (`--name-status -M`) and
   chaining old→new paths, rather than relying on `--follow`.
6. **Shallow-clone guard.** A shallow clone returns truncated history *without
   erroring*, producing plausible-but-wrong durations. Actively check
   `git rev-parse --is-shallow-repository`; if shallow, mark history
   **unavailable** and refuse to compute dwell/cycle metrics.
7. **Read-only.** Only `log` / `rev-list` / `show` / `rev-parse`. Covered by the
   extended guardrail test.

**Fixtures first:** before implementing, build fixture repos that reproduce
failure modes 2–6 (body-decoy `status:`, archive-move done, merge topology, multi
-rename, shallow clone). The source is correct only when those fixtures pass.

**Fallback (if rule set proves too costly in P2):** ship history as
**"last-changed" only** (`git log -1` per entity file) and **defer dwell/cycle
metrics** to a later milestone. Shipping wrong numbers is worse than shipping
fewer numbers; the "ambition" driver does not justify untrustworthy metrics.

#### Model

- **`Entity`** — evolution of today's `WorkItem`. Carries every current field:
  `path, id, title, status, source, started, completed, verdict, score, worktree,
  issue, pr, body`, plus worktree provenance (`worktree_source`, `main_body`).
  The `WorkItem → Entity` rename happens in **P0** (mechanical) so later phases
  refer to one name.
- **`WorkflowDefinition`**, **`StageDefinition`**, **`StageTransition`** — ported,
  with oklch stage colors and stage prose. `feedback-to` edges are first-class
  (they back the re-scoped "relationships").
- **`StageEvent`** *(new)* — `{ entity_id: String, from: Option<String>, to:
  String, at: CommitTime, commit: CommitId }`, where `CommitTime` is a Unix epoch
  seconds / RFC3339 timestamp and `CommitId` is the full 40-char SHA. The atom of
  history. The `to = "done"` event is the synthesized archive-move event. Shape
  is chosen to leave room for a future per-field replay event without reshaping
  the index.

#### `WorkflowIndex`

Built (fully, per change) from the sources; the single in-memory structure the
query API reads from. Holds:

- entities keyed by id and by slug,
- entities grouped by stage,
- a lightweight **text index** (substring/fuzzy over id + title + body),
- **cross-references** (re-scoped): `issue`/`pr` external links per entity, and
  the workflow's `feedback-to` stage edges. **Not** an entity dependency graph.
- a **per-entity timeline** (the `StageEvent`s for that entity, P2+).

**Rebuild model:** the watcher already debounces filesystem bursts into a single
`RefreshSignal`; on each signal the index is **rebuilt in full**. At the measured
scale this is single-digit milliseconds. Incremental `apply_change` is out of
scope (see Non-goals) — and note it could not patch the timeline anyway, since
the timeline is git-derived, not working-tree-derived.

#### Query API (the headless-reusable surface)

Returns **owned snapshots** (`Vec<Entity>` clones — cheap at this scale) or stable
**ids** the caller re-resolves, never borrowed `&Entity` (borrows cannot outlive a
rebuild). All return types are `serde`-serializable so the same calls back the TUI
and JSON export. Each method is annotated with the phase that makes it return real
data:

- `query(filter) -> Vec<Entity>` — by status, field predicate, or text match.
  **P1.**
- `timeline(entity) -> Vec<StageEvent>` — ordered history for one entity. **P2**
  (returns "unavailable" until history lands).
- `metrics(workflow) -> Metrics` — stage dwell-time, cycle-time, WIP per stage,
  throughput; derived from `StageEvent`s. **P2** (unavailable until history).
- `activity(since) -> Vec<ActivityEvent>` — recent status changes **within the
  loaded workflow** (not cross-workflow). **P3.** `ActivityEvent` carries the
  entity id + the `StageEvent`; no workflow tag is needed since scope is single
  -workflow.
- `related(entity) -> Vec<Relation>` — the entity's `issue`/`pr` links and any
  `feedback-to` stage relationships. **P3.** Not an entity dependency graph.

Before P2 lands, the history-derived methods are present in the API but return the
graceful-degradation "unavailable" state (consistent with Error handling below).
The TUI and CLI call **only** this API — never index internals.

#### Config

Loaded from a config file (location/format decided in the P4 plan — candidates:
XDG config dir, workflow-local override; TOML vs YAML). Carries theme,
keybindings, default sort/scope, per-workflow preferences. Session persistence
(selected entity + scope per workflow) is written back so selection survives
restarts. Config lives in core so headless surfaces honor the same settings.
Config is a **P4** deliverable; until then the rebuilt TUI uses ported hardcoded
defaults.

### TUI layer (rebuilt on top)

- **Decouple `App` from `OverviewState`.** A dedicated **layout pass** computes
  pane geometry once per frame; both render and input read it. This removes the
  Cell-based interior-mutability back-channel — today there are **six** `Cell`
  fields (four on `OverviewState`: `max_preview_scroll`, `max_preview_scroll_x`,
  `preview_viewport_height`, `task_page_size`; two on `PickerState`:
  `viewport_height`, `scroll_offset`) that the render pass writes and input
  handlers read.
- **Off-thread git, synchronous loop.** Keep a synchronous poll loop. Long git
  work (pull, history ingestion) runs on a background thread that returns results
  over an `mpsc` channel drained in the same loop that already drains watcher
  signals. No async runtime. The editor handoff still **suspends** the TUI
  synchronously — that is correct.
- **New views**, each a thin consumer of one query method: command palette +
  search overlay, timeline view, metrics view, activity feed, richer worktree/PR
  view (surfacing `pr`/`issue` status and the worktree diff summary from `Entity`
  provenance fields). Theming reads colors and keybindings from `Config` once P4
  wires it; the rebuilt TUI uses ported defaults until then.

### CLI / headless

The `spacetop` binary keeps current launch behavior (no args → discover and open
TUI; `-w <path>` → open that workflow). v2 adds headless subcommands over the
core query API — indicatively `list`, `metrics`, `export --json` (exact surface
defined in the P5 plan). `metrics` inherits the P2-history dependency: it reports
"unavailable" if run before history is wired or against a shallow clone. Headless
commands depend on `spacetop-core` only and emit serde-serialized output.

### Data flow

```
working tree ─┐
git log ──────┤→ Sources → WorkflowIndex (full rebuild) ──→ Query API ──┬─→ TUI views
worktrees ────┘                  ↑                                       └─→ CLI / JSON export
                  watcher RefreshSignal → rebuild
                  background git thread → mpsc result → loop
```

### Error handling

- **Per-entity parse errors** keep surfacing as synthetic "broken" rows in the
  list — and this is **extended to the archive scope** (closing a current gap
  where `load_archived_items` silently skips malformed archived entities).
- **Git-history failures degrade gracefully.** No git repo, **shallow clone**
  (actively detected), or an unparseable log → history-derived features
  (timeline, metrics, activity) render an "unavailable" state with a hint,
  mirroring today's `SyncAvailability` pattern. Never a crash, never wrong
  numbers presented as real.
- **Read-only guardrail** moves into `spacetop-core` and is extended. Note the
  current `no_write_git_calls` test is a **static source grep** (it asserts no
  `push`/`commit`/`checkout` string literals in `src/`, and that `--ff-only`
  appears exactly once) — *not* a runtime assertion. v2 keeps the static grep and
  **adds a behavioral assertion** that `GitHistorySource` only ever invokes
  `log`/`rev-list`/`show`/`rev-parse` (via the `GitRunner` seam stub).

### Testing

- **Core is pure logic** → comprehensive unit + integration tests with no
  terminal. The trait-seam discipline already present (`GitRunner`, `EditorEnv`,
  `EditorLauncher` — confirmed in `src/git_sync.rs` / `src/editor.rs`) is now
  physically enabled by the crate boundary.
- **`GitHistorySource`** → fixture repositories built with `tempfile` + real
  `git init` and scripted commits that reproduce the verified failure modes
  (body-decoy `status:`, archive-move done, merge topology, multi-rename, shallow
  clone). Extends the existing `git_sync_e2e` pattern.
- **Query API** → golden tests over fixture workflows, including the
  "unavailable" returns before history is wired.
- **TUI** → keep `TestBackend` render tests and `find_text`/list-pane-scoped
  assertions.

---

## Migration phasing (strangler)

Each phase ships independently; the app stays working throughout. P0–P2 build the
spine and are specified in depth; P3–P5 layer capabilities and are sketched here
to be detailed when we reach them.

### P0 — Workspace restructure + model rename *(behavior-identical)*

Stand up the Cargo workspace (baseline two crates: `spacetop-core` + `spacetop`
bin), move existing code into them **without changing behavior**, and perform the
mechanical `WorkItem → Entity` rename so later phases use one name. All current
tests pass unchanged (relocated as needed). Add the "core has no terminal deps"
CI guard. Pure mechanical restructuring — riskiest part is import churn, so it
lands first and isolated.

**Done when:** workspace builds, `make lint` clean, full existing test suite
green, `spacetop-core` links no terminal crate, `Entity` is the single model name.

### P1 — Index + Query API over existing sources *(no new features)*

Introduce `WorkflowIndex` (full-rebuild) and the query API built from the existing
working-tree/worktree sources (**no history yet**). Migrate the list and preview
to read from the query API (`query()` + text match) instead of iterating entities
directly. History-derived methods (`timeline`, `metrics`, `activity`) exist in the
API but return "unavailable".

**Done when:** the TUI renders entirely off the query API; the index rebuilds on
each watcher signal; `query()` has golden tests; history methods return the
documented "unavailable" state.

### P2 — Git-history source *(key timestamps, rigorous derivation)*

Implement `GitHistorySource` per the derivation spec (frontmatter-only diff,
archive-move done synthesis, `--first-parent`, rename stitching, shallow-clone
guard), build the fixtures that reproduce the failure modes **first**, fold
`StageEvent`s into the index as per-entity timelines, and light up `timeline()`
and `metrics()`. Add the behavioral read-only assertion for the new git calls.

**Done when:** `timeline(entity)` returns correct ordered events (incl. the
archive-move `done`) on the failure-mode fixtures; shallow clones report
"unavailable"; `metrics()` produces trustworthy dwell/cycle numbers on fixtures;
guardrail covers the new calls. (If the derivation proves too costly, fall back to
"last-changed only" and defer metrics — documented decision, not silent.)

### P3 — Capabilities *(sketch)*

Build on the spine: search + command palette overlay, metrics view (dwell/cycle/
WIP/throughput), timeline view, activity feed (within the loaded workflow),
richer worktree/PR view, and the re-scoped relationships (`issue`/`pr` +
`feedback-to`). Each is a consumer of one query method. Detailed per-feature plans
written when P2 lands and the query API's real shape is known.

### P4 — Config / theming / persistence *(sketch)*

Config file (theme, keybindings, default sort/scope), session persistence
(selection/scope per workflow), honored by both TUI and headless. Wire the TUI's
theming to `Config` (it uses ported defaults until this phase).

### P5 — CLI headless + export *(sketch)*

Headless subcommands (`list`, `metrics`, `export --json`, …) over the core query
API, with serde-serialized output for scripting. Re-evaluate the optional
`spacetop-tui` third-crate split here, justified only if headless build time
without ratatui is a real concern.

---

## Open questions (to resolve in the relevant phase plan)

- **History fallback trigger (P2):** define the concrete cost signal that would
  flip P2 from full derivation to "last-changed only."
- **Config location & format (P4):** XDG config dir vs workflow-local vs both;
  TOML vs YAML (YAML already in the dependency set).
- **CLI subcommand surface (P5):** exact command names, flags, and JSON schema.
- **Third-crate split (P5):** is a headless build without ratatui worth the
  `spacetop-tui` crate, or is the two-crate boundary enough?

---

## Review history

### 2026-06-11 — Adversarial review (3 parallel reviewers, ground-truthed against repo)

Findings applied to this revision:

| ID | Severity | Finding | Resolution in this doc |
|----|----------|---------|------------------------|
| A | BLOCKER | `done` never written to frontmatter (048/049 archived at `status: review`); `status:` pickaxe matches body/README decoys; `--follow` can't multi-rename; merge topology double-counts; shallow clones silently truncate. | Rewrote GitHistorySource as a rigorous derivation spec (frontmatter-only diff, archive-move done synthesis, `--first-parent`, rename stitching, shallow guard, fixtures-first) + documented "last-changed only" fallback. |
| E | BLOCKER | No `blocks`/`blocked-by`/`references` field in any entity or schema; no body-link conventions. Relationship graph builds on absent data. | Cut the entity dependency graph. Re-scoped "relationships" to extant data: `issue`/`pr` links + `feedback-to` stage edges. Added to Non-goals. |
| B | MAJOR | `Vec<&Entity>` borrows can't survive across frames while the index is rebuilt; conflicts with serde/export. | Query API returns owned snapshots / ids, never borrows. |
| C | MAJOR | Incremental `apply_change` is premature at 48 entities/<1 MB; can't patch git-derived timeline anyway. | Cut for v2; full rebuild per debounced change; revisit only behind a >16 ms benchmark. |
| D | MAJOR | Async overreach; watcher already uses thread+mpsc; no tokio in tree. | Cut async; background git thread + `mpsc` result channel folded into the existing poll loop. |
| M2 | MAJOR | `metrics()`/`timeline()` listed in P1 API but need P2 history. | Annotated every query method with its enabling phase; documented "unavailable" returns pre-P2. |
| M3 | MAJOR | `activity()` "across workflows" contradicts single-workflow load + "No portfolio view". | Scoped `activity()` to the loaded workflow; added "no multi-workflow index" non-goal. |
| M1 | MINOR | "Richer worktree/PR view" locked but never planned. | Gave it a home: Goal 5, a P3 view, built on existing `pr`/`issue`/worktree provenance fields. |
| — | MINOR | Guardrail mischaracterized (it's a static grep, not a runtime assertion). | Corrected the Error-handling description; v2 keeps the grep and adds a behavioral assertion. |
| — | MINOR | Cell back-channel is 6 fields across two structs, not one. | Corrected to enumerate all six. |
| — | MINOR | `WorkItem → Entity` rename unsequenced. | Assigned to P0. |
| — | MINOR | Theming ordering (read-from-Config vs Config ships P4). | Noted TUI uses ported defaults until P4. |
| — | MINOR | `CommitTime`/`CommitId` undefined (undercut the replay-seam question). | Pinned concrete types in the `StageEvent` definition. |
| — | MINOR | 3-crate workspace likely ceremony for one dev. | Baseline reduced to two crates; third (`spacetop-tui`) deferred to a justified need at P5. |

Verified-accurate claims (no change needed): 100 ms poll loop and blocking
git/editor; parser reads only the working tree (no history today); SHA-1 worktree
merge with the `sha1` crate; `GitRunner`/`EditorEnv`/`EditorLauncher` seams exist;
archive parse errors silently dropped; oklch stage colors + stage prose; terminal
crates ratatui/crossterm/termimad/ratskin and no async runtime present; single
binary crate today; `Y` sync runs only `pull --ff-only`.
