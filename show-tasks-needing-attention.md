---
title: Show tasks that need attention
status: implement
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-22T13:28:21Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-show-tasks-needing-attention
issue:
pr:
mod-block:
id: 086
---

Help a person unfamiliar with the project find work that needs attention and understand why it appears.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: domain / query / app-state / UI / docs
- Include review requests, recorded blockers, and missing information supported by existing workflow facts.
- Reuse the plain-language explanations from explain-status-in-plain-language.
- Non-goals: Automatic prioritization, delay or productivity guesses, agent dispatch, task transitions, or approvals.

## Acceptance criteria

**AC-1 -- A focused Needs attention view explains every inclusion.**
Each listed task shows the recorded reason it needs attention. Missing or invalid information is distinguished from a recorded blocker. Tasks without a qualifying fact do not appear.
Verified by: Query and app scenarios containing each supported reason and a task with no qualifying reason.

**AC-2 -- Users can reach the relevant evidence from a selected task.**
A person unfamiliar with the project can find a task awaiting review, explain why it appears, and open its report or supporting evidence without help.
Verified by: A terminal walkthrough using a representative workflow, including a missing-evidence case.

**AC-3 -- Runtime silence never means blocked work.**
No detected activity, old timestamps, and process absence alone cannot place a task in the view. Recorded workflow facts remain separate from detected AI activity.
Verified by: Negative query cases with idle or unavailable activity and no recorded attention reason.

**AC-4 -- Inspection remains read-only.**
The view does not approve requests, change stages, run agents, or modify workflow files.
Verified by: Before-and-after workflow file comparison during the walkthrough and existing write-safety guardrails.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.

## Implementation plan

### Baseline and boundaries

- Plan against `main` at `0f7f38e` (merged task 087). Its gate explanations are in `crates/spacetop/src/ui/preview.rs::gate_explanation`; topology explanations are in `app/overview.rs::StateTopologyDiagnostic`. Reuse that wording and preserve canonical readiness labels.
- This stage changes only task 086's body in the shared state checkout. No product edits, task transitions, approvals, builds, tests, formatting, or terminal acceptance runs are part of this planning delivery.
- The product remains a read-only inspector. Attention is a projection of recorded workflow facts and explicit verification diagnostics, not urgency, priority, elapsed time, or worker health.
- Keep the existing two-crate boundary, query spine, safe evidence reader, and task preview. No dependencies, new workflow schema, runtime detection changes, background evidence scan, CLI flags, or session/config schema changes are needed.

### Qualifying facts and exact inclusion rules

Add a small terminal-free `AttentionReason` enum and pure projection from an entity's existing `GateData`, `GateReadiness`, and `GatePreparation`. Return an optional reason without allocating or copying the entity, report body, or diagnostics. Keep facts and reasons distinct: the reason identifies the category; the existing gate details retain the full underlying evidence.

| Existing typed fact | Attention reason and user-facing meaning | Evidence destination |
| --- | --- | --- |
| Current `AwaitingCaptain` | Review requested: "Waiting for your decision." | Current gate/attempt, recorded briefing reference, and `B` browser. |
| Current `Held` with the selected hold decision | Recorded blocker: "A hold was recorded; this request is paused." | Selected resolution, recorded reason, actor and timestamp; a blank reason remains "No reason recorded", not an invented explanation. |
| Current `FeedbackPending` | Recorded follow-up: "Changes were requested; follow-up is pending." | Selected revise decision and its recorded reason. Do not call a requested revision a hold. |
| Current `NeedsPreparation` | Request preparation needed: "Checks passed; the review request still needs to be prepared." | Preparation proof and task report. This is not a submitted review request. |
| Current `WithdrawnAwaitingPrepare` | Request replacement needed: "The review request was withdrawn; a new request is needed." | Selected withdrawal and recorded reason. Do not present the withdrawn request as awaiting a decision. |
| Current `ApprovedAwaitingAdvance` | Recorded next step: "Approval is recorded; the move to the next stage is still pending." | Selected approval and application target; not a new review request. |
| Current `ApprovedAwaitingMerge` | Recorded next step: "Approval is recorded; merging is still pending." | Selected approval and application target; not proof of remote PR state or task completion. |
| `GateData::Invalid`, or `GateReadiness::Invalid` | Invalid information: "Gate information is invalid or unsupported; readiness cannot be trusted." | Existing parser/readiness diagnostics and raw recorded details, including on an otherwise ungated stage. |
| Current `Validating` with nonempty preparation diagnostics | Missing or unverified information: "Readiness for review has not been established." | Exact preparation diagnostic and report. Never label this a recorded blocker. |
| An active `EntityParseError` | Invalid task information; show the existing broken-file row with its filename and diagnostic, not a fabricated task ID/title/status. | Existing broken-row preview. |

