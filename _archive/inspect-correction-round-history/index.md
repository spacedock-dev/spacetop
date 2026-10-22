---
id: 081
title: Inspect correction-round history
status: done
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Round-pointer and JSONL fixtures, query ordering tests, app navigation tests, and TestBackend assertions; dropping earlier rounds or attributing a resolution to another Briefing must fail."
started: 2026-10-22T14:52:31Z
completed: 2026-10-22T16:46:24Z
verdict: PASSED
score: 0.85
worktree: .worktrees/spacedock-ensign-inspect-correction-round-history
issue:
pr: pr-merge:89
mod-block:
review-round:
    id: round:081:verify:2
    stage: verify
    cycle: 2
    briefing:
        id: briefing:081:implement:round-2
        digest: sha256:766287b3014674caa54d49514341f292c61802b1879dc163fc208a418a1aae58
        room-ref: ./review/verify/round-2
gates:
    version: 1
    records:
        - id: gate:081:verify
          stage: verify
          attempts:
            - id: gate-attempt:081-verify-1
              briefing:
                id: briefing:081:verify:attempt-1:revision-1
                digest: sha256:adb9e26781e500a62322a55dc71536b32cee195804d41f6677fd90fb9cb758a0
                room-ref: ./review/verify/briefing-1
              resolution:
                type: Resolution
                id: resolution:spacedock:081:verify:1
                briefing: briefing:081:verify:attempt-1:revision-1
                by: person:captain
                at: "2026-10-22T16:42:04.76572Z"
                decision: approve
                reason: Captain approved the presented task 081 verify gate and PR publication. All four criterion verdicts and evidence were directly inspected in the verifier table; structured AC scanner format limitation was disclosed.
              application:
                target-stage: done
                state: consumed
archived: 2026-10-22T16:46:24Z
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

## Stage Report: implement (cycle 1)

- DONE: Implement the recorded typed correction pointer, safe retained-round discovery, strict Briefing/JSONL decoding and same-Briefing relationship query contract, reusing existing evidence primitives without weakening gate safety or inventing relationships.
  Preserved the existing core implementation and task 088 boundary; product correction commit `e46270387660287d849a44e03103eaa636711b4c` changes only app selection, UI preview headers, their regressions and nearby docs.
- DONE: Deliver deterministic history browsing, actor/rationale/includes detail, current versus historical provenance and coherent refresh across all four acceptance criteria; preserve read-only behavior and task 088 ownership boundaries.
  F1 now stores optional source/target/direction identity, preserves reordered edges, clears removed edges with a notice and blocks implicit substitution; F2 fixes provenance outside scrollable artifact bytes.
- DONE: After implementation, run required final checks including real notify coverage, actual terminal history/relationship/invalid-data scenarios and unchanged-file proof; update docs, commit product changes and record falsifiable per-criterion evidence. Keep operational correction records in state, not new product evidence directories.
  Final checks and actual PTY evidence below passed; README and browser docs updated. Canonical input files remain temporary, with no product evidence directory, round publication, entity migration, frontmatter edit, gate approval or merge.

### Feedback Cycles

- Cycle 1, F1: **High / Material -> fix**, exactly as FO authorized. Removed ordinal clamping; full source/target/direction identity is now the navigation selection. Removed edges require explicit j/k or arrow reselection.
- Cycle 1, F2: **Medium / Material -> fix**, exactly as FO authorized. Current/retained provenance, stage/cycle and read-only status remain in a fixed preview header, independently of item SHA-256 verification.
- No finding was reclassified or declined; no acceptance criterion or task 088 ownership was changed.

### Acceptance evidence

