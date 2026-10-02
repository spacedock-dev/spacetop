---
id: 080
title: Browse gate Briefings and decisions
status: verify
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Briefing parser/path-boundary fixtures, app navigation tests, watcher refresh checks, and TestBackend assertions; wrong bindings and escaped refs must be rejected."
started: 2026-10-01T18:17:27Z
completed:
verdict:
score: 0.90
worktree: .worktrees/spacedock-ensign-browse-gate-briefings-and-decisions
issue:
pr: "#84"
mod-block: merge:pr-merge
review-round:
    id: round:080:verify:2
    stage: verify
    cycle: 2
    briefing:
        id: briefing:spacetop-dev:080:verify:round-2
        digest: sha256:82781799905904ca1ac4cdcebf7b570685bf6372c26f46da15321ef60413efc8
        room-ref: ./review/verify/round-2
gates:
    version: 1
    records:
        - id: gate:080:verify
          stage: verify
          attempts:
            - id: gate-attempt:080-verify-1
              briefing:
                id: briefing:080:verify:attempt-1:revision-1
                digest: sha256:e0826b843dab37759aaab3a83baafa5479782b51165e6cebb98516ad35d03c4a
                room-ref: ./review/verify/briefing-1
              resolution:
                type: Resolution
                id: resolution:spacedock:080:verify:1
                briefing: briefing:080:verify:attempt-1:revision-1
                by: person:captain
                at: "2026-10-02T01:44:00.229561Z"
                decision: approve
                reason: Captain approved the presented task 080 verify gate and its evidence; authorize PR publication.
              application:
                target-stage: done
                state: pending
---

Let users inspect a gate question, its evidence, and its recorded decision from Spacetop. Spacedock 0.27.1 prepares one canonical Briefing instead of requiring request.json.

## Scope

- Recommended build order: 2 of 5. Scores express the survey recommendation, not an approved schedule.
- Touches: parser / domain / app-state / UI / watcher / docs.
- Non-goals: Approval controls, embedded external providers, remote artifact fetching, or editing review evidence.
- Coordinate with support-durable-gate-status for shared typed records; avoid duplicate parsing.

## Acceptance criteria

**AC-1:** Resolve the selected attempt's briefing.room-ref and display the canonical Briefing question and artifact/reference links. Support current index.json, retained briefing.json, and documented legacy bindings without requiring request.json for current rooms.

**AC-2:** Display recorded decision, actor, timestamp, reason, withdrawal, and application state with clear attribution. Missing files, invalid identities, or digest mismatches show diagnostics rather than verified evidence.

**AC-3:** Keep referenced-file reads within explicitly supported safe roots; reject traversal and symlink escapes. The viewer never records, approves, consumes, or repairs a gate.

**AC-4:** Provide predictable keyboard navigation and narrow-terminal rendering. Prove room-format handling, safe path resolution, refresh behavior, and rendered details at the lowest practical test layer.

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

### Target and source of truth

Deliver AC-1–AC-4 as a read-only gate-room browser. Task 079 is merged at `108a4f983b7d29b9d2b52d05ff24a8de3637c08b`; reuse `GateData`, `GateRecord`, `GateAttempt`, `GateResolution`, `GateWithdrawal`, `GateApplication`, `GateDetails`, and `gates::selected_attempt`. Room verification must not alter current-stage readiness or runtime activity. Historical attempts remain inspectable even when their stage is no longer current. Do not implement correction-round history (task 081), approval controls, external provider embedding, or remote fetching.

Pinned schema evidence: Spacedock tag `v0.27.3`, `docs/specs/gate-resolution-frontmatter-contract.md`, `internal/gates/operation.go::boundBriefingPath`, `boundBriefingBytes`, `canonicalPresentationItems`, `prepare.go::preparedRoomBinding`, and `io.go::validateRetainedAuthorityExcept`. Fixtures used below must be committed under `tests/fixtures/gate_rooms/` with tag/source provenance; the temporary survey clone is only planning evidence, never an implementation dependency.

### 1. Typed room facts and pure decoding

Own new `crates/spacetop-core/src/domain/gate_room.rs` and `parser/gate_room.rs`, registered through `domain/mod.rs` and `parser.rs`. Add typed room format, canonical Briefing, ordered Artifact/Reference inventory, item verification, and stable diagnostics; reuse task 079 gate records instead of parsing gates again. Pure byte decoders accept binding and selected gate/attempt context explicitly. References nested in context children are flattened in source order; preserve artifact summaries verbatim, reject duplicate item ids and invalid Reference summaries. Unsupported context kinds remain opaque, never interpreted by UI as authority.

Require Briefing type/version `Briefing`/`1`, nonblank question, nonempty artifacts, matching bound Briefing id, valid item id/URI/revision, and duplicate-free JSON. Check stage-qualified identities against the selected historical record, not blindly against current entity status. Show recorded decision, actor, timestamp, reason, conn quote/source, includes, withdrawal, and application target/state directly from the existing typed attempt. Included Annotation ids are recorded references only; resolving correction-round logs belongs to task 081. A conn citation is attribution evidence, not authentication of the quoted grant.

### 2. Room loading and format selection

Own new `crates/spacetop-core/src/gate_room.rs` as the side-effect boundary, with small helpers for path resolution, read, digest, and local Git object lookup. Resolve `room-ref` relative to the selected source entity file's parent; retain task 079/main gate provenance when a worktree body overrides an entity. Archive rooms use the archived entity's own path. Do not infer a binding from body text, a provider session, or a nearby similarly named directory.