Projection precedence is invalid gate data/readiness first, then the current readiness mapping. Preparation diagnostics qualify only the validating case, not an unrelated past attempt. A single current reason is sufficient for these mutually exclusive readiness states; all underlying diagnostic lines remain visible in the selected preview.

Important limits:

- `GatePreparation.diagnostics` currently carries strings, not typed failure subcategories (`gate_proof.rs:46-59`). Use presence only and display the exact diagnostic; never substring-match to claim specifically "report missing" when Git verification, topology, tracking, or cleanliness could have failed. The category explicitly says missing **or unverified** information.
- `Validating` alone, default/empty preparation, absent gate data on an ungated/terminal task, `Consumed`, `Superseded`, and no applicable current readiness do not qualify. A historical hold, review, or revision does not qualify the current task. An independent invalid-data diagnostic still qualifies.
- Optional issue/PR/worktree/source/score fields being empty do not qualify. No rule asserts that every task owes those fields.
- A generic `blocked` field, `mod-block`, or prose containing "blocked"/"FAILED" is not a supported blocker source in the current typed `Entity`. Do not introduce a second schema or scan prose to fabricate one; the existing selected durable hold is the supported recorded blocker.
- No session status, `HumanGate` runtime event, last activity timestamp, process probe, age, unavailable activity scan, or filesystem silence enters the reason function. No runtime badge is restored to list rows or preview headers.
- Workflow-level missing/detached/wrong-branch/unverified storage remains the existing contextual warning/help, not a fake task reason attached to every row. An empty attention view with missing state is not evidence that no work needs attention.
- Gate-room verification is lazy today. An unresolved room cannot remove a known review request, and opening a browser must not silently change list membership. Missing room/artifact evidence is shown by the browser's typed diagnostics; this task does not scan every artifact or create a new background evidence index.

### Focused view and navigation contract

Use the existing overview list as a focused mode, not a new duplicate list or modal:

1. Add `needs-attention` and `all-tasks` to the existing command palette (`:` by default; respect configured command binding). The former activates the active-task focus; the latter restores the normal active list. Add discoverable entries to help and a mode label in the list/footer. Avoid claiming the unreserved `N` key, which users can already assign through keybinding config.
2. Store the focus per `OverviewState`, separate from `ViewScope`. Entering it selects active scope; `a` exits the focus and opens archives. Returning from archives is the ordinary active list. Do not serialize this transient focus into `WorkflowSession`, change defaults, or disturb another workflow tab.
3. Have `visible_items()` pass `FieldFilter::NeedsAttention` through `WorkflowIndex::query`. Expose the typed reason through the index/core projection for renderers; no UI match on raw YAML, status-name strings, or runtime state. Existing active parse-error rows remain selectable because invalid task information qualifies.
4. Keep ID/status sorting, with no priority ordering. Preserve selection by stable path/slug when entering/leaving focus or refreshing; if the selected task stops qualifying, clamp to the next valid row or empty state. Audit sort, search, row count, selection, mouse hit testing, preview, and reload together: all must use the same focused row set. Search results in this mode must not select excluded tasks behind the filter.
5. Each attention row displays a plain-language reason, not just a colored badge. Use a readable continuation line or equivalent bounded layout; keep title, ID and canonical stage. The selected preview begins with the full reason and the existing scrollable gate details. Preserve wrapping, worktree-diff previews, and precise recorded identifiers.
6. Retain `Enter` for the task/report preview and `B` for gate evidence. Show the explicit hint "Enter: report/details; B: review evidence" where applicable. The report is the existing task body, not a guessed external report path. Broken rows use `Enter` for their filename/parse diagnostic and must not open another task's browser.
7. `B` keeps current selected-attempt semantics. In the browser, `Tab` selects inventory, Up/Down or j/k selects an item, `Enter` opens verified bytes, and Esc returns through item/browser to the same attention selection. Historical fallback remains labeled historical and cannot be advertised as current evidence.
8. No current attempt, unreadable/missing room, invalid binding, unsupported reference, missing Git object, non-text artifact, or disappearing entity gets a success fallback. Keep the existing diagnostic and allow returning to report/details. Never launch arbitrary reference URLs, open an editor as the acceptance path, fetch objects, repair state, or interpret an actor/room reference as an action destination.
9. Empty-state copy is "No tasks with recorded attention reasons in the loaded state", with an `all-tasks` escape. Retain parse/load/topology warnings and task 087's warning that missing progress files are not a healthy empty workflow.