- AC-1: Existing typed pointer/discovery/strict-decoding implementation remains unchanged; the final full suite includes the numeric-cycle/current-pointer/reviewer oracle and core safety tests. Dropping cycles, weakening binding, or substituting last Resolution for reviewer still fails those tests.
- AC-2: `gate_room_history_removed_relationship_requires_explicit_reselection` truncates the real fixture log after selection, refreshes twice and presses Enter: selection stays absent and the finding remains open until explicit reselection. Ordinal replacement or automatic first-edge fallback fails.
- AC-3: `gate_room_history_relationship_survives_log_reordering_by_identity` moves the disposition ahead of the reviewer and checks the same source/target/direction at its new index, then opens it. PTY additionally traversed finding -> disposition -> closing Resolution and Back; no new relationship semantics were introduced.
- AC-4: `gate_room_correction_preview_keeps_current_and_retained_identity_when_narrow` opens verified bytes for both provenances at 40x12, 80x24 and 150x50, then scrolls/toggles wrap. Losing provenance, stage/cycle, verification or artifact text fails. Normal and ASCII 40x12 actual previews also retain these facts.

### Commands and terminal proof

- Passed final `cargo fmt && cargo test && make lint && cargo test -p spacetop-core -- --ignored && cargo build -p spacetop`: 738 normal tests passed, 6 intentionally ignored; all 6 real-backend ignored tests passed; clippy with warnings denied and build passed. Output: `artifact://294`.
- Actual `cargo run -p spacetop -- --workflow-dir <disposable workflow>` PTYs, normal and `SPACETOP_ASCII=1`, both exited 0; captured history ordering, second stage, partial-JSONL diagnostic, real response chain/Back, and narrow current/retained artifact previews.
- A separate mutation PTY reordered then removed the selected disposition. Live notify refresh retained the same edge after reorder, explicitly cleared it after removal, refused Enter on the adjacent reviewer, then opened that reviewer only after explicit j selection. The original log was restored.
- Inspection-only inventory: all 13 fixture regular-file SHA-256 values unchanged; tracked/untracked Git status empty. Inventory and Git status also matched after deliberate mutations were restored. XDG config/state were absolute external directories.
- Raw output, actions, 78 screen frames, mutation scripts, inventories and both exits: `/tmp/spacetop-081-cycle1-terminal-evidence.json`; SHA-256 `d76d18726a91011b018ed2a19cc6732fc65bf7b6df3c0e433e47ddc1e08573a8`. Capture uses Python standard-library PTYs and isolated `pyte==0.8.2`; no product dependency.
- Disposable proof root: `/private/var/folders/h_/tb6bynfj41g1dcvpb2472vrh0000gn/T/spacetop-081-cycle1-52viv899`. Prior report's broader safety coverage remains applicable; no stronger production polling claim is made.

### Canonical correction inputs for FO publication

- Briefing: `/tmp/spacetop-081-cycle1-round-inputs/briefing.json`; review log: `/tmp/spacetop-081-cycle1-round-inputs/briefing.review.jsonl`; receipt: `/tmp/spacetop-081-cycle1-round-inputs/receipt.json`.
- Prepared against installed upstream Spacedock 0.27.3 `operation.go` Briefing fields, `review.go` complete-line/same-Briefing/prior-includes rules, `model.go` Resolution rules and `round.go` answered-log rule. Eight records include both findings, reviewer revise, both FO dispositions, both worker responses and closing advisory revise; no approval fabricated.
- Briefing JCS digest: `sha256:af4fc0cc529ede12b4cd0a43d0d0b07e02ddb7eb09dfc877df1a239a13ed8784`. Artifacts bind exact rejected-source bytes to immutable commit-qualified raw GitHub URLs; upstream accepts URI schemes without fetching. Remote availability was not verified.
- Findings/dispositions are explicitly transcribed from the review/dispatch, timestamped at preparation rather than inventing their original timestamps. Upstream recorder was not invoked; FO owns round publication and flat-to-folder migration.

### Summary

Addressed exactly the two authorized material findings in product commit `e462703`. Identity-preserving relationship refresh and narrow preview provenance now have falsifiable app/render tests and actual terminal proof; all final checks passed. Canonical correction inputs are ready for FO handling, with independent verification and gate decisions still outside this worker's authority.

## Stage Report: verify (cycle 1)

