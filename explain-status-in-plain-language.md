---
title: Explain status labels in plain language
status: verify
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-22T09:48:59Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-explain-status-in-plain-language
issue:
pr:
mod-block:
id: 087
---

Help new users understand workflow status and where they can act without learning Spacedock terminology first.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: app-state / UI / docs
- Add short explanations for gate readiness, workflow coordinator activity, and storage warnings on their existing surfaces.
- Keep precise technical labels available and derive explanations from typed facts.
- Examples: awaiting captain means waiting for your decision; approved-awaiting-merge means approved but not yet merged; FO means workflow coordinator.
- Non-goals: Renaming canonical states or JSON fields, changing readiness rules, or restoring inferred activity to task rows and preview headers.

## Acceptance criteria

**AC-1 -- Explanations distinguish states without specialist vocabulary.**
Users can distinguish a pending decision, approved work awaiting merge, coordinator activity, and missing progress files. Explanations do not turn unknown information into a healthy or completed state.
Verified by: Representative state scenarios and a terminal walkthrough where a new reader explains each state.

**AC-2 -- Precise facts remain available.**
Users can inspect the original technical status. Existing machine-readable values and readiness semantics remain unchanged.
Verified by: App scenarios and existing headless contract checks for the affected states.

**AC-3 -- Suggested actions respect product limits.**
Explanations identify where an action belongs when that is known. They never imply that Spacetop can approve requests, move tasks, repair checkouts, or authenticate recorded actors.
Verified by: Walkthroughs of approval and storage-warning scenarios, including unknown action destinations.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.

## Implementation plan

### Outcome and boundaries

Deliver short explanations on the existing gate preview, storage footer/help,
and documented activity/help surfaces. Preserve canonical labels, compact list
badges, machine-readable fields and values, readiness calculation, and the
read-only product contract. This is task 087 only; no implementation or Rust
verification is performed in this planning stage.

Source inspection found **no current live workflow-coordinator UI indicator**.
`ui/activity.rs::render_in` shows Git stage-transition history, not live agent
activity; workflow tabs show names only. The backend retains typed
`ActivityHandler::FirstOfficer`, but task rows and preview headers intentionally
suppress inferred runtime state. The first officer confirmed the scope decision:
explain the coordinator term in relevant help/documentation, do not add a live
indicator, and do not portray Git history as live coordinator activity. AC-1's
coordinator scenario therefore tests understanding of the documented term, not a
new runtime-status display.

### Existing surfaces and ownership

Paths below are relative to the repository root.

| Surface | Existing owner / evidence | Planned change |
| --- | --- | --- |
| Readiness facts | `crates/spacetop-core/src/domain/gates.rs:118-183`; `src/gates.rs:15-73` in the same crate | Consume `GateDetails` and `GateReadiness`; leave serialization, labels, parsing and readiness projection unchanged. |
| Compact task badges | `crates/spacetop/src/ui/list.rs:309-323` | Preserve `[gate:captain]`, `[gate:merge]` and all other compact badges; explanations live in the selected task preview. |
| Gate header and scrollable evidence | `crates/spacetop/src/ui/preview.rs:421-423,528-614` | Keep `gate: <canonical-label>` in the header. Prepend meaning and safe next-step prose to the scrollable gate detail body, including when `GateData::Absent` still has readiness. Keep current/historical attempts and raw diagnostics. |
| Storage facts / precise diagnostics | `crates/spacetop/src/app/overview.rs:99-182,458-485` | Add concise explanation selection beside the typed diagnostic; preserve complete diagnostic formatting and branch/path/probe evidence for inspection. |
| Storage footer | `crates/spacetop/src/ui/footer.rs:85-90` | Show a short warning and a `?: details` hint, ahead of ordinary key pills. Left-align the warning-bearing line so clipping cannot hide its beginning. Do not expand a one-line footer into an unbounded message. |
| Existing help popup | `crates/spacetop/src/ui/help.rs:10-119`; `crates/spacetop/src/app.rs:429-438,1142-1150` | Put contextual warning meaning/full diagnostic/safe guidance before the keymap when a warning exists. Add a small status vocabulary section, including FO. Make overflow scrollable in this same popup; no new modal or action command. |
| Coordinator terminology | `crates/spacetop-core/src/domain/mod.rs` (`ActivityHandler`); `README.md:98-106` | Explain the existing typed term without changing the enum or labels. Help is a glossary, not a claim that a coordinator is currently running. |
| Recorded activity feed | `crates/spacetop/src/ui/activity.rs:21-34` | No runtime data or coordinator indicator added; document that this existing feed contains recorded stage changes. |
| Gate-room evidence | `crates/spacetop/src/app/gate_room.rs`, `ui/gate_room.rs`; `docs/gate-room-browser.md:29-31` | Keep browser semantics unchanged. Refer to `B` only as read-only inspection, never approval or authentication. |

