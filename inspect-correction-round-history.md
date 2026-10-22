---
id: 081
title: Inspect correction-round history
status: implement
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Round-pointer and JSONL fixtures, query ordering tests, app navigation tests, and TestBackend assertions; dropping earlier rounds or attributing a resolution to another Briefing must fail."
started: 2026-10-22T14:52:31Z
completed:
verdict:
score: 0.85
worktree: .worktrees/spacedock-ensign-inspect-correction-round-history
issue:
pr:
mod-block:
---

Make recorded correction cycles inspectable without searching through entity folders. Spacedock retains review-round pointers plus immutable Briefing and review JSONL records.

## Scope

- Recommended build order: 3 of 5. Scores express the survey recommendation, not an approved schedule.
- Touches: parser / domain / query / app-state / UI / watcher / docs.
- Non-goals: Recording correction rounds, routing workers, changing acceptance criteria, or mutating immutable rooms.
- Coordinate with support-durable-gate-status for shared typed records; avoid duplicate parsing.

## Acceptance criteria

**AC-1:** Parse the current review-round pointer and discover retained review/<stage>/round-N records through a typed core boundary. Display stage, cycle, reviewer decision, and recorded follow-up dispositions where present.

**AC-2:** Allow users to browse rounds in deterministic order and distinguish the current pointer from older rounds. Show missing, malformed, mismatched, or incomplete evidence explicitly.

**AC-3:** Render recorded Annotation and Resolution entries with actor, time, rationale, and includes relationships; do not infer unrecorded responses, completion, or approval from prose.

**AC-4:** Cover multiple stages and cycles, legacy entities without review-round, malformed or partial JSONL, stale pointers, safe path boundaries, and refresh/rendering behavior.

## Proof plan

- Use parser/scanner and query tests for core facts, app tests for state/input, and Ratatui TestBackend for rendering.
- Run cargo fmt, cargo test, and make lint for code changes. Run the ignored notify smoke test when watcher backend behavior changes.
- Update README and nearby docs for changed user-visible behavior.
- Preserve the read-only workflow contract and existing git/config/session guardrails.

## Survey sources

- Stable release: https://github.com/spacedock-dev/spacedock/releases/tag/v0.27.3
- Durable gate release: https://github.com/spacedock-dev/spacedock/releases/tag/v0.27.0
- Canonical room change: https://github.com/spacedock-dev/spacedock/releases/tag/v0.27.1
- Edge dispatch change: https://github.com/spacedock-dev/spacedock/releases/tag/v0.28.0-pre3
- Spacetop survey baseline: e503807 (main, 2026-08-24).

## Survey evidence

Current Spacetop successfully exported a temporary folder-form entity copied from the Spacedock 0.27.3 advisory-round fixture, but gates and review-round were absent from its typed export. This confirms metadata tolerance and missing projection, not full runtime compatibility. The survey made no production code changes.

## Implementation plan

### Verified contract and compatibility boundaries

Planning only; no product changes or implementation verification are claimed. The authoritative local upstream inspected is the installed `spacedock-pinned___spacedock___0.27.3` source tree. Implementation must vendor the cited small fixtures with upstream version/source provenance so tests do not depend on that private installation.