- DONE: Independently verify all four acceptance criteria and typed correction-round schema compatibility, exact identity/provenance, safe bounded reads, partial-data handling and same-Briefing relationship semantics; report concrete material defects.
  Reviewed corrected product `e46270387660287d849a44e03103eaa636711b4c`, the F1/F2 delta, core contracts and upstream 0.27.3; F1/F2 are resolved, but the newly published real round exposes material schema incompatibility F3 below.
- DONE: Review recorded 735-test, lint and six real-watcher results plus actual terminal navigation/refresh evidence without repeating green checks; exercise only missing or insufficiently proven changed-path claims.
  Inspected original artifacts `263`/`255`, corrected `294` (738 normal passes, six ignored-backend passes, clippy/build success), and both terminal receipts; executed only the previously uncovered actual-published-pointer/unchanged-upstream-pointer probe.
- DONE: Record per-criterion evidence and verdict, including task 088 consumer relationship readiness, preserving read-only and no-authentication/no-inferred-approval boundaries; do not implement fixes or approve the gate.
  Per-criterion verdicts and consumer constraints follow; only this report is changed, with no product fixes, frontmatter edits, round publication, gate approval or merge.

### Findings

- **F1 — resolved.** `app/correction_history.rs:11-29,98-109,167-179,222-244` retains source/target/direction rather than ordinal, clears removed relationships and refuses Enter without explicit reselection.
  The passing removal test refreshes twice and refuses adjacent-reviewer substitution; the reorder test preserves identity at its new index. Cycle-1 PTY frames show reorder, removal notice, unchanged finding after Enter, then reviewer opening only after explicit j; reverting to ordinal clamping falsifies both.
- **F2 — resolved.** `ui/gate_room.rs:9-57` keeps round provenance, stage/cycle and read-only in a fixed artifact header, independently of item-byte verification.
  The passing current/retained preview test covers 40x12, 80x24, 150x50 plus scroll/wrap; normal and ASCII PTYs visibly retain all labels and artifact text at 40x12. Replacing provenance with the digest banner falsifies these assertions.
- **F3 — High / proposed material — valid upstream-published rounds are rejected by an invented Briefing-ID grammar.** `crates/spacetop-core/src/parser/correction_history.rs:52-56,92-96` requires `briefing:<entity>:<room-stage>:round-<cycle>`; `crates/spacetop-core/src/correction_history.rs:122-126` synthesizes the same ID for discovery. Upstream `internal/gates/round.go:100-102,175-195` instead copies the manifest ID and requires a nonempty pointer Briefing ID, binding the exact ID plus digest on read at `57-82`.
  The already-recorded task081 `verify/1` pointer binds `briefing:081:implement:round-1` with digest `af4fc0cc529ede12b4cd0a43d0d0b07e02ddb7eb09dfc877df1a239a13ed8784`. An executed read-only probe against current core returns pointer `Invalid`, then `verify/1` as `Retained`, `has_briefing=false`, `records=0`, `IdentityMismatch`; its eight valid records become inaccessible.
  The same probe rejects the unchanged vendored upstream `recorded-entity.md` (`id: task`, Briefing `briefing:3j:implementation:round-1`). The fixture README explicitly normalizes that valid upstream identity difference away, so current compatibility tests cannot detect this defect. This is an existing implementation/plan assumption exposed by real publication, not a regression introduced by the F1/F2 correction.
  Required correction: preserve exact bound upstream Briefing identities instead of deriving them from entity/stage/cycle; retain origin/room namespace, digest, same-Briefing, safe-root and ambiguous-flat-origin safeguards. Add the unchanged upstream pointer and actual differing-stage publication as compatibility regressions; do not rename or republish immutable evidence to accommodate the parser.

### Per-criterion verdicts