### Explanation and action copy contract

Use exhaustive matches on typed states, not substring matching or YAML parsing
in rendering. Static meanings can return `&'static str`; interpolate only known
branch/path/target facts where required. Do not build a generic explanation
framework or introduce dependencies.

| Canonical readiness | Short explanation |
| --- | --- |
| `validating` | Readiness for review has not been established. |
| `needs-preparation` | Checks passed; the review request still needs to be prepared. |
| `awaiting-captain` | Waiting for your decision. |
| `withdrawn-awaiting-prepare` | The review request was withdrawn; a new request is needed. |
| `feedback-pending` | Changes were requested; follow-up is pending. |
| `not-applicable` (`Held`) | A hold was recorded; this request is paused. |
| `approved-awaiting-advance` | Approval is recorded; the move to the next stage is still pending. |
| `approved-awaiting-merge` | Approval is recorded; merging is still pending. |
| `consumed` | This approval was already used; it is not a new pending approval. |
| `superseded` | This approval was superseded; it is no longer pending. |
| `invalid` | Gate information is invalid or unsupported; readiness cannot be trusted. |

`validating` must not say a worker is actively checking anything: the existing
projection also uses it when preparation cannot be proven. `consumed` is not
"task complete"; `superseded` is not evidence of a new approval. A terminal
application target produces the existing merge label; the explanation does not
claim to have checked a remote PR. Preserve preparation diagnostics underneath.

An absent readiness value is not a new healthy state. For archived/currently
ungated/terminal items, retain historical records and use "No current gate
readiness applies" only as contextual explanation; do not invent an approval,
pending decision, or runtime state. Invalid data must remain visible even when
the stage would otherwise have no gate. An absent gate document at a gated
stage can still mean `validating` or `needs-preparation`.

Action guidance:

- Awaiting a decision: "Review the request with your workflow coordinator.
  Decide in your Spacedock workflow session, not in Spacetop."
- Approved states: "Continue through your workflow's advance or merge process;
  Spacetop cannot move or merge this task." A recorded target stage may be
  shown as a recorded target, never as a runnable action.
- Preparation, withdrawal, feedback and hold: describe the outstanding work and
  direct the reader to the workflow coordinator/session. Do not offer invented
  CLI commands or imply that a particular agent is online.
- Invalid/unproven data: "Inspect the recorded details and ask your workflow
  coordinator to check them." Never turn a failed proof into readiness.
- Unknown destination: "The action destination is not known here. Check with
  your workflow coordinator." Do not infer a session, service, URL or approval
  route from a room reference, actor string, stage name, issue or PR field.
- Preserve "recorded" qualifiers for decisions/actors. `B` inspects evidence;
  even verified evidence does not authenticate its recorded actor.

| Storage fact | Plain meaning and safe guidance |
| --- | --- |
| Single-root / attached | No warning; do not add claims that data is fresh or complete. |
| Missing | "Progress files are missing; no task state was loaded." Explain that an empty list is not proof there are no tasks; ask the workflow owner to check the configured state location outside Spacetop. |
| Detached | "Progress files are not on a branch; this view may be out of date." Preserve `detached`; explain state sync is unavailable and repair belongs outside Spacetop. |
| Wrong branch | "Progress files are on a different branch." Keep actual and expected branches in full details; no branch-switch recommendation executed by Spacetop. |
| Unverified | "Spacetop could not verify the progress-file checkout; state sync is blocked." Keep the exact typed problem, paths and errors. Available readable files are not verified freshness or safe sync authority. |

The outside-definition/root-mismatch cases retain their specific technical
diagnostic and readable-snapshot distinction. Do not claim all unverified cases
have readable data. Missing files, unavailable Git probes and empty results are
not interchangeable. Warning guidance must say `Y` cannot repair the state
checkout; do not alter the existing definition/state partial-sync behavior.