- `docs/schema/entity.mdschema.yml:124-134`, `internal/gates/model.go:52-79`, and `round.go:175-195` define `review-round` as exactly id/stage/cycle/briefing, with briefing id/digest/room-ref. ID is `round:<entity-id>:<stage>:<positive-cycle>`; stage matches `[a-z][a-z0-9-]*`; room-ref is exactly `./review/<stage>/round-<cycle>`. Validate entity identity as `resolveRound` does. The pointer is not a gate, changes no status, and does not authorize anything.
- Rooms are relative to the entity file's parent, including folder-form `index.md`, not the definition README. `round.go:150-173` reads `briefing.json` and `briefing.review.jsonl`; current-pointer Briefing ID and JCS SHA-256 must match. Discover retained rooms independently of the pointer, but do not attribute a flat entity's shared sibling review directory to it merely by proximity: require Briefing entity/stage/cycle identity and surface ambiguous/mismatched candidates.
- `internal/gates/review.go:12-94` requires nonempty UTF-8 JSONL ending with newline, unique record IDs, `Annotation` or `Resolution`, same Briefing binding, nonempty actor and RFC3339Nano time, and includes referring only to earlier IDs in that log. First Resolution is the reviewer verdict. Non-approve Resolution needs reason or an included Annotation. Existing gate timestamp validation is UTC-specific; do not accidentally reject valid review timestamps with RFC3339 offsets.
- Preserve Annotation `body`, `target`, and `kind` when present; they occur in the real advisory-round fixture even though upstream's minimal Go validation struct ignores them. Missing Annotation body is explicit, not an invented rationale. Resolution uses recorded reason/includes and `GateDecision`; reuse `GateResolution` where applicable through shared validation rather than implement another gate document parser. Advisory actors such as `software:roborev` and `actor:ensign` are not captain authentication.
- `round.go:120-138` defines an answered log: last entry is a Resolution after the first Resolution, or that first/last Resolution is approve. Annotation tail or lone revise/hold verdict remains incomplete. Older incomplete rooms stay readable; closure is not approval and says nothing about each finding being fixed.
- The unchanged `internal/gates/testdata/advisory-round/briefing.review.jsonl` demonstrates two reviewer findings, a revise verdict including both, a later disposition Annotation including both, and a closing revise Resolution including that disposition. The annotation body says declined; show the exact body, not a parsed/inferred completion classification.
- Important fixture incompatibility: its Briefing ID is `briefing:3j:implementation:round-1`, not the gate attempt/revision grammar. Its local artifact URI is `../../../candidate.patch`, contained within the entity parent, and its context Reference is label-only. Do not fabricate GateAttempt records to pass `decode_briefing`, weaken gate validation, or discard this real fixture. Factor shared strict JSON/JCS and inventory primitives while retaining separate gate-versus-round binding/URI rules. Label-only references remain visible but non-openable.

### Ordered implementation and ownership