| Criterion | Verdict | Evidence and falsification |
| --- | --- | --- |
| AC-1 | FAIL: F3 | Numeric 1/2/10 and stage ordering, first reviewer verdict and follow-up projection pass for normalized fixtures, but the actual current upstream pointer yields no records. Accepting the unchanged published ID rather than the synthesized ID changes this probe's result. |
| AC-2 | FAIL: F3; F1/F2 resolved | Removal/reorder and narrow-preview regressions now pass; missing/stale pointers and malformed/partial logs remain explicit. The valid actual current pointer is nevertheless mislabeled invalid/retained and cannot be browsed. |
| AC-3 | PASS for decoded same-Briefing semantics; blocked for F3 rooms | The real finding -> disposition -> closing Resolution chain and Back, actor/time/body/reason, reviewer-evidence versus follow-up roles, and repeated-ID/unlinked negative controls remain covered. Cross-round/prose-derived links or a transitive shortcut fail existing assertions; F3 prevents these records from reaching the API for the published round. |
| AC-4 | FAIL on compatibility coverage; other inspected coverage passes | Core safety tests cover origins, legacy data, partial prefixes, malformed bindings, traversal/symlink/nonregular/size limits and exact Git blobs; watcher/PTY evidence covers replacement/deletion and preview invalidation. The normalized identity oracle misses actual upstream pointers; F1/F2 refresh/rendering gaps are now closed. |

### Evidence, boundaries and task 088 readiness

- New executed probe: `/tmp/spacetop-081-cycle1-reverify-evidence.json` preserves stdin Rust source, exact compile/run commands, zero exits and observed failures. It links the existing core build without rerunning green suites; the temporary executable was removed. No upstream recorder/validator was invoked by this reviewer; upstream acceptance is grounded in existing publication and inspected recorder/readback source.
- Independently recomputed receipt SHA-256 values match: cycle-1 `/tmp/spacetop-081-cycle1-terminal-evidence.json` = `d76d18726a91011b018ed2a19cc6732fc65bf7b6df3c0e433e47ddc1e08573a8`; original `/tmp/spacetop-081-proof-evidence.json` = `7f4c02ca865f730743cbb150a5e99c1900c4104da6f4e1512e834d20b3235015`.
- The corrected receipt has 78 frames, both zero exits and identical 13-file inspection/restored-mutation inventories with empty Git status. Early compile-buffer frames are not proof; the later ready/redrawn frames establish the claimed behavior. Formatting remains implementer-recorded, not rerun.
- Safe reads remain descriptor-relative/no-follow/regular-file checked, capped at 2 MiB with bounded discovery; explicit opens revalidate evidence and exact local Git blobs without fetch. The no-write/no-terminal guard results remain green. Poll evidence still uses `compare_contents(true)` directly, not a full exercise of production fallback defaults.
- Task 088 can reuse `WorkflowIndex::correction_rounds` and `CorrectionHistory::{correction_round, correction_record, correction_relationships}`, full origin/round/Briefing record identities, inverse includes, edge validity/roles and `correction_history::read_item`; F1/F2 no longer block those navigation primitives. **End-to-end consumer readiness remains blocked by F3**, since the actual upstream round exposes no records.
- Task 088 still owns its question-first/report UX. Relationships remain same-Briefing recorded edges only; Answered, incoming responses, actor text and closing revise never authenticate a person, prove findings fixed or authorize approval. Unsupported cross-Briefing/semantic-response requirements remain outside this schema.

### Summary

Independent re-verification resolves both authorized correction findings but recommends **request changes** for new High/material F3, with concrete execution against the already-published round and the unchanged upstream pointer. The product cannot yet claim complete upstream round compatibility or task 088 readiness; no fixes, approval, merge or republishing were performed.

## Stage Report: implement (cycle 2)

- DONE: Implement the recorded typed correction pointer, safe retained-round discovery, strict Briefing/JSONL decoding and same-Briefing relationship query contract, reusing existing evidence primitives without weakening gate safety or inventing relationships.
  Product commit `362a936014bb024a152ababcd8a9ebb02b18a1c3` preserves opaque recorded Briefing IDs, exact pointer ID/JCS digest binding and origin/stage/cycle namespaces; no gate decoder or relationship semantics changed.