Coordinator glossary: "FO (first officer) means workflow coordinator: the agent
that coordinates workflow steps. A recorded `running · FO` value identifies
that handler; it does not mean the task is complete or ready for your decision."
Document that `idle`/missing activity is not evidence of task completion or
coordinator health. The Activity view shows recorded stage changes, not current
coordinator status. Do not add this runtime vocabulary to task-row or preview
header metadata.

### Ordered implementation steps

1. **Pin presentation inputs and negative cases.** Extend existing table-driven
   UI/app fixtures for every readiness and topology variant. Reuse
   `tests/fixtures/durable-gates` and the existing runtime-suppression helpers.
   Add no parser schema or readiness rules. For `None`, cover archived history,
   an ungated/terminal stage, and a gate with absent records separately.
2. **Implement typed explanation selection.** Keep the gate meaning/action
   helper adjacent to `gate_detail_body` in `ui/preview.rs`; keep topology
   meaning/guidance alongside `StateTopologyDiagnostic` in `app/overview.rs`.
   Renderers consume existing typed facts and return text, never read files,
   query Git, authenticate actors or derive new readiness.
3. **Integrate gate explanations.** Add short prose at the start of the
   scrollable details, not more fixed header rows. Preserve raw readiness,
   compact badges, record IDs, warnings and current/historical distinctions.
   Cover the worktree-diff path as well as normal markdown, wrap on/off and
   archived previews; gate details must not disappear when the body is a diff.
4. **Integrate storage and help explanations.** Footer gets a bounded summary;
   the existing `?` popup gets the full typed diagnostic and guidance plus
   coordinator vocabulary. In `app.rs`, add popup-local scroll state/reset and
   consume Up/Down, PgUp/PgDn, Home/End only while help is open. In `ui/help.rs`,
   wrap content, clamp scrolling to rendered height, and retain a visible
   `?/Esc: close` hint. Route all existing help-open paths through the same
   reset behavior. No underlying selection movement, sync or task mutation
   while help consumes those keys. This is overflow support for the expanded
   existing surface, not a new status dashboard.
5. **Update nearby user guidance.** Update README Status/readiness/activity
   sections and keyboard help for help scrolling, plain meanings, unknown
   destinations, missing progress files and product limits. Update
   `docs/gate-room-browser.md` only with the relationship between explanations,
   recorded facts and read-only evidence inspection. No policy/architecture
   rewrite or unrelated workflow documents.
6. **Verify once after implementation is complete.** Run the commands and
   terminal walkthrough below; record observed outcomes against AC-1/2/3 in the
   implementation/verification report. Do not claim planned checks passed.

### Acceptance evidence and lowest test layers

| Claim | Evidence to add or preserve | What would falsify it |
| --- | --- | --- |
| AC-1: distinct gate meanings | Table-driven explanation tests and `ui/tests/preview.rs` TestBackend scenarios for all 11 states, absent records, historical/archived and invalid records. | Swapping pending decision with pending merge, showing active validation from unproven data, or labelling spent approval complete. |
| AC-1: coordinator understood | Help rendering assertion pins "FO" and "workflow coordinator"; new reader paraphrases the README/help glossary and distinguishes it from Git activity history. | Calling the Activity feed live coordinator state or showing an unsupported current handler. |
| AC-1: missing vs empty/unknown | `app/overview.rs` diagnostic tests and `ui/tests/task_list.rs` storage cases cover missing, detached, wrong branch, each unverified problem, attached and single-root. | Missing progress files look like a healthy empty workflow, or unavailable verification becomes "up to date". |
| AC-2: exact facts retained | `ui/tests/preview.rs` keeps raw gate labels and records; help tests retain full diagnostic branch/path/error values after scrolling; existing `crates/spacetop/tests/durable_gates.rs` list/export/filter contracts remain intact. | Copy changes JSON/filter values, suppresses diagnostics, or drops full technical facts behind clipping. |
| AC-2: semantics unchanged | Preserve core tests `readiness_uses_only_last_attempt_in_current_gated_nonterminal_stage`, `query_keeps_runtime_and_archive_authority_separate`, and preparation/source tests in `crates/spacetop-core/tests/durable_gates.rs`. | Explanation work changes readiness, latest-attempt authority, archive behavior or preparation proof. |
| AC-3: safe next steps | Preview/help assertions cover awaiting decision, merge, invalid gates, missing/external/unverified state and unknown destinations. Retain gate-room attribution disclaimers. | A prompt offers approval/advance/repair in Spacetop, infers an action destination, or treats a recorded actor as authenticated. |
| Layout and interaction | `ui/tests/chrome.rs`, `ui/tests/task_list.rs`, `ui/tests/preview.rs` plus `app/tests.rs`: 120x40, 80x24, 40x12 and 20x6, long Unicode paths, wrapping, popup scroll/reset/exit, active/archive and worktree diff. | Critical warning disappears, full diagnostic is unreachable, popup scroll moves the underlying task, or the close hint is lost. |
| Runtime suppression | Preserve `task_rows_ignore_runtime_across_scopes_and_widths`, `task_row_stays_unchanged_while_scanner_replays_and_stops`, `preview_ignores_runtime_preserving_metadata_and_body`. | Runtime transitions change task-row or header metadata. |
| Read-only/core boundaries | Existing `no_write_git_calls`, `no_terminal_deps`, state topology and sync tests; add behavior assertions for help/preview keys remaining inspection-only. | A new workflow writer, repair command, render-time I/O, core terminal dependency or sync-eligibility change appears. |