1. **Typed source facts and strict decoding.** Add core `domain/correction_history.rs` and `parser/correction_history.rs`, registered from their existing module roots. Add absent/valid/invalid review-pointer state to Entity via `parser/item.rs`; reuse/extract duplicate-key-safe YAML support from `parser/gates.rs`, without a second ad hoc frontmatter parser. Introduce typed round identity (entity origin, entity ID, stage, numeric cycle, Briefing ID), provenance (current pointer bound versus retained discovered), Annotation/Resolution records, ordered line ordinals, and typed diagnostics. Separate pointer validity, Briefing binding, log completeness, and artifact availability; a missing artifact must not erase a recorded decision. Keep raw diagnostics attributable to room/file/line.
2. **Safe discovery and evidence reads.** Add core `correction_history.rs` as bounded I/O owner. Reuse `gate_room::safe_root`, descriptor-relative no-follow regular-file reads and 2 MiB evidence bound, strict JSON/JCS, and exact-local Git reader; minimally extract these internals for both callers rather than duplicate them. Enumerate only immediate `review/<stage>/round-N` entries with normalized positive numeric suffixes, no recursive arbitrary directory walk. Enforce explicit entry/round limits with a visible truncated/incomplete diagnostic, not silent loss. Preserve current pointer as a missing row when its target is absent.
3. **Binding and partial data.** Current rooms require exact pointer ID/digest; retained rooms require matching entity/stage/cycle Briefing identity and same-Briefing records, but must not claim a historical pointer digest exists. Invalid pointer never marks a row current. Decode complete JSONL lines in order; keep the valid prefix inspectable, report the first malformed/mismatched/duplicate/forward-reference line, and do not promote later lines or a partial tail into trusted relationships/closure. Annotation-only and empty logs remain explicit incomplete/error states. Unknown record kinds or invalid field types are diagnosed, not silently converted. Do not reinterpret stale current pointers as the numerically latest round.
4. **Round-specific artifact coordinates without weaker reads.** For local round links, normalize relative components against the room with an entity-parent floor, reject absolute paths, escapes, schemes not explicitly supported, mutable `index.md`, symlinks and nonregular files, then pass the root-relative result through the existing descriptor reader. This supports the real `../../../candidate.patch` without changing gate `clean_relative`. Verify raw digest on open; retain exact revision behavior for supported Git links and never fetch network objects. Separate label-only context references from readable inventory items. Revalidate room identity/digest and selected record/item identity on every open.
5. **Index/query boundary.** Extend `index.rs`/`query.rs` with proposed `correction_rounds`, `correction_round`, `correction_record`, and `correction_relationships` methods over owned typed facts; names are planned, not existing APIs. Cache each selected entity's bounded history per snapshot/refresh rather than scan per draw. Order stages by workflow stage order, unknown historical stages lexically afterward, and cycles numerically ascending (2 before 10); records keep physical log order, never timestamp order. Main/state origin owns pointer and retained evidence under worktree merges (`parser/worktree.rs`); keep origin separate from displayed worktree body path. Apply the same identity rules to active/archive sources and same-ID entities in different workflows.
6. **App navigation.** Extend `spacetop/src/app/gate_room.rs` and its App dispatch integration with browser-local `h` to toggle gate attempts/history, including history-only entities with no gate attempts. In history mode Tab switches round/record focus, j/k/arrows select, Enter opens record detail or a selected recorded relationship target, Esc/q returns one level and then closes. Preserve existing B, preview, scrolling and wrap behavior. Keep a typed source/target navigation stack; retain selection by full identity, not list position. Default to the valid current pointer, otherwise latest round in deterministic order with “no current pointer”; never label that fallback current.
7. **Rendering and refresh.** Extend `ui/gate_room.rs`, browser help/footer and existing app/UI tests. Show stage/cycle, current versus retained provenance, reviewer verdict, subsequent recorded dispositions, actor/time/body or reason, includes targets and explicit missing/malformed/mismatch/partial states. Narrow layouts and ASCII mode must retain trust/current labels. UI does no parsing or filesystem reads. Extend `watcher.rs` dependency/event coverage and lib watcher wiring for review directories, Briefing/JSONL create/rename/delete and new rounds; reuse existing debounce/poll fallback. Refresh replaces facts coherently, clears/revalidates previews, retains surviving identity/focus, and reports removed selections rather than opening an adjacent record. Event paths are invalidation signals, never trusted read coordinates.
8. **Docs and integration.** Update README and `docs/gate-room-browser.md` for history keys, ordering, provenance, advisory-versus-gate decisions, failure labels, legacy behavior and includes limitations; update the AGENTS code map only where module ownership changes. No new dependency is anticipated. Keep workflow markdown, immutable rooms, config/session boundaries and Git mutation policy unchanged.

### Typed relationship contract for task 088

081 owns parsing, discovery, diagnostics and relationship queries; 088 owns its question-first/main-report UX and correction-focused presentation. Do not implement 088 in this task.

Proposed query results expose full source/target record identities, edge kind `RecordedIncludes`, direction, ordinal, and validation status. Resolve an includes ID only within its entity-origin/stage/cycle/Briefing/log namespace. Return inverse edges so a finding can navigate to a later Annotation explicitly including it, and onward to a later Resolution including that Annotation. The advisory-round fixture's job-592/job-594 -> decline-duplicate-member -> ensign Resolution chain is the concrete consumer example. Distinguish the reviewer's includes edge (evidence for the request) from later recorded inclusion (follow-up evidence); show roles from validated log order, not guessed actor names.