- DONE: Deliver deterministic history browsing, actor/rationale/includes detail, current versus historical provenance and coherent refresh across all four acceptance criteria; preserve read-only behavior and task 088 ownership boundaries.
  Actual published verify/1 now opens all eight records as CURRENT bound; F1/F2 app/UI code remains unchanged and final tests plus terminal removal/preview proofs preserve those fixes.
- DONE: After implementation, run required final checks including real notify coverage, actual terminal history/relationship/invalid-data scenarios and unchanged-file proof; update docs, commit product changes and record falsifiable per-criterion evidence. Keep operational correction records in state, not new product evidence directories.
  Updated browser/fixture docs; final checks passed (`artifact://339`), terminal and canonical-input receipts remain under `/tmp`; no round publication, frontmatter change, gate or merge action.

### Feedback Cycles

- Cycle 2, F3: **High / Material -> FIX**, exactly as FO authorized. The upstream recorder copies arbitrary nonempty manifest IDs; removed the invented entity/stage/cycle Briefing grammar and synthesis rather than renaming immutable evidence.
- Preserve safeguards: folder-form retained rooms use structural entity ownership and the manifest ID; shared flat-entity candidates without the exact current pointer remain explicitly ambiguous with no exposed records. Origin/stage/cycle remain part of each record/relationship identity; ID and digest mismatches still fail closed.
- F1/F2 remain resolved; task 088 and cross-Briefing/semantic-response inference remain out of scope. The earlier plan's derived Briefing identity assumption is superseded by this authorized compatibility correction.

### Acceptance evidence

- AC-1: `unchanged_upstream_pointer_binds_manifest_not_entity_name` uses the unchanged upstream entity task/Briefing 3j pair and verifies current and retained records; deriving IDs from entity names fails it. `differing_stage_and_opaque_ids_bind_exactly_without_cross_room_edges` covers verify/1 binding implement/round-1 and opaque text with eight records; deriving from stage or cycle fails.
- AC-2: The same regressions reject changed pointer IDs; existing digest/partial-prefix/stale-pointer tests retain explicit failures. Actual task081 terminal frames open records #1 through #8 under CURRENT verify/1 and follow finding -> FO disposition -> Back; rejecting or relabeling the published binding fails this proof.
- AC-3: Existing same-Briefing relationship oracle plus the differing-stage regression retain exact source/target namespaces; actor/body/reason/includes remain visible in actual and disposable terminal runs. Cross-room linking or inferred transitive responses fails these assertions.
- AC-4: Extended flat/folder/archive/split-root/worktree origin test refuses unbound shared flat rooms even for the same entity ID. Existing safe-root/symlink/nonregular/size/exact-Git, F1 relationship removal/reorder and F2 current/retained preview tests all pass; weakened ownership, adjacent-edge substitution or lost narrow provenance fails.

### Commands and terminal proof

- Passed final `cargo fmt && cargo test && make lint && cargo test -p spacetop-core -- --ignored && cargo build -p spacetop`: 740 normal tests passed, six intentionally ignored; all six real-backend ignored tests passed; clippy warnings denied and build passed. Output: `artifact://339`.
- `/tmp/spacetop-081-cycle2-terminal-evidence.json` contains raw PTY bytes, commands, actions and frames: actual task081, disposable multi-stage/cycle/partial-JSONL/relationship browsing, normal and ASCII 40x12 current/retained artifact previews, notify-driven removed-edge invalidation and Enter refusal. All four captured sessions exited zero; one initial disposable run explicitly demonstrated symlink-root rejection before rerunning with the canonical temp path.
- Actual entity and immutable room file inventory was byte-identical before/after inspection. Disposable inspection inventory also matched; the only deliberate mutation truncated the disposable log to three complete records and was restored byte-for-byte. No operational evidence was added to product docs.
- Read-only compiled core probe in `/tmp/spacetop-081-cycle2-round-inputs/receipt.json` confirms actual `CurrentPointer`, exact `briefing:081:implement:round-1`, original `sha256:af4fc0cc529ede12b4cd0a43d0d0b07e02ddb7eb09dfc877df1a239a13ed8784`, eight records and no diagnostics. Temporary probe executable removed.