- A nonempty `request-digest` selects request-backed legacy loading: validate `request.json` JCS digest, type/version, exact gate/attempt, Briefing id/digest, and captain actor/approver; follow only its clean room-relative locator. A missing or invalid request does not fall back to another format.
- A request-less directory with retained `briefing.json` selects that retained Briefing. Do not require request.json for retained rooms. Verify the selected Briefing id/digest and inventory.
- A request-less prepared directory selects `index.json` first, then `gate-briefing.json` only when the reserved file is absent. A malformed, unreadable, or mismatched index.json is a diagnostic, never permission to choose a valid older file. Missing both names is a missing canonical Briefing diagnostic; no room repair.
- A request-less regular-file binding reads that exact legacy Briefing file. Opaque provider refs and unsupported schemes are display-only diagnostics, never fetched or passed to an opener.

Use RFC 8785/JCS canonical JSON SHA-256 for Briefing and request digests, and raw SHA-256 for referenced bytes. Sorted serde_json output alone is insufficient: numbers and UTF-16 property order need JCS semantics. Before feature work, pin golden canonicalization vectors from upstream including reordered keys, Unicode, escapes, exponent/fraction numbers, invalid Unicode and duplicate members. Existing dependencies provide neither JCS nor SHA-256. Propose focused `sha2` and a proven JCS crate only after checking its RFC vectors and strict input handling; document the dependency rationale. Do not substitute sha1 or a hand-written serializer. A failing canonicalization spike is a blocker to verified-room labels, not a reason to weaken verification.

### 3. Explicit safe read boundary

Room files may be read only inside the workflow's canonical resolved entity/state root (including its archive); a single-root workflow uses the definition root. A state root that canonically escapes the definition boundary stays readable as existing workflow state but does not become an automatic safe root for gate evidence: report unverified-root and disable room/artifact reads there. Never widen the roots using HOME, cwd, arbitrary body paths, worktree paths, or URI contents.

Reject absolute paths, parent components, backslashes, NUL, unsupported schemes, and ambiguous encoded paths before I/O. Permit documented leading `./` in room-ref, but require clean request locators. Resolve canonical room and candidate under the declared safe root, require room-relative artifacts to stay inside that room, and require regular files. Refuse symlink components for evidence reads (including root, directory and final file), not just lexical traversal. Keep open/read validation and canonical containment together; handle path replacement races with a diagnostic rather than trust a previously checked pathname. No external editor or URL launch for rejected references.

For `git-root://main|state/<full-commit>/<canonically-escaped-path>`, use only the workflow's verified local definition/state Git history. Validate canonical URI encoding exactly once, full commit identity, clean repository path, regular blob mode (reject symlink/submodule/tree), object presence and raw revision digest. Read the exact local commit blob using read-only Git plumbing; never use current worktree bytes as fallback, fetch objects, repair checkout topology, or retain refs. Distinct state history is required for state URIs; linked/detached checkouts are readable when their history is verified, without implying sync eligibility. Reject mixed Git-root and local inventory as upstream does. Other URI schemes are visible unsupported links. Bound read size/nesting and report too-large/truncated evidence without a verified badge.

Typed diagnostics include missing room/file/object, unsupported format/root/scheme, unsafe path/symlink, invalid JSON/schema/identity, binding mismatch, request/Briefing/artifact digest mismatch, nonregular object, and size limit. Valid recorded frontmatter remains visible alongside a failing room diagnostic; label it recorded, with binding verification separate from person authentication. Unverified question/artifact content must never be presented as verified evidence.

### 4. Query, app and UI ownership

Add `WorkflowIndex`/`query.rs` APIs listing record/attempt identities and returning the selected attempt's typed room view; preserve source provenance through `sources.rs` and worktree merge code. Load room inventory for the selected attempt, and artifact bytes only on explicit item selection. Integrate reload invalidation rather than adding Git/filesystem reads inside Ratatui rendering. Keep expensive object reads out of the render loop using the existing app worker/channel pattern if measurements require it.

Own `crates/spacetop/src/app/gate_room.rs`, with wiring in `app.rs`, `app/keys.rs`, and `app/session.rs`; own `ui/gate_room.rs` plus routing in `ui/mod.rs`, help/footer and reserved-key tests. Proposed input: `B` opens a read-only gate browser for the selected entity, defaulting to task 079's current attempt; Tab changes focus between attempt history and item inventory, Up/Down or j/k moves the focused selection, Enter previews a supported verified item, PgUp/PgDn scrolls, `w` toggles preview wrapping, and Esc closes the item preview then browser. Keep q/global behavior consistent with existing overlay routing. Reserve B against configured bindings and preserve existing entity Enter, workflow arrows/P, archive a, and sync Y behavior outside the browser. Within the browser no key invokes a gate mutation or sync.

Attempt selection is keyed by stable gate/attempt ids, not indices. Reload preserves identity/focus/scroll when still present, clamps shortened content, and selects the current attempt with a visible diagnostic if the selected identity disappears. Entity/workflow switches close stale room views. At 40x12 show a stacked layout with wrapped question, selected attempt/decision and diagnostics; at 20x6 clip safely with scrollable content and visible Esc hint. At 80x24 use the dense split view. Never depend on horizontal truncation to communicate verification failure.

### 5. Refresh and docs

Own `crates/spacetop-core/src/watcher.rs` and watcher lifecycle in `crates/spacetop/src/lib.rs`. Extend relevant-event filtering for canonical room files (`index.json`, `gate-briefing.json`, `briefing.json`, `request.json`) and safely resolved local artifact dependencies; do not read paths merely because an event mentions them. Watch the actual supported entity/state root as well as the definition root when distinct, and retained archive room changes. Create/delete/rename/replacement and entity binding updates invalidate the selected room; a previously verified item must lose that state after mismatching bytes or a missing source. Git-object content is immutable; retry a previously missing local object on explicit refresh without fetching. Keep polling fallback behavior and debounce coherent; no watcher on rejected external roots.

Update README keyboard/format/safe-root/verification limitations, nearby gate docs, AGENTS.md code map and development-policy ownership if new modules require it, plus footer/help pinned strings. State clearly that recorded approvals remain read-only and identity/digest verification does not authenticate a human decision. Implement on an isolated feature branch/worktree from task 079 main; task entity/report stays in split-root state.