### Owned modules and ordered implementation

| Step | Owner/files | Change and lowest practical proof |
| --- | --- | --- |
| 1. Pin the classifier contract | New small core `domain/attention.rs`, exported from `domain/mod.rs`; core unit tests | Implement the exhaustive allocation-free reason mapping above from existing typed inputs. Do not change readiness calculation, canonical labels, gate parsing, or entity serialization. Table-test every inclusion/exclusion and invalid-data precedence. |
| 2. Connect the query spine | `crates/spacetop-core/src/query.rs`, `index.rs` | Add `FieldFilter::NeedsAttention` and an index reason accessor using the same projection. Query tests cover each reason, combinations with status/text/sort/scope, and no qualifying fact. Existing parse-error accessors remain the typed source for broken rows. No CLI filter flag is requested. |
| 3. Add focused app state | `crates/spacetop/src/app/overview.rs`, `app/search.rs`, `app/keys.rs`, `app.rs`, `app/tests.rs` | Wire the two palette actions and transient per-workflow focus; align all selection/search/reload/sort paths with the filtered rows. Exercise active/archive transitions, independent tabs, broken rows, empty view, and removal of the selected reason on reload. Preserve configurable bindings and existing popup input isolation. |
| 4. Reuse explanation presentation | `crates/spacetop/src/ui/preview.rs`, `ui/list.rs`, `ui/footer.rs`, `ui/help.rs`, and owning UI tests | Move the existing gate explanation match to a small shared UI helper if needed, migrate every existing caller, and reuse it for attention rows/preview. Do not maintain a second gate vocabulary or move terminal copy into parser logic. Add full reason labels, mode/escape/help hints, and narrow-screen rendering. |
| 5. Connect existing evidence paths | `crates/spacetop/src/app/gate_room.rs` and its tests; existing core `gate_room.rs`/tests unchanged unless a directly affected callsite requires adjustment | Reuse `WorkflowIndex::gate_room`/`gate_room_item`, existing verified-byte loading, and refresh invalidation. Test opening from focus and returning to the same task, unavailable evidence, and reload while the browser is open. No rendering I/O or expanded safe roots. |
| 6. Add reproducible fixtures and user guidance | `tests/fixtures/` alongside durable-gates patterns; `README.md`, `docs/gate-room-browser.md` | Provide a representative workflow with each reason, a nonqualifying control, malformed task, valid bound report/artifact, and missing-evidence variants. Document activation/escape, reason limits, navigation, missing versus blocked, and external workflow-session action boundary. |
| 7. Final verification and evidence report | Implementation worker after all edits land; independent verify stage reviews evidence | Run the commands and walkthrough below once the implementation is complete, record actual outcomes, and retain the separate new-reader proof obligation. |

Fixture prerequisite, not a product spike: extend/reuse `tests/fixtures/durable-gates` and the temporary verified-room builders in `crates/spacetop-core/tests/gate_rooms.rs`. Existing durable-gates rows are sufficient for readiness cases but are not proof that their external room references resolve. Supply an actually valid Briefing v1 binding with correct digest and local report/artifact bytes for the successful navigation path. Use temporary local Git repositories for preparation-proof clean/missing-report cases so proof does not depend on a developer's checkout dirtiness. Do not embed machine-specific home paths.

### Acceptance evidence and falsification matrix