There is **no authoritative cross-round response edge** in this contract: upstream rejects includes IDs not earlier in the same log/Briefing. A multi-round test must demonstrate linked responses within a round and prove adjacent rounds, repeated IDs, similar prose, actor names or timestamps do not create links. The query returns `NoRecordedRelationship` or unavailable/invalid evidence, never “fixed” or “approved” from absence. Dangling references are visible invalid edges, not navigable targets. A closing Resolution provides a round-level answered fact only; a finding without an explicit incoming follow-up edge remains without a recorded response relationship. Do not manufacture a transitive edge: return the actual ordered edge path with each intermediate record.

This satisfies 088's dependency as recorded response navigation, not a guarantee of responses to every finding. If 088 requires cross-Briefing linking or semantic fixed/declined classification beyond the recorded body, that requirement remains blocked on an upstream explicit relationship schema; do not silently broaden 081 or misrepresent includes as such a schema.

### Acceptance proof and falsification

| AC | Lowest-layer implementation proof | Falsifying change |
| --- | --- | --- |
| AC-1 | Vendor the unchanged advisory-round files and add normalized local multi-stage/multi-cycle fixtures in `tests/fixtures/correction_history`; core parser/integration tests cover exact pointer keys/ID/digest, retained discovery, numeric order and actor/body/verdict/follow-up projection. | Drop an earlier room, sort cycles lexically, treat the last Resolution as reviewer verdict, reject the real round Briefing grammar, or conflate advisory closure with a gate approval. |
| AC-2 | Core query tests plus app tests cover current pointer to an older round, missing/stale pointer, malformed YAML/Briefing, digest/identity mismatch, missing log/artifact, duplicate IDs, invalid UTF-8, incomplete tail and removed selection. | Automatically mark latest current, erase a valid prefix, hide a failure behind an empty list, or preserve trusted preview after replacement. |
| AC-3 | Relationship tests use the real two-finding disposition chain plus two stages/three rounds, an unlinked finding, an unrelated response with matching prose, repeated IDs in another Briefing, forward/dangling references, multiple targets, and differing clock order. App tests prove source -> target -> source navigation; TestBackend pins actor/time/body/reason and includes labels. | Link by prose/time/adjacency, resolve a cross-Briefing ID, infer every finding fixed from closure, or lose the originating record on Back. |
| AC-4 | Parser/scanner tests cover legacy no-pointer/no-room and no-pointer/retained-room, flat/folder/archive/split-root/worktree origins, invalid stage/cycle/overflow, oversized files/discovery limits, traversal/symlink/directory replacement, unreadable/nonregular files, exact Git revision, and read-only guards. Watcher/app tests cover atomic rename, append/partial-write/recovery, new round discovery, deletion and polling refresh. TestBackend covers narrow/wide/ASCII layouts. | Read outside the entity/safe root, select dirty working bytes for a Git revision, accept a partial record as complete, fail to invalidate JSONL changes, or mutate workflow files. |

Planned tests live in a new `crates/spacetop-core/tests/correction_history.rs`, parser/query unit tests, existing app gate-room tests and UI TestBackend tests. Extend rather than replace existing `gate_rooms` safety coverage; the unchanged upstream fixture is a compatibility oracle, while generated variants test specific rejection and relationship boundaries. Record upstream provenance and expected limitations alongside fixtures, not private machine paths.

Implementation verification commands: `cargo test -p spacetop-core --test correction_history`, `cargo test -p spacetop-core --test gate_rooms`, `cargo test -p spacetop-core correction_history`, `cargo test -p spacetop gate_room`, `cargo test -p spacetop-core watcher`, `cargo test -p spacetop-core --test no_write_git_calls`, and `cargo test -p spacetop-core --test no_terminal_deps`; then `cargo fmt`, `cargo test`, and `make lint`. Because refresh subscriptions change, run `cargo test -p spacetop-core -- --ignored` for real notify evidence. These are future implementation checks, not runs performed during planning.