### Acceptance and falsifiable proof

| Criterion | Lowest layer and test scenario | Failure it must catch |
|---|---|---|
| AC-1 | Core loader fixtures: current one-file index room with no request; retained briefing.json; legacy gate-briefing/request-locator and exact-file binding; flat/folder/archive origins; ordered nested references | Requiring request.json for current rooms, selecting another attempt, dropping nested references, or reading a nearby basename changes result |
| AC-2 | Pure decoder/digest tests: JCS golden vectors and formatting invariance; tampered question/request/artifact; wrong gate/attempt/stage/Briefing identity; duplicate JSON/id; all approve/revise/hold, FO conn, withdrawal and application states | Wrong binding/digest becomes verified, conn becomes authenticated authority, or recorded fields are hidden by missing room |
| AC-3 | Filesystem and temporary-Git tests: traversal at room/locator/item/decoded-URI level; escaping directory/final symlink and path replacement; unsupported root; missing object; blob vs symlink/tree; dirty current artifact differing from selected commit | Any outside-root bytes are returned, worktree bytes substitute for committed evidence, unsafe refs open, or local reads cause workflow/Git writes |
| AC-4 | Query/app tests: open B, focus, historical attempt selection, preview/back, missing entity, archive/workflow switch, reload identity preservation/deletion. TestBackend at 80x24, 40x12, 20x6 with long Unicode question/reason/URI and diagnostics | Key conflicts change existing controls, reload shows stale verified evidence, narrow layout panics or loses the failure/exit hint |
| AC-4 refresh | Watcher unit tests for room JSON/artifact events, unrelated/editor/access exclusions and debounce; temp-root integration drives room create/change/delete and split-root replacement through reload | A supported changed dependency does not refresh, rejected external paths are watched, or a removed/tampered room retains verified state |

Read-only integration records exact workflow/room/artifact bytes and tree entries before and after successful and rejected browsing, and asserts equality; fake Git audit permits only read plumbing for this feature. Retain `no_write_git_calls`, `no_terminal_deps`, config/session root guardrails, and worktree gate-fact retention tests. Fixtures and temporary repos must be built in tests; no dependency on the survey clone or user configuration.

Before code completion run focused crate tests while implementing, then `cargo fmt --check`, `cargo test`, `make lint`, and `cargo test -- --ignored` for this watcher change. Capture command exit codes and assertion meaning in the implementation report; do not claim these future tests passed during planning. Optional terminal observation supplements TestBackend rather than replacing it. Acceptance requires all four rows plus refresh/read-only evidence, current docs, and zero clippy warnings.

## Stage Report: plan

- DONE: Produce a concrete implementation plan for canonical/current, retained, and legacy gate rooms, reusing task 079 typed gate records and naming owned core/app/UI/watcher modules.
  Implementation plan steps 1–5 name the existing task 079 facts, format precedence, new owned modules, provenance, and app/watch lifecycle integration.
- DONE: Define digest and identity attribution plus supported safe-root/path boundaries, diagnostics, and tests that reject traversal and symlink escapes without workflow writes.
  Steps 2–3 distinguish JCS/raw SHA-256, recorded/authenticated attribution, root/URI containment, symlink refusal, exact local Git blobs, diagnostic states and read-only proof.
- DONE: Map all four acceptance criteria to lowest-layer falsifiable tests, keyboard/narrow-terminal behavior, refresh checks, docs updates, cargo fmt, cargo test, and make lint.
  Acceptance table maps AC-1–AC-4 to fixtures, negative tests, app/TestBackend and watcher scenarios; final verification commands and doc owners are explicit.

### Summary

Planned a read-only gate-room browser using task 079's typed durable records and the pinned Spacedock v0.27.3 contract. The main implementation risk is correct JCS and safe evidence resolution; a failing digest spike or unsupported root must produce diagnostics rather than verified evidence. This stage changes only task 080 planning state; product implementation and its test gates remain pending.


## Implementation evaluation provenance

FO-owned JEV `jev-1.13.0` assessments are preserved below as response data, not test results or human authorization. Plan judgments were supported at confidence 1.0 / 0.97 / 1.0; the dependency spike was in scope at 0.99. FO reported cumulative approved spend $0.002680902 of $0.10 after these assessments; this worker made no paid model calls.

- Plan request SHA-256: `e2445a1ed480f47adaf886f6889c3984d426eca743e39f43428ab0a0600ad33a`; response original bytes SHA-256: `d5a8893b30f32aa995ddfd1be042d922240ae97f9847e68772e69c69a06148ba`.
- Dependency request SHA-256: `36d5ea42232fa17e148406fe7e63ee21fb11a157fc1975b31ec540497e52e67d`; response original bytes SHA-256: `ccd4dca0ffe272bb2af038b3d5b04b198e622058c8198b099aa6a78774ed5ffd`.
- Original request/response filenames: `/private/tmp/spacetop-080-jev-{plan,deps}-{request,response}.json`. The response values are retained here without changing the entity topology.

```json
{"plan_response":{"model":"jev-1.13.0","answers":{"linchpin_1":{"type":"choice","choice":"supported","confidence":1.0,"probabilities":{"supported":1.0,"insufficient_evidence":0.0,"missing_or_unsafe":0.0}},"linchpin_2":{"type":"choice","choice":"supported","confidence":0.97,"probabilities":{"supported":0.98,"insufficient_evidence":0.0,"missing_or_unsafe":0.02}},"linchpin_3":{"type":"choice","choice":"supported","confidence":1.0,"probabilities":{"supported":1.0,"insufficient_evidence":0.0,"missing_or_unsafe":0.0}}},"usage":{"input_tokens":4807,"output_tokens":147}},"dependency_response":{"model":"jev-1.13.0","answers":{"dependency_scope":{"type":"choice","choice":"in_scope_spike","confidence":0.99,"probabilities":{"needs_captain_decision":0.01,"insufficient_evidence":0.0,"in_scope_spike":0.99}}},"usage":{"input_tokens":596,"output_tokens":57}}}
```