Required full implementation checks, from repository root:

```sh
cargo fmt
cargo test
make lint
```

Focused commands for a failure investigation (not a replacement for the full gate):

```sh
cargo test -p spacetop --test durable_gates
cargo test -p spacetop-core --test durable_gates
cargo test -p spacetop-core --test state_checkout_fixtures
cargo test -p spacetop-core --test no_write_git_calls
cargo test -p spacetop-core --test no_terminal_deps
```

No watcher/backend changes are planned, so ignored notify tests are not required.
No CLI option/schema change is planned; headless checks are regression evidence.

### Real terminal walkthrough and fixtures

Use the actual TUI, not only TestBackend screenshots:

```sh
cargo run -p spacetop -- --workflow-dir tests/fixtures/durable-gates
```

Select `pending`, `merge`, `advance`, `held`, `withdrawn`, `invalid`, `consumed`,
`superseded` and historical rows. Open the preview, scroll gate explanations,
toggle wrap, return to the list, open `?` and inspect the glossary. Repeat at
80x24 and 40x12; exercise the smallest popup close/scroll path at 20x6. Record
terminal size, input keys, visible wording and where the precise label appears.

Before the storage walkthrough, implementation must provide reproducible fixture
setup in the task evidence: exact temp-directory Git/file commands based on the
existing `state_checkout_fixtures.rs` recipes, with local test Git identity and
no network dependency. Use a temporary definition plus contained split-root
checkout for attached/detached/wrong-branch, a declared absent state directory
for missing, and a contained non-Git directory for an unverified probe. Include
an ordinary readable task in the applicable cases. Do not alter the live
workflow checkout to induce warnings or depend on a contributor's HOME sessions.
Run the binary against each generated workflow using its explicit path; inspect
footer and full help details. Fixture creation is test setup, not product writes.

Ask a new reader, without teaching the answer first: "What is this waiting
for?", "Has the work merged?", "What does FO mean?", "Does this activity feed
tell you the coordinator is running?", "Are there no tasks, or are progress
files missing?", and "Where can you act?" Include a case with no known action
destination and an invalid/unverified case. Record the reader's actual
paraphrases and corrections alongside AC-1/3; if no reader is available, record
that human proof as outstanding, not as a successful agent simulation.

For AC-2, compare observed labels with `list --json`, readiness filters and
`export --json` on the same gate fixture, using the existing CLI contract tests
as the structural regression proof. For AC-3, record that preview/help/room
inspection does not change fixture workflow bytes or Git HEAD; capture fixture
before/after evidence. Do not press `Y` against the live project or imply its
existing fast-forward sync is checkout repair.

### Risks and scope exclusions

- Expanded text can hide task content or warnings. Keep gate prose scrollable,
  warnings short, full facts reachable through help, and test narrow terminals.
- `validating`, `not-applicable`, consumed and missing state have especially
  misleading everyday meanings; the explicit copy matrix prevents healthy,
  active-work or completed-work guesses.