Actual terminal proof is required after implementation, not substituted by TestBackend: document a self-contained fixture README with `mktemp -d`, copy the committed fixture into a disposable workflow, initialize/commit a local Git repository with explicit test identity, and use absolute XDG config/state directories outside it. Run `cargo run -p spacetop -- --workflow-dir "$proof_root/workflow"` in a real PTY. Open B then h; browse two stages and cycles 1/2/10, verify current is not inferred from newest, follow the real includes chain and Back, inspect missing/unlinked/malformed states, resize narrow/wide, and repeat with `SPACETOP_ASCII=1`. From a separate terminal modify only that disposable fixture: append an incomplete line, finish it, rename/delete a room and add a new round; observe refresh preserving identity while invalidating stale evidence. Capture terminal output/screenshots and exact actions/dimensions/exit result in the implementation report, and compare fixture bytes/Git diff before intentional external edits to demonstrate the UI itself did not write. No terminal behavior has been exercised in this planning stage.

## Stage Report: plan

- DONE: Verify the current Spacedock review-round pointer, retained Briefing and JSONL contract from authoritative local schema/examples; map reusable safe readers and existing typed gate records without duplicate parsing.
  Verified upstream schema, model/review/round implementations and unchanged advisory-round fixture; identified round-ID, relative-URI and label-only Reference differences from the existing gate decoder.
- DONE: Plan typed correction-round discovery and relationship queries plus deterministic history navigation/rendering, covering all four acceptance criteria, legacy data, malformed/partial/mismatched evidence and safe refresh behavior.
  Ordered implementation assigns domain/parser/safe I/O/index/app/UI/watcher/docs ownership with separate provenance, partial-log diagnostics, identity-preserving refresh and numeric ordering.
- DONE: Define implementation ownership and falsifiable tests plus actual terminal proof; expose the recorded correction-to-response relationships required by task 088 without implementing 088 or inventing inferred links.
  AC matrix and commands specify lowest-layer and real-PTY proof; 088 contract exposes validated same-Briefing inverse includes paths and explicitly disallows cross-round inference.
- SKIPPED: Product implementation and build/lint/tests/formatters/terminal execution.
  Planning-only assignment; future proof is specified without claiming observed product behavior.

### Summary

Completed a source-grounded implementation and proof plan for all four acceptance criteria. The plan reuses safe evidence and typed gate primitives while separating advisory rounds from gate authority, and gives task 088 concrete recorded-response navigation without inventing cross-Briefing links.

## Stage Report: implement

- DONE: Implement the recorded typed correction pointer, safe retained-round discovery, strict Briefing/JSONL decoding and same-Briefing relationship query contract, reusing existing evidence primitives without weakening gate safety or inventing relationships.
  Product commit `9a3c37f` on `spacedock-ensign/inspect-correction-round-history`; shared duplicate-safe YAML/JSON/JCS, resolution validation, descriptor reads and exact-local Git readers retain gate boundaries.
- DONE: Deliver deterministic history browsing, actor/rationale/includes detail, current versus historical provenance and coherent refresh across all four acceptance criteria; preserve read-only behavior and task 088 ownership boundaries.
  `B` then `h` browses cached typed history, record/item identities and a Back stack; recorded outgoing/incoming includes expose reviewer-evidence versus follow-up roles only within one Briefing. No task-088 question-first/main-report UX was implemented.
- DONE: After implementation, run required final checks including real notify coverage, actual terminal history/relationship/invalid-data scenarios and unchanged-file proof; update docs, commit product changes and record falsifiable per-criterion evidence. Keep operational correction records in state, not new product evidence directories.
  README, browser contract, AGENTS ownership map and self-contained fixture README updated; reusable upstream/normalized fixtures live under `tests/fixtures/correction_history`, operational evidence remains local and this report remains in split-root state.

### Acceptance evidence