| Acceptance criterion | Required evidence | Regression that must fail |
| --- | --- | --- |
| AC-1: every inclusion has a recorded reason | Pure core mapping and query cases for every table row, app reason/row alignment, TestBackend rendering of each category | Including an ordinary task, confusing missing/unverified information with a hold, labeling approval as a review request, or rendering a reason for the wrong selected row. |
| AC-1: missing/invalid facts are honest | Invalid gates on gated and ungated stages, malformed task rows, nonempty versus empty preparation diagnostics, absent optional fields | Turning default/absent metadata into a blocker; hiding an invalid row; inferring a specific missing file from arbitrary diagnostic text. |
| AC-2: navigation and understandable evidence | App/browser tests plus actual terminal path from palette to review task to report and verified inventory bytes; missing-room and missing-artifact paths; separate unfamiliar-person exercise | Opening another row's evidence, presenting historical evidence as current, showing stale verified bytes after deletion, or offering an unavailable destination as success. |
| AC-3: no runtime inference | Cross-product of unchanged workflow facts with idle, running, runtime human-gate, old/recent timestamps and unavailable scan results; no-reason tasks stay excluded and held/review tasks stay included | Any activity-only change alters membership or its reason. Process absence cannot be an input to the classifier. |
| AC-4: read-only inspection | App assertions for no sync/dispatch/transition request from attention/palette/browser/help inputs; existing write-safety/core-boundary guardrails; actual before/after workflow content inventory | Changing workflow bytes, creating a workflow file, issuing an agent or approval action, broadening Git commands or evidence roots, or writing config/session state into the workflow. |
| Focus integration | App tests for sort/search/mouse/keyboard, reload removing/reordering reasons, broken-only list, tabs, archive exit and palette cancellation | Selection index refers to unfiltered rows, hidden search results open, focus leaks between workflows, or empty view traps the user. |
| Presentation | TestBackend at 80x24 and 40x12, minimal 20x6 escape/scroll case; wrap on/off and worktree-diff body | No reason is legible/reachable, precise recorded facts disappear, or help/browser keys leak into underlying actions. |

Implementation-stage commands, executed from the product worktree after edits:

```sh
cargo fmt
cargo test
make lint
```

The full suite must include `crates/spacetop-core/tests/no_write_git_calls.rs`, `crates/spacetop-core/tests/no_terminal_deps.rs`, existing durable-gate/room regressions, app tests, and affected UI tests. Record command outcomes and falsifiable assertions, not a pass count alone. Run `cargo test -- --ignored` only if implementation changes watcher behavior; this plan does not require such a change. A planning-only state report does not warrant Rust checks.

### Actual-terminal and new-reader proof protocol

The following is required implementation evidence, not an assertion that it has happened:

1. Build/use the actual updated binary (`cargo build -p spacetop`, then `target/debug/spacetop --workflow-dir <representative-workflow>`). Copy the committed fixture to a temporary location if preparing a Git proof case. Put HOME/XDG config and session roots in separate temporary absolute directories outside the workflow. Record the code commit, fixture setup, platform, terminal sizes and input sequence.
2. Before launch, record the full relative-path inventory and byte hashes for every file in the representative definition and state roots, including rooms, artifacts, untracked files and archive files. After exiting, compare both inventories and hashes; this must detect additions/deletions as well as changed bytes. Exclude `.git` internals explicitly, not workflow files; separately confirm no Git refs/index or write commands were changed by inspection. A tracked-only `git diff` is insufficient.
3. At 80x24, use `:` then `needs-attention`; find the review row, read why it appears, press `Enter` and inspect recorded details/report. Use `B`, `Tab`, item selection and `Enter` to open the bound report/artifact. Record visible title/reason, gate/attempt identity, actual evidence content and return path. Check held, revision, preparation, withdrawn, approval-next-step and invalid/missing-information rows against their distinct facts.
4. Repeat with a review request whose room is missing and one with a valid inventory but missing artifact. Record the displayed failure rather than substituting task text as verified evidence. Remove/change an already-open artifact in the disposable fixture between captures to exercise existing refresh invalidation; this deliberate fixture mutation belongs to a separate run and must not contaminate AC-4's no-write comparison.
5. Confirm a normal idle task and an old/no-session task do not appear. Keep review/hold inclusion independent of detected runtime activity. Exercise `all-tasks`, archive exit, sorting, search, workflow tabs, empty results, and a missing state checkout warning. Repeat reason/evidence/help reachability at 40x12 and escape/scroll behavior at 20x6; record observed results, not just screenshots.
6. In a separate uncontaminated inspection-only run, execute only navigation/help/preview keys, exit, and perform the before/after comparison from step 2. Do not press the sanctioned sync action `Y`, launch an external editor, or deliberately mutate fixture files in this run.
7. Ask a person unfamiliar with the project, without coaching, to find work awaiting review, explain its inclusion, open the report/supporting evidence, and explain the missing-evidence case. Record their actual actions and paraphrase, any help required, and whether they understand that decisions happen outside Spacetop. Only identify the participant with their consent. An agent-driven PTY run or string assertion cannot satisfy this human criterion.