- No action-destination model exists here. Generic coordinator guidance plus an
  explicit unknown destination is safer than parsing links or runtime sessions.
- Readiness and recorded actors are inspection facts, not authority to act.
  This work must not add workflow writes, new probes, new runtime detection,
  row/header runtime metadata, automatic checkout repair or freshness inference.
- The new-reader walkthrough is human evidence owed by implementation/verify;
  source mapping and this plan do not satisfy that future behavioral proof.
- No dependency, schema, CLI, stage-transition, gate-room verifier, watcher or
  storage-classifier change is needed. Other usability/runtime tasks remain
  independent; do not absorb their scope into task 087.

## Stage Report: plan

- DONE: Map the existing gate-readiness, coordinator-activity, and storage-warning surfaces and specify plain-language explanations while preserving precise labels and machine-readable values.
  The surface ownership table and exhaustive copy contract cite current source; the first officer confirmed glossary-only coordinator coverage because no live coordinator indicator exists.
- DONE: Produce an implementation plan with owned modules and acceptance-criterion evidence, including unknown states, safe action guidance, and a real terminal walkthrough.
  Ordered steps, AC evidence/falsification matrix, exact implementation check commands and actual-terminal/new-reader proof procedure are recorded above.
- DONE: Record scope boundaries and risks; do not change readiness semantics, restore inferred activity to task rows, or add workflow writes.
  Explicit exclusions preserve projection/JSON/suppression/sync contracts; this stage changes only this assigned entity body.
- SKIPPED: Implementation builds, tests, lint, formatting and live terminal proof.
  Planning-only assignment; these checks and human evidence are specified for the implementation/verification stages, not claimed as exercised.

### Summary

Mapped existing status surfaces and produced an implementation path for typed,
plain-language explanations with precise facts still inspectable. The plan
preserves runtime suppression and read-only behavior, identifies the missing
live coordinator surface without inventing one, and defines falsifiable
automated and real-terminal/new-reader acceptance evidence.

## Implementation walkthrough evidence

The actual TUI was driven through a PTY at 80x24 and 40x12, with a 20x6 help
check. Inputs included Home plus j/k to select the fixture rows, Enter, PgDn,
w twice, q to close preview, ? / End / Home / Esc for help, and B for recorded
evidence. The narrow walkthrough exposed a zero-height gate body; implementation
now reserves scrollable body space before fixed metadata. At 40x12, PgDn exposes
`Waiting for your` / `decision.` while the canonical label remains in the body.

Storage fixtures were created under
`/var/folders/h_/tb6bynfj41g1dcvpb2472vrh0000gn/T/spacetop-087-ebpsqyzl`;
no live workflow checkout was changed. The following commands recreate the
same fixture shapes from the worktree root, using only local Git/file operations:

```sh
ROOT=$(mktemp -d)
for CASE in attached detached wrong missing unverified; do
  WF="$ROOT/$CASE"
  mkdir -p "$WF"
  printf '%s\n' '---' 'commissioned-by: spacedock@0.27.3' \
    'state: .spacedock-state' 'state-branch: spacedock-state/demo' \
    'stages:' '  states:' '    - name: implement' '---' \
    '# Storage walkthrough' > "$WF/README.md"
  [ "$CASE" = missing ] && continue
  STATE="$WF/.spacedock-state"
  mkdir -p "$STATE"
  printf '%s\n' '---' 'id: active' 'title: Ordinary readable task' \
    'status: implement' '---' 'Readable task body.' > "$STATE/active.md"
  [ "$CASE" = unverified ] && continue
  git -C "$STATE" init --initial-branch spacedock-state/demo
  git -C "$STATE" add active.md
  git -C "$STATE" -c user.name='Spacetop Test' \
    -c user.email=spacetop@example.invalid commit -m fixture
  [ "$CASE" != detached ] || git -C "$STATE" checkout --detach
  [ "$CASE" != wrong ] || git -C "$STATE" switch -c wrong-state
done
# Run separately for each CASE after building; set terminal dimensions as above.
CASE=missing
HOME="$ROOT/home" XDG_CONFIG_HOME="$ROOT/config" \
  XDG_STATE_HOME="$ROOT/session-$CASE" \
  target/debug/spacetop --workflow-dir "$ROOT/$CASE"
# Gate cases use the same isolated environment and this workflow path:
# target/debug/spacetop --workflow-dir tests/fixtures/durable-gates
```