- AC-1: `oracle_numeric_order_current_pointer_and_advisory_closure` validates unchanged v0.27.3 Briefing digest, cycles 1/2/10 across implementation/validation, cycle-1 current pointer, first reviewer Revise, actor/body and answered-not-approved closure; dropping earlier rooms, lexical cycles or last-verdict substitution fails.
- AC-2: Pointer-schema, missing/invalid/stale/absent-pointer, malformed-log and binding tests preserve valid prefixes and explicit failures; app tests retain full identity or clear removed selections, invalidate previews and refuse changed detail opens. Inferring newest as current, trusting a partial tail or opening an adjacent replacement fails.
- AC-3: `same_briefing_relationships_expose_actual_chain_not_prose_or_adjacent_rounds` pins job-592 -> disposition -> ensign Resolution as two real inverse edges, with repeated IDs/prose and reversed clock ordering in another round as negative controls; inventing cross-round/transitive links fails.
- AC-3 UI: App Back-stack tests and TestBackend assertions pin actor/time/body/reason/includes at 150x50 and current/trust/Back at 80x24, 40x12 and 20x6; both real PTY runs followed the recorded chain and returned to the originating finding.
- AC-4: Integration tests cover folder/flat/archive/split-root/worktree origins, strict pointer keys/cycle overflow, malformed/duplicate/mismatched/invalid-UTF-8/partial logs, size/discovery limits, traversal/symlinks/nonregular/unreadable evidence, changed bindings, exact Git blobs and unchanged Git diff; unsafe reads or dirty-worktree fallback fails.
- AC-4 refresh: Real notify smoke covers partial append, tail completion, atomic replacement, new rounds and deletion; real PollWatcher covers log replacement. Existing gate-room artifact/state-replacement smoke and both static read-only/core-boundary guardrails pass.

### Commands and terminal proof

- Passed: `cargo fmt`; `cargo test` (735 passed, 6 intentionally ignored); `make lint` (`cargo clippy --all-targets --all-features -- -D warnings`).
- Passed individually: `cargo test -p spacetop-core --test correction_history` (12); `--test gate_rooms` (12, one ignored); `cargo test -p spacetop-core correction_history`; `cargo test -p spacetop gate_room`; `cargo test -p spacetop-core watcher`; `--test no_write_git_calls` (2); `--test no_terminal_deps` (1). Final command output: local tool artifact `263`.
- Passed: `cargo test -p spacetop-core -- --ignored` (6 real-backend tests), including both new correction-history notify/poll tests and existing gate-room smoke; output artifact `255`.
- Actual PTY command: `cargo run -p spacetop -- --workflow-dir "$proof_root/workflow"` with absolute external XDG config/state roots, then repeated with `SPACETOP_ASCII=1`; both exited 0. Captured sizes: 150x50, 40x12, 20x6, then wide again.
- Actual actions: B/h, numeric cycles 1/2/10 and second stage; Tab/Enter finding, j/Enter disposition, jj/Enter closing Resolution, Esc/Back twice; unlinked cycle-2 finding, partial cycle-10 log, legacy empty history and missing current target all observed explicitly.
- A second PTY modified only the disposable fixture: partial append -> completed Annotation -> atomic log replacement, selected round rename, new round-3 discovery/deletion, and selected artifact removal. Notify refresh preserved current identity, cleared removed selections and invalidated artifact preview without erasing reviewer Revise.
- Inspection-only proof: all 13 fixture regular-file SHA-256 values matched before/after and tracked/untracked `git status --porcelain` was empty; repeated ASCII inspection remained unchanged before deliberate mutations. No application workflow writes were observed.
- Raw PTY bytes, 52 labeled screen/action/dimension captures, mutation actions, exits and file inventories: `/tmp/spacetop-081-proof-evidence.json`, SHA-256 `7f4c02ca865f730743cbb150a5e99c1900c4104da6f4e1512e834d20b3235015`. Disposable root: `/private/var/folders/h_/tb6bynfj41g1dcvpb2472vrh0000gn/T/spacetop-081-proof-hq6k7igo`.
- Capture tooling: Python standard-library PTYs plus isolated `pyte==0.8.2` for transcript rendering. REA capture failed its process-cleanup ownership check and supplied no usable proof; reported as a tool issue, replaced by the successful independently owned PTYs above.

