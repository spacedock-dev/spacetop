---
id: 080
title: Browse gate Briefings and decisions
status: plan
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Briefing parser/path-boundary fixtures, app navigation tests, watcher refresh checks, and TestBackend assertions; wrong bindings and escaped refs must be rejected."
started: 2026-10-01T18:17:27Z
completed:
verdict:
score: 0.90
worktree:
issue:
pr:
mod-block:
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