Observed: pending says `Waiting for your decision.`; merge says approval is
recorded and merging is pending; invalid says readiness cannot be trusted.
Consumed, superseded, hold, withdrawn, advance and historical rows retained their
distinct meanings and recorded details. Help's glossary distinguishes FO from
the Git stage-change Activity view and states the unknown action destination.
Missing state shows zero loaded tasks plus the missing-files warning; attached
state has no topology warning; detached, wrong-branch and failed Git-probe cases
retain readable tasks with warnings and full technical details in help.
At 20x6, the fixed `?/Esc: close` hint stayed visible through PgDn/End/Home,
and reopening returned to the warning's top. No new-reader paraphrases were
obtained; these observations are agent-operated terminal evidence only.

Read-only proof compared all fixture markdown SHA-256 values and Git HEADs
before/after preview, help and room inspection: unchanged. The code checkout
HEAD was `f1c087d5c0b61dd830548d5fc5f9d1dea16a0a61`; all three temporary state
repositories remained at `f7763c3048e8d064ac286800f39496b67953ffe4`.
`list --json` and `export --json` agreed on all 12 readiness values;
`list --gate-readiness approved-awaiting-merge --json` returned only `merge`.
Initial REA PTY captures were followed by direct Python PTY/pyte captures after
the capture provider reported an ownership-cleanup error. Final direct runs
exited 0. Raw session evidence is available to this handoff at
`local://task087-terminal-evidence.json`; pyte was temporary external tooling,
not a project dependency.

## Stage Report: implement

- DONE: Implement the recorded plain-language gate, storage, and contextual help explanations while preserving canonical labels, machine-readable values, unknown states, runtime suppression, and read-only boundaries.
  Code commit `67755d2` on `spacedock-ensign/explain-status-in-plain-language`; typed explanations, scrollable contextual help, warning-first footer, and narrow-preview body reservation are implemented without changing core projection or schemas.
- DONE: Prove the changed paths with focused behavioral checks and an actual terminal walkthrough; run final required formatting, tests, and make lint once edits are complete, and report any unavailable new-reader evidence honestly.
  Final `cargo fmt && cargo test && make lint` succeeded: 705 tests passed across 21 suites, 4 ignored; Clippy ran all targets/features with warnings denied. Actual PTY observations and reproducible setup are above.
- DONE: Update affected user documentation and record acceptance-criterion evidence and committed implementation changes in the stage report.
  README and `docs/gate-room-browser.md` explain gate meanings, FO, history versus live activity, help scrolling, storage warnings, unknown destinations and read-only limits.
- DONE: AC-1 automated and actual-terminal state distinction evidence.
  `all_readiness_meanings_preserve_labels_even_without_records`, `no_readiness_does_not_invent_health_or_hide_invalid_data`, rendered gate/diff/preparation scenarios and the glossary test fail if pending, spent, held or unknown states become ready/completed; 80x24/40x12 walkthroughs observed their distinct meanings.
- DONE: AC-2 precise facts and machine contracts.
  `contextual_help_preserves_every_topology_problem_and_unicode_evidence` exercises every typed unverified problem at 120x40, 80x24, 40x12 and 20x6 and fails if exact diagnostics become unreachable; existing durable-gate JSON/filter and core authority tests passed, plus actual list/export/filter comparison above.
- DONE: AC-3 safe guidance and inspection boundaries.
  Gate/help assertions reject in-app approval/advance implications; actual wrong-branch/missing/probe-failure help directs repair outside Spacetop, room inspection displays unauthenticated recorded attribution, and fixture bytes/HEADs stayed unchanged.
- DONE: Layout, input isolation and runtime suppression regression evidence.
  `help_scrolls_without_acting_and_reopens_at_top` pins scroll bounds/reset, unchanged selection/index/scope and no sync request; `short_gate_preview_keeps_explanation_reachable_by_scrolling` catches the 40x12 zero-body failure. Existing task-row/header runtime-suppression and read-only/core guardrails passed.
- SKIPPED: New-reader comprehension and paraphrase evidence for AC-1/AC-3.
  No human reader was available; the first officer explicitly directed recording this limitation for captain review. Agent observations and string assertions are not substituted for that human proof.

### Summary