## Stage Report: implement

- DONE: Implement typed read-only gate-room loading for current, retained and legacy bindings, with strict identity/digest verification, bounded safe roots and rejected traversal/symlink cases; reuse task 079 facts.
  Code `abc14cc2c76ab76644e1d752242b2d63259c9f1d`; domain/parser/core/index modules reuse existing GateRecord/GateAttempt and selected-attempt provenance rather than reparsing gate frontmatter.
- DONE: Deliver the gate briefing/recorded-decision browser with predictable keyboard navigation, historical attempts, artifact/reference visibility, narrow-terminal diagnostics and refresh invalidation.
  B/Tab/j/k/Enter/PgUp/PgDn/w/Esc browser; app tests reject Y/editor/scope/picker/switch mutations, retain attempt/focus across reload and remove invalid item previews. TestBackend covers 80x24, 40x12 and 20x6 including long Unicode content.
- DONE: Prove all four ACs with committed fixtures and lowest-layer tests, update docs, pass cargo fmt --check, cargo test, make lint and applicable real watcher checks; commit deliverables on the isolated branch and report reproducible evidence.
  From the assigned worktree: cargo fmt --check exit 0; cargo test exit 0 (683 passed); make lint exit 0 (zero warnings); cargo test -- --ignored exit 0 (4 real notify tests, outside sandbox). README, gate-room contract, code map and dependency policy updated in the same commit.

### Acceptance evidence

- AC-1: `formats_origins_history_and_order_do_not_require_request` exercises current/retained/request-backed/exact-file across flat/folder/archive origins and ordered nested References; requiring a current request or following current status rather than recorded stage fails it.
- AC-1 provenance/query: `worktree_body_never_rebases_main_gate_room_provenance` supplies a conflicting external worktree room and queries an unknown attempt; rebasing main bindings or selecting a nearby attempt fails it.
- AC-2: RFC8785 golden vectors, upstream v0.27.3 canonical digest, raw SHA-256 golden, strict duplicate/Unicode refusal and tampered Briefing/request fixtures reject false verification; replacing JCS with sorted JSON or trusting changed bytes fails them.
- AC-2 recorded fields: app tests show approve/revise/hold, actor/time/reason/conn/includes, withdrawal and pending/consumed/superseded application facts without a room; hiding attribution on missing evidence or treating conn as person authentication fails them.
- AC-3: traversal/encoded path/locator/symlink/root/size tests, deterministic final-file and directory replacement races, exact Git blob/mode/missing-object tests and wrong-branch/detached state history reject outside or fallback bytes; returning escaped/current-worktree bytes fails them.
- AC-3 read-only: entity/room bytes and tree-entry counts remain equal after browsing and rejection; fake Git audit allows only exact local rev-parse/ls-tree/cat-file calls. Existing no_write_git_calls, no_terminal_deps and config/session guardrails also pass.
- AC-4: app B/history/focus/preview/back/reload/deletion/reserved-key tests plus TestBackend details/failure/Unicode tests fail on changed controls, stale previews, clipped failure labels or lost Esc hints.
- AC-4 refresh: exact dependency/filter/access/debounce tests and real notify room/artifact create/change/delete/state replacement test fail on missed invalidation. One recursive contained root covers state/archive; event paths never authorize evidence reads.

### Verification boundary

The first real notify run timed out inside the sandbox, including all three pre-existing smoke tests; the unchanged suite and new refresh test passed outside the sandbox. Supported release targets are macOS/Linux (Unix); other platforms show an unsupported safe-read diagnostic. A bound Briefing badge does not verify inventory bytes until explicit selection, and recorded identity/digest checks do not authenticate a person. No approval, consumption, repair, fetch, editor/URL launch or gate-state write was added.

### Summary

Implemented and committed a read-only browser for canonical Briefings, recorded decisions and historical attempts with strict JCS/digest and safe-path/Git-object boundaries. All four acceptance criteria have reproducible lower-layer evidence, including read-only equality and real refresh checks; product code is committed only on the assigned isolated branch.

### State publication boundary

Code is committed at `abc14cc2c76ab76644e1d752242b2d63259c9f1d`; implementation report and preserved JEV response data were committed path-scoped at `bfc6cc5`. Automatic approval review rejected `git -C docs/spacetop-dev/.spacedock-state push origin spacedock-state/spacetop-dev`: it did not find trusted user authorization for publishing the internal report/evaluation data to that exact shared remote destination, and considered remote trust unverified. No bypass or retry was performed. Remote state publication remains pending explicit user approval; the First Officer was notified. This is a publication boundary, not an implementation/test failure.


## Stage Report: verify

- DONE: Independently assess AC-1 and AC-2 against abc14cc: upstream current/retained/legacy room formats, strict digest/identity semantics and complete recorded-decision attribution; challenge meaningful uncovered cases.
  Compared pinned v0.27.3 decoder/identity functions with independent Rust probes: valid null context is rejected and a plus-prefixed ordinal is falsely verified; AC-1/AC-2 require corrections below.
- DONE: Independently verify AC-3 safety and AC-4 usability/refresh: safe roots, traversal/symlink/race/Git-object boundaries, read-only proof, keyboard/history/reload behavior and narrow Unicode rendering; report material findings with exact evidence and proposed correction.
  Reviewed descriptor-relative NOFOLLOW reads, raw/JCS checks, exact blob plumbing, app identity/reload/keys and TestBackend/watch assertions; additional symlink-free escaped-state-root probe passes and registers no dependencies.
- DONE: Check required green formatting, full tests, lint and real watcher evidence, docs/policy/dependency fit; write a reproducible AC-by-AC verdict and checklist accounting without taking over implementation or approving the captain gate.
  Independently ran cargo fmt --check (exit 0); inspected retained test/lint/ignored/focus logs (683/0/4 full suite, zero clippy warnings, 4/0/0 real notify); no implementation, frontmatter, or paid-model edits/calls.