Proof status at planning: no task 086 actual-terminal walkthrough has been performed, and no task 086 new-reader result is available in the assignment/state. Task 087's archived reports explicitly lacked human comprehension evidence; its merge and its recorded PTY run do not prove task 086. Implementation must supply its own terminal evidence. If no unfamiliar person is available, record AC-2 human evidence as unavailable/incomplete and carry it to the first officer/captain; do not waive, mark passed, or quietly weaken the explicit acceptance criterion.

## Stage Report: plan

- DONE: Map existing typed workflow facts to explicit attention reasons for review requests, recorded blockers, and missing or invalid information; exclude runtime silence, age, process absence, and unsupported guesses.
  The qualifying-facts table maps current durable readiness, selected hold/revise records, preparation diagnostics and broken-task errors; exclusions prohibit prose/schema guesses and all activity-only membership.
- DONE: Plan the focused Needs attention view and navigation to reports or evidence using existing query/app/UI conventions and task 087 explanations, preserving read-only behavior and clear missing-evidence handling.
  The focus/navigation contract uses the existing palette, WorkflowIndex query, report preview and verified B browser; task 087 at `0f7f38e` supplies wording, and unavailable evidence fails visibly without writes or fallback reads.
- DONE: Record implementation ownership, acceptance-criterion evidence and negative scenarios, actual terminal proof, and any unavailable new-reader proof without inventing or waiving it.
  Owned modules, ordered steps, AC falsification matrix, exact final commands and real-terminal/hash-comparison protocol are above; current terminal/new-reader evidence is explicitly unavailable, not claimed complete.
- SKIPPED: Execute implementation checks and task 086 terminal/new-reader acceptance walkthrough.
  Planning-only assignment: no product implementation exists in this stage; implementation and human acceptance proof remain explicit obligations, including AC-2's unfamiliar-person requirement.

### Summary

Planned a focused read-only attention view from existing typed facts, with distinct review, recorded hold/follow-up, preparation and invalid/unverified-information reasons. The implementation path reuses task 087 explanations and current query/browser conventions, specifies negative and integration tests, and preserves the actual-terminal and unavailable new-reader proof obligations without claiming feature acceptance.

## Stage Report: implement

- DONE: Implement the recorded typed attention projection and focused query-backed view with visible reasons, consistent selection/search/reload behavior, and navigation to existing report and safe gate evidence surfaces.
  Product commit `fa4fe00` on `spacedock-ensign/show-tasks-needing-attention` implements the allocation-free core reason projection, `NeedsAttention` filter, per-tab transient palette focus, two-line reasons and existing report/browser routes.
- DONE: Preserve read-only, canonical-state, runtime-suppression, no-guessing, invalid-information and missing-evidence boundaries; reuse task 087 explanations without duplicate conventions.
  Attention presentation reuses `gate_explanation`; precise labels, diagnostic strings and verified-byte readers remain authoritative. Blank decision reasons explicitly say “No reason recorded”; no schema, runtime detector, evidence root or workflow-write capability was added.
- DONE: After edits, run final formatting, cargo test and make lint; exercise actual terminal navigation and byte-inventory safety; update docs and commit implementation plus criterion evidence, explicitly noting unavailable human proof.
  Final `cargo fmt`, `cargo test` (716 passed, four existing ignored tests, 22 suites), `make lint` and `cargo build -p spacetop` passed; final command output is `artifact://150`. README, browser guidance and representative fixtures are committed.
- DONE: AC-1 — classify every supported reason and exclude unsupported guesses.
  `domain::attention::tests::current_facts_are_exhaustive_and_invalid_data_takes_precedence` fails on any wrong readiness mapping, diagnostic-presence rule or invalid-data precedence; core `tests/attention.rs` checks exact fixture membership, compound query filters and clean-repository preparation proof.
- DONE: AC-1 — preserve focused row identity across navigation and refresh.
  `attention_palette_search_sort_reload_and_archive_share_visible_rows` proves a task remains in all-tasks but disappears from focus when its recorded reason is removed; broken-row, tab and continuation-click tests reject hidden selections, focus leakage and incorrect mouse mapping.