### Canonical correction inputs for FO publication

- Briefing: `/tmp/spacetop-081-cycle2-round-inputs/briefing.json`; log: `/tmp/spacetop-081-cycle2-round-inputs/briefing.review.jsonl`; receipt: `/tmp/spacetop-081-cycle2-round-inputs/receipt.json`.
- Four actual-schema records retain F3, reviewer revise, the exact FO Material/FIX disposition and the committed worker response. The core decoder verifies same-Briefing/prior-includes and Answered closure; canonical digest is `sha256:766287b3014674caa54d49514341f292c61802b1879dc163fc208a418a1aae58`.
- FO owns publication once as verify/2. This worker did not invoke upstream recording or validation, republish round 1, change immutable bytes, claim approval, or merge.

### Summary

Corrected only the authorized F3 compatibility boundary in product commit `362a936`, preserving F1/F2. Actual published correction history is now inspectable with its original binding and all eight records; committed regression coverage, final checks and terminal proofs retain safety and relationship boundaries. Canonical cycle-2 inputs are ready for independent verification and FO publication.

## Stage Report: verify (cycle 2)

- DONE: Independently verify all four acceptance criteria and typed correction-round schema compatibility, exact identity/provenance, safe bounded reads, partial-data handling and same-Briefing relationship semantics; report concrete material defects.
  Independently reviewed `362a936014bb024a152ababcd8a9ebb02b18a1c3` against rejected `e462703`, upstream 0.27.3 and the published rooms; F3 is resolved, F1/F2 remain resolved, and no new material defect was found.
- DONE: Review recorded 735-test, lint and six real-watcher results plus actual terminal navigation/refresh evidence without repeating green checks; exercise only missing or insufficiently proven changed-path claims.
  Inspected original artifacts `263`/`255` and current `339` (740 normal tests, six ignored then six real-backend passes, successful warnings-denied clippy/build); only the newly published two-room/current-pointer transition received a fresh executable probe.
- DONE: Record per-criterion evidence and verdict, including task 088 consumer relationship readiness, preserving read-only and no-authentication/no-inferred-approval boundaries; do not implement fixes or approve the gate.
  All four criteria PASS below; no product edit, frontmatter mutation, publication, gate action or merge was performed.

### Findings

- **F3 — resolved.** `parser/correction_history.rs:49-54,88-100,118-120` accepts the recorded nonempty ID and binds exact ID/JCS digest; `correction_history.rs:250-276` adopts the manifest ID without synthesizing grammar. This matches upstream `internal/gates/round.go:70-71,100-102,191-192`.
  New read-only probe observes retained verify/1 with exact `briefing:081:implement:round-1`, eight records and original digest `sha256:af4fc0cc529ede12b4cd0a43d0d0b07e02ddb7eb09dfc877df1a239a13ed8784`; current verify/2 has exact `briefing:081:implement:round-2`, four records and `sha256:766287b3014674caa54d49514341f292c61802b1879dc163fc208a418a1aae58`. Both have no diagnostics and remain advisory Revise/Answered.
- **F1 — remains resolved.** Full source/target/direction selection and explicit removal invalidation remain in `app/correction_history.rs:11-29,98-109,222-244`; recorded passing removal/reorder tests fail under ordinal substitution. ASCII PTY frames show notify removing the selected response, Enter retaining the finding rather than opening the adjacent reviewer, and restoration not automatically selecting an edge.
- **F2 — remains resolved.** Fixed provenance header remains in `ui/gate_room.rs:18-32`; current and retained 40x12 normal/ASCII artifact frames display provenance, stage/cycle, read-only and separate item SHA-256 status. The passing narrow-preview regression fails if byte verification replaces round provenance.