### Summary

Implemented all four acceptance criteria with typed, bounded, read-only correction history and explicit same-Briefing relationship navigation. Current pointer provenance, incomplete evidence, artifact verification and advisory closure remain separate facts; no cross-round response, finding completion or gate approval is inferred. Product and operational evidence are committed/referenced without approving a gate, merging work, or implementing task 088.

## Stage Report: verify

- DONE: Independently verify all four acceptance criteria and typed correction-round schema compatibility, exact identity/provenance, safe bounded reads, partial-data handling and same-Briefing relationship semantics; report concrete material defects.
  Independently reviewed product commit `9a3c37f` against its parent, the specified contract, implementation/tests and retained execution evidence; found two reproducible navigation/presentation defects below.
- DONE: Review recorded 735-test, lint and six real-watcher results plus actual terminal navigation/refresh evidence without repeating green checks; exercise only missing or insufficiently proven changed-path claims.
  Inspected artifacts `263`/`255` and actual PTY screens/actions; ran only two previously uncovered app/TestBackend probes, whose observations reproduce F1/F2 (artifact `281`).
- DONE: Record per-criterion evidence and verdict, including task 088 consumer relationship readiness, preserving read-only and no-authentication/no-inferred-approval boundaries; do not implement fixes or approve the gate.
  Criterion verdicts and the concrete consumer API are recorded below; no production changes, frontmatter changes, gate action or merge action were made.
- SKIPPED: Repeat full builds/tests/lint/formatters, implement corrections, or retain temporary verification scaffolding.
  Existing green checks were not repeated; the temporary two-probe test source was removed after recording observations.

### Findings and proposed classifications

- **F1 — High / material — selected response silently changes to another relationship after refresh.** `crates/spacetop/src/app/correction_history.rs:18,78,183-197` stores the selected relationship as an ordinal, clamps it after replacing facts, then opens the new edge at that ordinal. This violates identity-preserving refresh and the requirement to report removed selections rather than open an adjacent record; task 088 must not reuse this behavior.
  Reproducer using the committed sample: B, h, Tab, Enter, j selects the incoming `annotation:decline-duplicate-member` response to job-592. Replace round-1's log with its first three complete newline-terminated records and refresh. Observed `selected_before=annotation:decline-duplicate-member; selected_after=resolution:roborev-3j-round-1; notice=None`; Enter opens that reviewer Resolution. Retain the selected edge by full source/target/direction identity and explicitly invalidate a removed edge instead of retargeting it.
- **F2 — Medium / material — opening an artifact loses current/retained provenance on narrow terminals.** `crates/spacetop/src/ui/gate_room.rs:15-20,47-118` replaces the provenance banner with `Item SHA-256 verified` and omits the round column below width 70; `app/correction_history.rs:296-299` also replaces all contextual content with artifact text. Thus digest verification is visible but the current-versus-historical distinction required by AC-2 and the rendering plan is not.
  Reproducer: on the unchanged sample use B, h, Tab, j five times, Enter at 40x12. The actual TestBackend screen contains `Item SHA-256 verified | read-only` and the candidate text, but no CURRENT, RETAINED, stage or cycle. Preserve round provenance separately from item-byte verification while previewing; add current and retained narrow-preview coverage.

### Per-criterion verdicts