### Findings

- High — `crates/spacetop-core/src/parser/gate_room.rs:159`: `positive_suffix` uses Rust u64 parsing, which accepts a leading plus. Rebinding the fixture to `briefing:docs-dev:3k:validation:attempt-+1:revision-1` and recomputing its JCS digest produces a verified room; upstream `canonicalBriefingStage` rejects it. AC-2 explicitly requires invalid identities to remain diagnostic. Require ASCII `[1-9][0-9]*` syntax for both ordinals, a nonempty identity prefix, and regression tests; preserve upstream string semantics rather than imposing an undocumented numeric maximum.
- High — `crates/spacetop-core/src/parser/gate_room.rs:204`: `flatten` requires arrays even for explicit JSON null. With the fixture's `context` replaced by null and the bound digest recomputed, a valid upstream Briefing becomes `InvalidSchema: context children must be arrays` and its question disappears. Upstream `parseBriefingManifest` plus `canonicalPresentationItems` accept null context as an empty inventory. Treat absent/null context and children as empty arrays while rejecting other wrong types; pin top-level and nested-null compatibility tests.

### Reproducible evidence

- Independent Rust probes reused the committed `Setup` fixture without changing production code: null-context acceptance and plus-identity rejection assertions both fail; symlink-free external state-root rejection succeeds. Command: `cargo test -p spacetop-core --test gate_room_verify_probe verify_probe -- --nocapture`; exit 101, 1 passed / 2 failed.
- Null probe: deserialize `room/index.json`, set `v["context"] = Value::Null`, call `s.replace_manifest(v)`, then assert `s.view().verified()`. Identity probe: replace both `v["id"]` and `s.record.attempts[0].briefing.id` with the identity above, call `s.replace_manifest(v)`, then assert `!s.view().verified()`.
- Pinned v0.27.3 independent Go probe calls `canonicalBriefingStage` on that identity (false), then `parseBriefingManifest`/`canonicalPresentationItems` on the same null-context schema (success). `go test ./internal/gates -run TestVerifyProbe -v` passed; these are executable schema results rather than source-string assertions.
- Local evidence: `/private/tmp/spacetop-080-independent-probes.log`, `/private/tmp/spacetop-080-independent-probe-source.rs`, and `/private/tmp/spacetop-080-upstream-probes.log`. Temporary test files were removed from both trees; the code worktree stays clean. Local files supplement the exact mutations above and are not fresh-setup dependencies.

### AC-by-AC verdict

- AC-1: FAIL — current/retained/request-backed/exact-file precedence, nested Reference ordering, flat/folder/archive origins and main-worktree provenance are exercised; null context is an uncovered upstream-compatible case that loses the canonical question/inventory.
- AC-2: FAIL — JCS numeric/UTF-16 vectors, strict duplicate/Unicode rejection, tampered request/Briefing/artifact checks and recorded actor/time/reason/conn/includes/withdrawal/application coverage are sound; malformed plus identity still earns verified evidence.
- AC-3: PASS for reviewed boundaries — unsafe local/encoded paths, symlinks, replacement races, size limits, exact local Git blob modes/commits and read-only equality have falsifiable tests; the independent escaped-root probe prevents evidence reads and dependencies even without a symlink. This does not claim person authentication.
- AC-4: PASS for covered behavior — stable history/focus/reload, stale preview invalidation, reserved B and inert mutation keys plus 80x24/40x12/20x6 Unicode/failure/Esc rendering assertions; room/artifact create/change/delete/state replacement has real notify evidence. No render-time I/O or new gate writes were found.

### Open Questions

No product decision is required for the two corrections: both are pinned upstream compatibility requirements. Full-suite and real notify results are retained implementation evidence, not reruns by this verifier; the sandbox-failing watcher run was not mistaken for behavioral failure. Dependency rationale and README/code-map/gate-browser docs match the owned changes.

### Proposed correction routing

Both findings are Material: the ordinal issue breaks AC-2 invalid-identity verification, and null inventory handling breaks AC-1 upstream compatibility. Assign both to the task 080 implementation owner on the existing feature worktree; add the failing lowest-layer cases first, keep captain criteria unchanged, then return to independent verify. These are reviewer proposals; FO owns semantic disposition and workflow routing.

### Judgment

Request changes. Return the two proven parser defects to implementation, then independently verify their new regression tests; this report does not approve or advance the captain gate.

### Summary

Verification ruled out missing green build/lint/watch evidence and found two concrete schema-compatibility gaps in `abc14cc2c76ab76644e1d752242b2d63259c9f1d`. The decision changes from gate-ready to correction-required; implementation quality and completed checks remain separate from these failed behavior assertions.


## Stage Report: implement (cycle 1)

- DONE: Implement typed read-only gate-room loading for current, retained and legacy bindings, with strict identity/digest verification, bounded safe roots and rejected traversal/symlink cases; reuse task 079 facts.
  Correction commit `0efb0e5e78b4a03d93f5f419c25c18f7e00a5ad5` fixes only the two dispatched parser defects; existing loader, safe reads and task 079 ownership remain covered by the green suite.
- DONE: Deliver the gate briefing/recorded-decision browser with predictable keyboard navigation, historical attempts, artifact/reference visibility, narrow-terminal diagnostics and refresh invalidation.
  Existing browser/app/TestBackend/refresh tests pass in the full rerun; this cycle changes no UI/input/watcher behavior or gate authority.
- DONE: Prove all four ACs with committed fixtures and lowest-layer tests, update docs, pass cargo fmt --check, cargo test, make lint and applicable real watcher checks; commit deliverables on the isolated branch and report reproducible evidence.
  Worktree: cargo fmt --check exit 0; cargo test exit 0 (685 passed, 4 ignored); make lint exit 0 (zero warnings); gate_rooms focused exit 0 (11 passed, 1 ignored). Docs specify ordinal and null compatibility.