### Per-criterion verdicts

| Criterion | Verdict | Evidence and falsification |
| --- | --- | --- |
| AC-1 | PASS | Passing unchanged-upstream and differing-stage/opaque-ID regressions plus the new actual two-room probe preserve exact IDs, stage/cycle, reviewer and dispositions; deriving an ID from entity/stage/cycle or dropping either room fails. The unchanged upstream task/3j pointer also parses successfully in the new probe. |
| AC-2 | PASS | Existing numeric-order/current/stale/mismatch/prefix tests and PTY cycle 1/2/10 navigation remain green. The new probe temporarily selects historical round1 only in memory: eight unchanged records become current while newer round2 stays retained; selecting newest instead fails. F1/F2 retain refresh/provenance proof. |
| AC-3 | PASS | Actual round1 record #1–#8 detail frames show actor/time/body/reason/includes and finding -> FO disposition -> Back. The new probe validates every edge endpoint against its own full room identity in both published logs; cross-room resolution fails. Existing same-Briefing chain tests reject inferred transitive/prose/adjacent-round links and distinguish reviewer evidence from follow-up. |
| AC-4 | PASS | Current artifact `339` includes partial/invalid JSONL, legacy/stale pointers, flat/folder/archive/split-root/worktree origins, bounds, symlinks/nonregular replacement, exact Git/read-only, app/render and all six real-backend checks. Removing prefix stops, safe-root/descriptor bounds or refresh invalidation falsifies these tests; PTYs additionally show partial-tail diagnostics, narrow/ASCII headers and notify invalidation. |

### Evidence and limits

- New probe receipt: `/tmp/spacetop-081-cycle2-reverify-evidence.json`, SHA-256 `0126d79828848b8d77c25fa8557b9737027320724f95682cef597307afc1a724`; contains stdin Rust source, exact commands, zero compile/run exits and all observed record/edge counts. It links the existing core build; the temporary executable was removed.
- Inspected terminal receipt: `/tmp/spacetop-081-cycle2-terminal-evidence.json`, independently hashed `e1973b2efe06fc25b2b2572d31347341334f2f469e0c95009fd65fc1ab642ccf`; four zero-exit sessions include an explicitly unsuccessful symlink-root attempt before canonical-path success. Its actual workflow and disposable inspection inventories are unchanged.
- Published round1 raw SHA-256 values still match that receipt: Briefing `b2cff26e7f260d211889ca93b788e52588f6eae04132cacd443572bc6d27d380`, JSONL `4c729399068136238c616441a1f4bcc426ae0260432fdae82fa1dc0d68e2315f`. Neither round was renamed or republished.
- Existing green suites, formatter, build and terminal navigation were not rerun. Formatting is implementer-recorded; this reviewer inspected retained check output and exercised only the post-publication compatibility gap. No upstream recorder/validator was invoked.

### Task 088 consumer readiness and judgment

Task 088 may consume `WorkflowIndex::correction_rounds` and `CorrectionHistory::{correction_round, correction_record, correction_relationships}` for recorded same-Briefing response navigation, with full origin/stage/cycle/Briefing identity, edge role/direction/ordinal/validity and incomplete-evidence diagnostics. Unbound shared flat-entity rooms intentionally remain ambiguous with no records; retained folder rooms do not claim a historical pointer digest. Cross-Briefing links or semantic fixed/declined classifications remain unsupported without an explicit upstream schema; attribution is not authentication, Answered is not approval, and no missing relationship implies completion.

Code-review judgment: **Approve the reviewed changes**; verification is PASS with no outstanding material findings. This is not captain gate approval or authorization to mutate workflow state; FO/captain retain routing and merge authority.

### Summary

Independent cycle-2 verification resolves F3 against the unchanged upstream pointer and both actually published correction rooms while preserving the F1/F2 fixes. All four acceptance criteria pass with existing green evidence plus a narrowly scoped new read-only compatibility probe; task 088's recorded-relationship dependency is ready within the stated schema boundaries.