Implemented and committed the complete presentation change, including exact
wrapped-help scroll bounds via the existing Ratatui rendered-line-info feature
(no new project dependency). Final required checks and real-terminal scenarios
passed after correcting the narrow preview and test-harness issues; human
new-reader comprehension remains outstanding, so AC-1 is not fully proven.

## Stage Report: verify

- DONE: Independently review implementation against all acceptance criteria and read-only, canonical-label, unknown-state, runtime-suppression, and safe-action boundaries; identify only material defects with concrete evidence.
  Reviewed `f1c087d...67755d2` across all 11 changed files against the task and repository policies; no material code defects found. No implementation fixes or gate approvals made.
- DONE: Assess recorded 705-test, lint, and real-terminal evidence without redundantly rerunning owned green checks; exercise any changed-path claim whose evidence is absent or insufficient.
  Accepted the implement report's final `cargo fmt && cargo test && make lint` (705 passed, 4 ignored), corroborated final-run ordering in the implementation transcript, and inspected raw PTY screen evidence; no uncovered machine-checkable changed-path claim required another run.
- DONE: Record a verdict and per-criterion coverage, explicitly retaining missing human new-reader comprehension evidence as an unmet acceptance requirement rather than inventing or waiving it.
  Proposed verdict for FO: verification evidence incomplete / hold completion for human proof; code review is comment-only with no material defects, not gate approval.
- DONE: AC-1 technical state-distinction coverage.
  Exhaustive typed meanings in `ui/preview.rs:530-610`, storage explanations in `app/overview.rs:115-133`, and glossary in `ui/help.rs:109-120` preserve unknown/spent-state caveats; gate/diff/wrap and absent-record tests would fail on swapped meanings or invented readiness.
- FAILED: AC-1 required new-reader comprehension proof.
  Entity lines 35-37 and 260-266 require actual new-reader explanations; lines 365-367 and 397-398 explicitly record none. Classification proposal: missing human acceptance evidence, not a demonstrated code defect; an agent cannot supply or waive it.
- DONE: AC-2 precise facts and unchanged machine semantics.
  Core/parser/query/CLI code is unchanged; canonical readiness remains alongside prose, full topology diagnostics remain scroll-reachable, and recorded list/export/filter evidence retains all 12 fixture results (11 distinct non-null states are not claimed).
- DONE: AC-2 layout and diagnostic regression coverage.
  `contextual_help_preserves_every_topology_problem_and_unicode_evidence` checks exact rendered diagnostics for nine unverified variants at four sizes; it fails if wrapping/scrolling loses path/error facts. `short_gate_preview_keeps_explanation_reachable_by_scrolling` catches lost pending-decision text at 40x12.
- DONE: AC-3 technical action-boundary coverage.
  Gate/help text explicitly disclaims in-app approval/move/merge/repair and actor authentication, and says the destination is unknown; captured missing/wrong/unverified screens direct users outside Spacetop, not to invented commands.
- FAILED: AC-3 human understanding of where to act remains unproven.
  The specified new-reader prompts include unknown destinations and invalid/unverified cases; no actual paraphrases or corrections are present. Classification proposal: same human-proof gap as AC-1, not a second code defect.
- DONE: Read-only and runtime-suppression boundaries.
  The diff adds no I/O, workflow writer, runtime projection, or sync eligibility change; recorded fixture bytes/HEADs are unchanged, while `help_scrolls_without_acting_and_reopens_at_top` would fail on selection/scope/index mutation, quit, or a pending sync request.
- DONE: Assess actual-terminal evidence and documentation.
  `local://task087-terminal-evidence.json` contains 11 direct PTY walks with exit 0, gate screens at 80x24/40x12, all five storage shapes at 40x12, and missing-state help at 20x6; README and gate-room guidance match the delivered limits.
- SKIPPED: Repeat builds, formatting, tests, lint, and terminal walkthroughs.
  Existing green ownership evidence and inspected screen captures cover the changed machine behavior; repeating them would not resolve the only outstanding requirement, human comprehension.

### Summary

Independent standards/spec review found no material implementation defect in `67755d2`; existing automated checks and actual-terminal captures support the technical AC-1/2/3 claims. Overall acceptance remains incomplete because the explicitly required new-reader comprehension and action-location proof is absent: FO should retain that human-evidence blocker and arrange the prescribed walkthrough, without treating this report as approval or a code-fix request.