- SKIPPED: Rerun real notify checks for this correction.
  Dispatch requires rerun only for watcher changes or unresolved watcher concern; neither applies. Original 4 passing real notify checks remain prior evidence, not a new run.

### Feedback Cycles

- verify/1 identity: Material / Fix, FO-authorized. Canonical ASCII `[1-9][0-9]*` replaces integer parsing; nonempty identity prefix is required; no numeric maximum is added. No AC change.
- verify/1 null inventory: Material / Fix, FO-authorized. Null/absent context and nested children are empty; object/string/bool/number remain InvalidSchema. No AC change.
- Red-first proof: `cargo test -p spacetop-core --test gate_rooms -- --nocapture` failed 2 new regressions against the rejected parser (9 pass / 2 fail / 1 ignored); corrected run passes both. Restoring integer parsing or array-only null refusal fails them.
- AC-1/AC-2: `absent_and_null_context_or_children_are_empty_but_wrong_types_fail` retains the question and single Artifact for empty context; `stage_identity_requires_canonical_unbounded_ascii_ordinals_and_prefix` rejects malformed bindings and verifies large canonical strings. Both reuse the committed upstream fixture; no machine-only test source.
- AC-3/AC-4: full rerun retains safe-path/Git-blob/read-only, historical browser, reload and narrow-render coverage; removing refusal or retaining stale evidence breaks their existing assertions.
- Advisory correction staging inputs: `review/verify/correction-1-inputs/briefing.json` binds original/corrected parser and corrected regression raw SHA-256 Git artifacts; `review/verify/correction-1-inputs/briefing.review.jsonl` preserves the original reviewer revise as an explicitly labeled report projection, FO Material/Fix dispositions, and ensign closing entries. Closing decision stays revise pending independent verification; no gate approval or frontmatter mutation. These byte-preserved inputs are staged outside the recorder-owned immutable `review/verify/round-1` output; FO records verify/1 exactly once from these inputs.
- Actual JEV `jev-1.13.0` response data is retained in the Briefing provenance. Request SHA-256: `ebf8dcf7f3d0cb96e191738d6bad0ba99d58413265b79cd902c33577249fd0c9`; original response SHA-256: `1dd676396973a3452fd98d3c5fa8efa0acd067f04bdc64b54b91e45ca2b50fc3`. No raw input snapshots or paid model calls.
- Reproduction: use committed tests from the code worktree; supplementary command outputs are `/private/tmp/spacetop-080-correction-{red,focused,fmt,test,lint}.log` and are not runtime dependencies.

### Summary

Corrected the two upstream compatibility defects within the authorized parser scope and committed the regression proof and nearby docs. All required formatting, full tests and lint pass; original rejection and reports remain intact, and independent verify/1 is still the First Officer's next step.


## Stage Report: verify (cycle 1)

- DONE: Independently assess AC-1 and AC-2 against abc14cc: upstream current/retained/legacy room formats, strict digest/identity semantics and complete recorded-decision attribution; challenge meaningful uncovered cases.
  Re-reviewed correction `0efb0e5e78b4a03d93f5f419c25c18f7e00a5ad5` atop abc14cc: original null-context/plus-ordinal probes now pass, but a new prefix-newline probe still verifies an upstream-invalid identity.
- DONE: Independently verify AC-3 safety and AC-4 usability/refresh: safe roots, traversal/symlink/race/Git-object boundaries, read-only proof, keyboard/history/reload behavior and narrow Unicode rendering; report material findings with exact evidence and proposed correction.
  The correction changes only parser predicates, core tests and nearby docs; original escaped-state-root probe remains green. No new I/O, key, watcher, mutation, or rendering boundary is introduced.
- DONE: Check required green formatting, full tests, lint and real watcher evidence, docs/policy/dependency fit; write a reproducible AC-by-AC verdict and checklist accounting without taking over implementation or approving the captain gate.
  Inspected correction logs: red 9/2/1, focused 11/0/1, full 685/0/4 and make lint zero warnings; fmt log empty with reported exit 0. Prior 4 real notify checks remain valid because watcher code is unchanged.

### Findings

- High — `crates/spacetop-core/src/parser/gate_room.rs:155`: the new nonempty-prefix guard still accepts a newline in the prefix, unlike pinned upstream `^briefing:(.+):([^:]+):attempt-[1-9][0-9]*:revision-[1-9][0-9]*$` where Go's dot excludes newline. Binding fixture id and attempt briefing id to the Rust string `"briefing:docs\ndev:3k:validation:attempt-1:revision-1"`, then recomputing its correct JCS digest, produces `GateRoomView::verified() == true`; upstream `canonicalBriefingStage` returns false. This leaves AC-2's invalid-identity diagnostic requirement unsatisfied. Reject newline in the identity prefix, preserve the already-correct ordinal/stage semantics, and add this exact loader regression.

### Independent proof and closed findings

- Original independent probes ran unmodified against correction 0efb0e5: null context accepted, plus-prefixed ordinal rejected, symlink-free escaped state-root rejected; `cargo test -p spacetop-core --test gate_room_verify_probe verify_probe -- --nocapture` exit 0, 3 passed. Both original reported defects are fixed, rather than restated as new failures.
- Added prefix-newline assertion to the same fixture probes: set the two bound ids to the string above, call `s.replace_manifest(v)` to recompute the bound JCS digest, then assert `!s.view().verified()`. The same command exits 101 with 3 passed / 1 failed; actual output confirms a verified Current room with no diagnostics.
- Pinned v0.27.3 executable contrast: `GOCACHE=/private/tmp/spacetop-080-go-cache go test ./internal/gates -run TestVerifyProbeCycle1 -v` exits 0; direct `canonicalBriefingStage` assertions reject both plus ordinal and newline prefix. The cache path is only an execution-environment workaround, not a product/test dependency.
- Local outputs: `/private/tmp/spacetop-080-independent-cycle1.log`, `/private/tmp/spacetop-080-independent-cycle1-boundary.log`, `/private/tmp/spacetop-080-upstream-cycle1-boundary.log`; supplementary reproducer `/private/tmp/spacetop-080-independent-cycle1-probe-source.rs`. Exact mutations above permit reproduction using the committed Setup on a fresh checkout. Temporary tests were removed and the code worktree is clean.
- Committed regressions cover both ordinal positions, empty/zero/leading-zero/non-ASCII/sign syntax and large canonical strings, plus absent/null context/children and wrong-type rejection. Restoring integer parsing or array-only handling breaks their observed red-first assertions. Docs now specify those boundaries.