- DONE: AC-2 — exercise report/evidence navigation and honest failures, without claiming human comprehension.
  Real PTY at 80x24: `:needs-attention`, `/3k`, Enter, B, Tab, Enter displayed “Review requested: Waiting for your decision.” and SHA-256-verified bytes `Exact candidate review.`; Esc returned through item/browser to the same task.
  Observed binding: gate `gate:docs-dev:3k:validation`, attempt `gate-attempt:3k-validation-1`, Briefing `briefing:docs-dev:3k:validation:attempt-1:revision-1`; the report is the selected task body, not a guessed external path.
  `/missing-room` then B displayed `UNVERIFIED` with `Missing`; `/missing-artifact` then B/Tab/Enter retained the verified inventory but displayed `Missing`, never substituted task text. A separate deliberate-deletion run produced `Preview invalidated: Missing` after the watcher refreshed.
- DONE: AC-3 — keep runtime silence, age and activity out of membership.
  Core integration crosses idle/running/runtime-human-gate, old/recent timestamps and unavailable scan reports with unchanged facts; membership and reasons stay identical. Actual `ordinary` and `old` tasks appeared in all-tasks but not attention; review/hold inclusion did not require session activity.
- DONE: AC-4 — prove inspection-only navigation leaves workflow storage unchanged.
  Before/after complete relative-path + SHA-256 inventories matched for all 26 representative workflow files (including rooms, archive and an untracked control) and separately all 52 `.git` files, including index/refs; no additions/deletions or byte changes. No Y, editor, dispatch, transition or approval action was used.
  Missing-state definition inventory also matched and its absent state root stayed absent. Existing `no_write_git_calls`, `no_terminal_deps`, durable-gate and room regressions passed in the full suite.
- DONE: Exercise actual narrow terminals and contextual failure states.
  At 40x12 and 20x6, Space reached “Waiting for your decision.”, B/Tab/Enter opened the verified report, and help scrolling/Esc returned safely; attention layout now reserves reason space rather than letting the graph consume it.
  Actual all-tasks/archive exits, status sorting, excluded-task search, independent workflow tabs and missing-state empty focus were exercised. The latter retained “Progress files are missing; no task state was loaded” and help's warning that an empty list is not proof of no tasks.
  TestBackend cases cover every reason, wrap on/off, tiny scrolling, empty-state escape, continuation-line mouse selection and worktree-diff reachability. The diff test caught and fixed scrollbar-width rewrapping that previously made the final diff lines unreachable.
- DONE: Retain reproducible terminal setup and observations.
  Platform: Darwin 27.0.0 arm64; debug binary from `fa4fe00`; fixture copied from `tests/fixtures/attention` into a temporary local Git repository, committed as `c5e6f6f1b4353886077083416084fc4531c5d920`; HOME/XDG config/state directories were separate absolute sibling directories.
  Launch: `target/debug/spacetop --workflow-dir <canonical temporary workflow>`, TERM=xterm-256color, SPACETOP_ASCII=1. Captures, input-step labels and complete inventories: `local://attention-086-terminal.json` (116 real PTY observations); inventory-manifest SHA-256 `1b8e7202030333a1026773dac3e5d9c50b043b707a1a30491456417b8029a4cc`.
- SKIPPED: AC-2 unfamiliar-person comprehension exercise.
  No unfamiliar participant or human walkthrough result was available in this assignment. The agent-driven PTY and automated tests are not human evidence; AC-2's human criterion remains incomplete for the first officer/captain and is neither waived nor marked passed.
- SKIPPED: Ignored watcher suite and gate/merge actions.
  Watcher implementation was unchanged; normal refresh was exercised in the real PTY. This stage has no gate approval, transition or merge authority.

### Summary

Implemented the complete recorded attention projection and read-only focused UI in `fa4fe00`, including shared explanations, consistent navigation, verified evidence and explicit missing-data failures. Automated checks and actual-terminal byte-inventory proof passed; the separately required unfamiliar-person AC-2 proof remains unavailable and must be resolved before claiming full acceptance.

## Stage Report: verify