| Criterion | Verdict | Inspected evidence and falsification |
| --- | --- | --- |
| AC-1 | PASS | Strict duplicate-safe pointer/Briefing/log decoding, unchanged v0.27.3 fixture digest, stage ordering and numeric 1/2/10 ordering, first reviewer verdict, actor/body and advisory closure are covered by the 12 core integration tests and observed PTY screens. Dropping round 2, sorting lexically or using the closing Resolution as reviewer would fail those assertions. |
| AC-2 | FAIL | Current older pointer, retained/no-pointer fallback, missing target, malformed/mismatched evidence and valid-prefix visibility are evidenced; however F2 removes the promised provenance during narrow artifact inspection, and F1 silently substitutes a removed relationship selection. |
| AC-3 | PASS for typed record/relationship semantics; navigation correction required | Real job-592 -> disposition -> closing Resolution edges, source/target identities, reviewer-evidence versus follow-up roles, physical order, Back navigation and negative controls for repeated IDs/prose in another round are exercised. No inferred cross-Briefing edge or finding-completion/approval classification was found. F1 affects navigation after evidence replacement, not the validity of the core edge graph. |
| AC-4 | FAIL pending F1/F2 | Existing tests and real PTY evidence establish origins, legacy cases, unsafe-path/size/digest/exact-Git refusal, partial-tail recovery, round rename/add/delete and artifact invalidation. The new probes falsify identity-preserving relationship refresh and narrow-preview provenance, which existing tests did not exercise. |

### Verification evidence and limits

- Artifact `263` contains the focused checks, the full 735-pass/6-ignored run and successful `cargo clippy --all-targets --all-features -- -D warnings`; artifact `255` contains six passing real-backend checks. Formatting is recorded by the implementer; no formatter was rerun.
- Read the actual navigation, actor/rationale/includes, missing/legacy/partial, narrow/ASCII and mutation screens in `/tmp/spacetop-081-proof-evidence.json`; its independently recomputed SHA-256 is `7f4c02ca865f730743cbb150a5e99c1900c4104da6f4e1512e834d20b3235015`. The receipt includes raw PTY bytes, 52 frames, both zero exits and the unchanged 13-file inspection inventory.
- Real notify evidence shows partial append -> completed Annotation -> atomic replacement, selected-round removal, new-round discovery/deletion and artifact-preview invalidation. Poll evidence is narrower: `watcher_fs.rs:173-203` exercises a direct PollWatcher with `compare_contents(true)`, not the production fallback's default configuration or end-to-end app reload; do not describe it as full production polling proof.
- Additional execution: `cargo test -p spacetop --test correction_history_verify -- --nocapture` ran two temporary probes (`removed_selected_relationship_retargets_to_reviewer`, `artifact_preview_hides_round_provenance_on_narrow_terminal`). Both passed assertions documenting the defects, not acceptance assertions; artifact `281` preserves the observed identities and 40x12 screen. The reproducible fixture/key/mutation procedures above remain after probe removal.
- Read-only boundary: safe-root/descriptor-relative no-follow regular-file reads, 2 MiB bounds, discovery caps, exact local Git blobs and digest checks remain shared with gate rooms. Main/state evidence origin survives worktree body merges; historical rows do not claim a pointer digest. Existing guards and inspection inventory provide exercised no-write evidence; recorded actor/decision text is neither authentication nor approval authority.

### Task 088 consumer readiness

The backend prerequisite now exists: `WorkflowIndex::correction_rounds(&Entity) -> CorrectionHistory`, then `CorrectionHistory::{correction_round, correction_record, correction_relationships}`; the consumer owns/caches the bounded snapshot. Relationships carry full source/target identities, direction, includes ordinal, validity and reviewer-evidence/follow-up role; invalid edges and partial-log diagnostics remain available on each round. Task 088 can consume same-Briefing inverse includes without another parser, and use `correction_history::read_item` for contained round-relative evidence. It must not equate a response or Answered with fixed/approved, treat reviewer evidence as a response, invent cross-round links, or inherit F1's ordinal-based selection; integration readiness is conditional on correcting the shared UI defects, not a gate/merge decision.

### Summary

Independent verification recommends **request changes** for F1 and F2, both proposed material findings grounded in executed missing-path probes. The typed schema, bounded read-only evidence and same-Briefing query contract have substantial existing proof, but the delivered navigation/rendering does not yet satisfy the full refresh/provenance contract; no gate approval or merge was performed.