### AC-by-AC verdict

- AC-1: PASS for the reviewed room formats/provenance and empty/null inventory semantics; the original current/retained/legacy/history/origin coverage remains unchanged and the null-context question now survives.
- AC-2: FAIL — both previous defects are closed, but the residual prefix-newline identity is falsely verified. Existing digest/recorded-decision attribution checks remain green.
- AC-3: PASS for reviewed boundaries — no safe-read/Git behavior changed, original guardrails and read-only assertions remain green, and the independent escaped-root case passes.
- AC-4: PASS for covered usability/refresh — no UI/input/watcher behavior changed; full app/TestBackend suite is green, prior real notify evidence applies. No duplicate full-suite or real watcher rerun was warranted for this parser-only correction.

### Proposed correction routing and judgment

Proposed Material / Fix: the residual prefix guard violates the same declared AC-2 compatibility boundary as the original identity finding. Assign the narrow guard and regression to the existing task 080 implementation owner; no product fork or AC narrowing is required. Request changes; FO owns JEV disposition/routing and the captain gate remains unapproved.

### Summary

Correction 0efb0e5 fixes both previously reported cases and supplies meaningful red-first regression proof with green full-suite/lint evidence. Independent re-review found one remaining exact upstream identity mismatch; verification is correction-required for that bounded case, with AC-1/AC-3/AC-4 evidence retained.


## Stage Report: implement (cycle 2)

- DONE: Implement typed read-only gate-room loading for current, retained and legacy bindings, with strict identity/digest verification, bounded safe roots and rejected traversal/symlink cases; reuse task 079 facts.
  Commit `4368d4fe6576976a05f2b96fd70f8290ce19caef` adds only a newline-prefix guard matching upstream Go dot semantics; prior ordinal/stage/null behavior and safe-read ownership remain unchanged.
- DONE: Deliver the gate briefing/recorded-decision browser with predictable keyboard navigation, historical attempts, artifact/reference visibility, narrow-terminal diagnostics and refresh invalidation.
  Full suite rerun retains existing app/TestBackend/history/reload evidence; this parser-only correction changes no browser input, UI, watcher or mutation behavior.
- DONE: Prove all four ACs with committed fixtures and lowest-layer tests, update docs, pass cargo fmt --check, cargo test, make lint and applicable real watcher checks; commit deliverables on the isolated branch and report reproducible evidence.
  Worktree commands: cargo fmt --check exit 0; cargo test exit 0 (686 passed, 4 ignored); make lint exit 0 (zero warnings); gate_rooms focused exit 0 (12 passed, 1 ignored). Nearby docs now state the prefix newline boundary.
- SKIPPED: Rerun real notify checks for this correction.
  No watcher change or new watcher concern; dispatch expressly limits rerun to those triggers. Original 4 passing real notify checks remain prior evidence, not a new run.

### Feedback Cycles

- verify/2 newline prefix: Material / Fix, FO-authorized; declared AC-2 requires upstream-invalid identities to remain diagnostic. Reject only LF within the prefix; retain nonempty prefix, stage and unbounded ASCII ordinal semantics. No AC narrowing or product decision.
- Red-first command: `cargo test -p spacetop-core --test gate_rooms identity_prefix_newline -- --nocapture` exit 101 (0 passed / 1 failed); the exact LF-prefixed identity produced verified Current evidence on rejected `0efb0e5`. Corrected focused/full runs pass.
- AC-2 regression: `identity_prefix_newline_is_rejected_without_broadening_control_restrictions` checks LF in several prefix positions and requires IdentityMismatch; reverting the guard fails it. CR, tab and U+2028 remain accepted, so broadening the refusal beyond upstream dot semantics also fails it. Existing invalid ordinal/large ordinal/null tests remain green.
- AC-1/AC-3/AC-4: full tests retain format/provenance, safe root/Git blob/read-only, historical input/reload and narrow-render assertions; no new I/O or UI boundary introduced.
- Two-file staging inputs: `review/verify/correction-2-inputs/briefing.json` binds rejected/corrected parser and corrected regression raw SHA-256 Git artifacts; `review/verify/correction-2-inputs/briefing.review.jsonl` retains original reviewer revise as an explicit report projection, FO Material/Fix and closing ensign response. Closing decision stays revise pending independent verification; FO creates recorder-owned immutable `round-2` exactly once.
- Actual JEV response is retained in input Briefing provenance: `jev-1.13.0`, material_fix confidence 1.0. Request SHA-256: `15fd0ac2f477ad7f1b2295ab3b724caa304b64fbaf4757082b81c55a0b2b5c56`; original response SHA-256: `e5430d63ee7e35cc46c8c1e712563f128c190cf613d43e361667412d118669c5`. No raw input snapshots or paid-model calls.
- Supplementary command logs: `/private/tmp/spacetop-080-cycle2-{red,focused,fmt,test,lint}.log`; committed regression and fixture are sufficient for fresh-setup reproduction. Immutable round-1 and entity frontmatter are untouched; previous reports remain intact.

### Summary