- DONE: Independently review the attention projection, focused row identity, reason rendering and report/evidence navigation against all task acceptance criteria and no-guessing/read-only boundaries; report concrete material defects.
  Reviewed `0f7f38e...fa4fe00`, owning app/query/render paths, fixtures and assertions; found one High technical defect, V-1 below. No implementation edits, approval or merge performed.
- DONE: Assess recorded 716-test, lint, build and actual PTY byte-inventory evidence without repeating green checks; exercise changed-path claims only where evidence is missing or insufficient.
  Inspected `artifact://150` successful suite/guardrail, clippy and build output and `local://attention-086-terminal.json` captures/inventories; accepted recorded formatting evidence without rerunning green checks. Ran only the missing focused-search display/activation scenario.
- DONE: Record explicit per-criterion coverage and verdict, keeping unavailable unfamiliar-person evidence separate from technical defects and never waiving it or claiming it occurred.
  AC-1 and AC-2 have the technical failure below; AC-3 and AC-4 pass reviewed evidence. AC-2 additionally has a separate unavailable human-acceptance obligation.
- FAILED: V-1 — High technical defect: focused search renders a different row set from the one Enter activates.
  `crates/spacetop/src/app.rs:1352-1358` newly filters search activation by attention, but `crates/spacetop/src/ui/search.rs:35-40` still renders the unfiltered query; displayed indices therefore identify different tasks.
  Independent actual PTY reproduction against the existing `fa4fe00` debug binary: launch `target/debug/spacetop --workflow-dir tests/fixtures/attention`, then `:needs-attention`, Enter, `/`, Down, Down, Enter, Enter. Search highlights `consumed`; the opened preview identifies `#feedback`, `feedback.md`, and `feedback-pending` instead.
  Observed at 100x24, TERM=xterm-256color, SPACETOP_ASCII=1, with temporary absolute HOME/XDG roots outside the workflow; output retained in `artifact://164`. The same unfiltered popup is visible in the author's “missing room diagnostic” PTY capture.
  Impact: users can open another task's report/evidence than the highlighted search result, and excluded tasks remain visible in focused search. Align renderer and activation with one filtered result set; add a rendered-search-to-Enter identity regression, including a nonqualifying row before a qualifying row.
- FAILED: AC-1 — focused inclusion and row identity.
  Core classification, invalid-data precedence, diagnostic-presence distinction and main-list reasons pass reviewed tests; overall criterion fails because focused search shows nonqualifying rows and disagrees with activation (V-1).
- FAILED: AC-2 — selected-task navigation technical coverage.
  Successful verified-byte navigation, missing room/artifact failures, refresh invalidation and narrow-screen reachability have app/TestBackend/PTY evidence; V-1 breaks the selected-result-to-report path and prevents technical acceptance.
- SKIPPED: AC-2 — unfamiliar-person comprehension acceptance.
  No unfamiliar participant's uncoached actions/paraphrase are available. This is an outstanding acceptance-evidence gap, not a second code defect; agent PTY evidence cannot satisfy or waive it.
- DONE: AC-3 — runtime silence never means blocked work.
  `domain/attention.rs` accepts only gate facts/readiness/preparation; core `tests/attention.rs:84-118` crosses runtime states, timestamps and unavailable scans against invariant membership. Introducing activity-based inclusion would fail this assertion.
- DONE: AC-4 — inspection remains read-only.
  Reviewed inventories compare equal for all 26 workflow and 52 Git paths/hashes, including additions/deletions; write-safety/core-boundary guardrails passed. Navigation tests assert no pending sync/editor request; no new writer, evidence-root expansion or action capability appears in the diff.
- DONE: Assess remaining integration and presentation evidence.
  Sort/reload/archive/broken-row/tab tests preserve focused identity; continuation-click, reason categories, tiny preview and worktree-diff tests cover their claims. Existing search test at `app/tests.rs:2874-2884` asserts only the internal filtered query, not the popup, explaining why V-1 escaped 716 green tests.
- SKIPPED: Repeat builds, full suites, lint, formatters, and implementation fixes.
  Existing green checks were sufficient for their stated claims; this stage exercised only missing search integration proof and leaves remediation to implementation.

### Summary

Verification verdict: request changes for High defect V-1; the stage review is complete but task acceptance is not. After the focused-search renderer/activation mismatch is fixed and its regression demonstrated, AC-2 still requires the separately unavailable unfamiliar-person walkthrough; neither a technical fix nor captain gate handling may silently count agent evidence as that human result.