Corrected the remaining upstream newline-prefix mismatch within the authorized parser scope, with red-first loader proof and green formatting/full-suite/lint gates. Code is committed on the isolated branch; correction inputs are staged outside recorder-owned output for FO recording and independent verify/2.


## Stage Report: verify (cycle 2)

- DONE: Independently assess AC-1 and AC-2 against abc14cc: upstream current/retained/legacy room formats, strict digest/identity semantics and complete recorded-decision attribution; challenge meaningful uncovered cases.
  Verified final `4368d4fe6576976a05f2b96fd70f8290ce19caef` atop abc14cc/0efb0e5; all original independent cases now pass: null context, plus ordinal, newline prefix and escaped state root. All three previously reported defects are closed.
- DONE: Independently verify AC-3 safety and AC-4 usability/refresh: safe roots, traversal/symlink/race/Git-object boundaries, read-only proof, keyboard/history/reload behavior and narrow Unicode rendering; report material findings with exact evidence and proposed correction.
  Final correction adds only the upstream LF-prefix guard and loader regression/docs; no safe-read/Git, app/UI or watcher change. Prior lowest-layer safety, read-only, history/reload and narrow rendering evidence remains applicable; fresh escaped-root probe passes.
- DONE: Check required green formatting, full tests, lint and real watcher evidence, docs/policy/dependency fit; write a reproducible AC-by-AC verdict and checklist accounting without taking over implementation or approving the captain gate.
  Fresh cargo fmt --check exits 0; retained cycle-2 focused/full/lint logs show 12/0/1, 686/0/4 and zero warnings. Prior 4 real notify passes apply to unchanged watcher code; no paid-model call, product-code edit or gate approval.

### Findings

No new material findings. The LF guard matches upstream Go dot semantics while preserving CR/tab/U+2028, exact recorded stage and unbounded ASCII ordinals; the explicit null inventory handling keeps wrong types diagnostic. The previous rejection reports and immutable advisory rounds remain intact.

### Independent and falsifiable proof

- Ran the unchanged verifier source from the prior rejection against final code: `cargo test -p spacetop-core --test gate_room_verify_probe verify_probe -- --nocapture` exits 0, 4 passed / 0 failed. Null context retains a canonical question, both malformed plus/newline identities lose verification, and a symlink-free escaped root refuses reads/dependencies. Reverting either correction makes its original assertions fail again.
- Exact final result is retained at `/private/tmp/spacetop-080-independent-cycle2.log`; prior source at `/private/tmp/spacetop-080-independent-cycle1-probe-source.rs` and pinned upstream contrast at `/private/tmp/spacetop-080-upstream-cycle1-boundary.log`. Prior reports specify exact mutations using the committed Setup, so these supplementary local artifacts are not fresh-setup dependencies.
- Reviewed cycle-2 committed `identity_prefix_newline_is_rejected_without_broadening_control_restrictions`: multiple LF prefix placements require IdentityMismatch; CR/tab/U+2028 must remain verified. Red log observes the exact pre-fix failure (0/1/0), then focused final loader suite passes (12/0/1); restoring or broadening the guard would fail this proof.
- Reviewed retained `/private/tmp/spacetop-080-cycle2-{red,focused,fmt,test,lint}.log`: full 686 passing tests, 4 intentional real-notify ignores, completed clippy with `-D warnings`. Full suite/lint and original `/private/tmp/spacetop-080-ignored.log` real-notify results are author-run evidence, not claimed verifier reruns.
- Removed the temporary Rust test after execution; fresh formatting passes and code worktree is clean. Only this report is written/committed; recorder-owned round-1 (8 entries) and round-2 (5 entries) retain revise provenance without reviewer append or approval.

### AC-by-AC verdict

- AC-1: PASS — current index without request, retained Briefing, request-backed/exact-file legacy precedence, historical selected stage, ordered nested references and flat/folder/archive/main provenance have parser/query proof; corrected null context preserves the question and artifact inventory.
- AC-2: PASS — strict JCS/UTF-16/numeric/duplicate/Unicode vectors, digest and exact binding negatives, canonical identity syntax including original independent plus/LF cases, and complete recorded actor/time/reason/conn/includes/withdrawal/application coverage. Recorded attribution remains distinct from person authentication.
- AC-3: PASS for declared boundaries — safe-root/traversal/symlink/replacement/size negatives, exact local Git commit/blob/mode/no-fallback checks, workflow byte/tree equality and static write/config/core guardrails. Independent external-root rejection passes; unsupported platforms/roots remain diagnostic.
- AC-4: PASS for covered behavior — browser/history/focus/back/reserved/inert keys, identity-preserving reload and preview invalidation, 80x24/40x12/20x6 Unicode/failure/Esc assertions plus canonical room/artifact/state replacement real-notify proof. No watcher change or unresolved refresh concern warrants rerunning the unchanged real suite.

### Actual FO JEV provenance

Retained original-byte `jev-1.13.0` responses and cost ledger under `evidence/jev/`; `provenance.json` records request/response SHA-256, model and exact source refs without raw input snapshots. Core AC-2 initially returned insufficient confidence 0.24; added actual app/UI/parser evidence yielded final AC-2 satisfied 1.0. Actual other judgments: AC-1 satisfied 0.93, AC-3 satisfied 0.91, AC-4 satisfied 0.73. Cumulative actual spend is $0.004364682 of the approved $0.10 budget for tasks 079–083; these typed judgments supplement independent probes and do not grant captain approval.

### Judgment

Approve the implementation's independent verification. All four ACs have reviewed falsifiable evidence and no blocking finding remains; FO owns its final JEV cross-check, captain-gate presentation and workflow transitions. This is not captain approval, gate recording, PR merge or release authorization.

### Summary

Final correction 4368d4f closes the residual upstream identity mismatch, and all previously failed independent probes now pass. Formatting, full tests and required lint are green with applicable real watcher evidence; task 080 is ready for the captain's review gate.
