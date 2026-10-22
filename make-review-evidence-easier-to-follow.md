---
title: Make review evidence easier to follow
status: verify
source: "User-requested product improvement brief, 2026-10-22"
kind: feature
risk: medium
milestone: usability
proof: "Behavior scenarios and a live terminal walkthrough; see acceptance criteria and proof plan."
started: 2026-10-22T14:47:06Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-make-review-evidence-easier-to-follow
issue:
pr:
mod-block:
id: 088
---

Help reviewers understand the question, read the main report, and follow the supporting evidence without searching through files.

## Scope

- Kind: feature
- Risk: medium
- Milestone: usability
- Touches: app-state / UI / docs
- Put the review question first and make its main report directly accessible in the gate browser.
- Explain missing or invalid evidence in plain language.
- Connect requested corrections to recorded later responses using task 081, inspect-correction-round-history, as the owner of correction-history parsing and typed records.
- Integration dependency: task 081 supplies correction-round facts. Do not duplicate its scanner or invent response links from prose.
- Non-goals: Approval controls, correction recording, worker routing, relaxed evidence checks, network fetching, or authenticated-person claims.

## Acceptance criteria

**AC-1 -- The review question and main report are easy to reach.**
The browser shows the selected review question first and provides a direct route to its main report and supporting items. Missing question or report data is explicit.
Verified by: App navigation scenarios and a terminal walkthrough with complete and incomplete review rooms.

**AC-2 -- Evidence failures explain their effect.**
Missing, invalid, or mismatched evidence has a clear explanation. Recorded decisions remain inspectable without falsely presenting failed evidence as verified.
Verified by: Existing safe evidence fixtures plus navigation scenarios for missing and tampered items.

**AC-3 -- Corrections lead to recorded responses.**
Reviewers can navigate from a requested correction to its recorded response and identify unresolved corrections. Links use the typed relationships owned by task 081; absent relationships remain unknown.
Verified by: Multi-round scenarios with linked responses, an unresolved correction, and an unrelated response that must not be linked.

**AC-4 -- Existing evidence safety stays intact.**
Navigation preserves path, size, digest, and exact Git revision boundaries. It neither writes workflow state nor treats recorded approval as permission to execute an action.
Verified by: Existing gate-room safety checks and a read-only terminal walkthrough.

## Proof plan

- Lowest test layer: Typed domain/query and app-state behavior, with rendering checks where presentation affects interpretation.
- Required commands for implementation: cargo fmt, cargo test, and make lint. Use existing safety checks for the affected surfaces.
- Manual check: Run the actual terminal UI with representative normal, missing-data, and failure cases. Record evidence for each acceptance criterion.
- Docs update: Update README and relevant user guidance for the delivered behavior. Keep workflow files read-only during product use.

## Implementation plan

### Readiness and dependency hold

Planning is complete; full implementation is **blocked by task 081, inspect-correction-round-history**, which is still at `shape`. AC-3 remains required. The steps below distinguish work possible with current facts from integration that must wait; delivering only AC-1/2/4 is not completion of 088.

Current source has `GateAttempt` and `GateResolution` with actor/time/decision/reason and `includes: Vec<String>` (`crates/spacetop-core/src/domain/gates.rs:44-88`). These are durable gate records, not correction-round records. `Entity` has no review-round field (`domain/mod.rs:275-303`); `CanonicalBriefing` has question/items only (`domain/gate_room.rs:54-71`). `WorkflowIndex::gate_attempts`, `gate_room`, and `gate_room_item` exist (`index.rs:401-456`), but no correction-round scanner, typed Annotation log, current/retained round projection, or correction-to-response relationship query exists in the inspected core/app/UI source. The browser prints includes as recorded reference IDs (`app/gate_room.rs:284-288`); `docs/gate-room-browser.md:29-31` explicitly excludes correction-log resolution.

Unblock contract, owned by 081: land the actual typed round/Annotation/Resolution records and query surface, with stable entity/stage/round/record identities, current-versus-retained provenance, recorded relationships and missing/malformed/partial/stale evidence states. Its AC-3 promises includes relationships, not a license to interpret every includes edge as a later response: demonstrate the recorded relationship semantics that permit correction-to-response navigation with a real multi-round fixture. If 081 exposes only opaque IDs or no authoritative response relationship, retain this blocker and coordinate its contract with the owner; do not manufacture links from reason text, matching filenames, round adjacency, timestamps, or an approval decision. No proposed API below is asserted to exist.

Dependency coordination clarification (081 planner, 2026-10-22): its upstream advisory-round investigation reports same-Briefing backward `includes` edges from a later disposition Annotation to the finding, then from the closing Resolution to included records; `parseReviewLog` rejects cross-round includes. Accordingly, “later response” below means a later recorded disposition in the same Briefing, reached through 081's typed inverse edges, not an inferred link into a later round. Multi-round coverage proves isolation as well as navigation: similarly worded records and attempted cross-round references must not link. Keep 081's round-level answered state separate from any individual finding's response/completion state. This is a coordinated dependency contract, not a claim that the APIs have landed.

081 also owns the distinct round-evidence URI boundary: its fixture includes `../../../candidate.patch` that normalizes within the owning entity. Consume its eventual bounded, verified reader for those records; do not weaken gate-room `clean_relative`, bypass safe reads, or route round-relative URIs directly through the gate inventory reader. Add an integration case for a valid contained round reference and a rejected escaping one once 081 supplies that boundary.

### Current surfaces and gaps

| Surface | Current implementation | Planned change |
| --- | --- | --- |
| Selected question | `app/gate_room.rs:251-264` places diagnostics/notices before the question; a failed Briefing has no question text. | Put the selected question or an explicit unavailable-question explanation first; keep evidence trust status separately visible. Never display unverified JSON as the question. |
| Main report and supporting evidence | `RoomItemKind` is Artifact/Reference; the decoder preserves artifacts then nested References (`parser/gate_room.rs:125-149`). App focus is Attempts/Items; Tab then Enter opens any selected item. There is no typed main-report designation. | Add a small pure core selection result for the canonical report, surfaced through the index; add a direct report action and keep the complete inventory reachable. |
| Report identity evidence | Both committed `gate_rooms/index.json` and unchanged `upstream-briefing.json` identify the report as `artifact:gate-review`; neither defines a general primary-report field. | Use this explicit Artifact identity as the documented canonical report convention, not first-item order, filename, summary prose, or extension. An otherwise valid room without this designation says “Main report not identified”; its other items stay browseable. |
| Failure and decision | `RoomDiagnostic` carries typed kind/message but Display prints enum names. `GateRoomView::verified()` verifies the Briefing, not every artifact. Recorded decisions are rendered independently. | Explain cause and consequence in plain language while retaining exact diagnostic detail and independently inspectable recorded facts. |
| Evidence reads | `gate_room::read_item` revalidates the bound Briefing and hashes selected bytes; app refresh invalidates preview; UI rendering does no I/O. | Route every new report/response open through the owning verified core reader, preserving revalidation and selection identity. |
| Corrections | Includes IDs are display-only; no round navigation exists. | Consume 081's real typed facts/query surface after the dependency lands; no second scanner in 088. |

### Ordered implementation and ownership

1. **Executable now — typed presentation inputs.** In `crates/spacetop-core/src/domain/gate_room.rs` and `index.rs`, add the minimal typed main-report selection result over the verified inventory (canonical Artifact ID versus absent designation versus unavailable Briefing). Keep the selection pure and return an item identity/reference, not copied evidence bytes. Keep `parser/gate_room.rs` strict: a blank question, missing artifacts, invalid identity, or digest mismatch still invalidates the room. Do not introduce a new upstream schema field. Add selector/query tests including reordered artifacts, a different first artifact, a Reference with the report-like ID, and missing designation.
2. **Executable now — question-first navigation.** Extend `crates/spacetop/src/app/gate_room.rs` with a browser-local `r` action to open the selected attempt's canonical report by identity through `gate_room_item`; retain B, Tab, j/k/arrows, Enter, PgUp/PgDn, w, Esc/q behavior. Keep question/context visible when entering a report, and return to the same attempt/focus/item on Back. Missing report designation and unreadable report bytes are different states. Preserve selection by ID on refresh/reorder; clear stale preview/notice appropriately when changing attempts or when an entity/target disappears. No auto-reading every artifact and no I/O in content rendering.
3. **Executable now — failure explanations and layout.** Use exhaustive typed `RoomProblem` matching for browser-facing copy, not parsing diagnostic strings. Missing room means question/inventory cannot be verified; missing item means the Briefing may still be bound but this item cannot be read; invalid JSON/schema and identity/request/Briefing binding mismatches reject room trust; artifact digest mismatch rejects the selected bytes; unsafe path/symlink/nonregular/replacement/unsupported-root failures mean safe reading was refused; size limits mean content was not read as verified; unsupported schemes are display-only; missing Git objects cannot fall back to current files. Non-UTF-8 verified bytes are not a readable text report. Preserve technical details below the explanation. In `ui/gate_room.rs`, separate “Briefing binding verified” from selected-byte verification, keep recorded decisions visible on room failures, and make the question/absence first in the content area at narrow and wide widths.
4. **Dependency-held — correction response integration, all of AC-3.** After 081 lands, inspect its actual APIs and reuse its ordered round queries, identity and diagnostics without adding JSONL parsing or folder discovery. Add a typed correction focus/detail route in app state: select a requested correction, show its recorded provenance, list only authoritative recorded response targets, Enter follows the selected target, and Esc returns to the originating correction. Keep linked response(s), explicitly unresolved correction, and unknown/unavailable relationship distinct; no “resolved” label merely because a response exists. If a relationship points to a missing target, show the recorded link and unavailable target without substituting a nearby response. Preserve source/target selection on refresh and expose historical stage/round identity. Read target evidence through 081's verified reader or the existing gate-room reader where its typed binding applies; never coerce a round into a fabricated gate attempt.
5. **Executable in part now; final coverage held — tests and fixtures.** Extend existing app/unit/Ratatui tests and `crates/spacetop-core/tests/gate_rooms.rs`; add a self-contained `tests/fixtures/review-evidence` workflow and provenance/instructions using committed local report bytes. Reuse existing room fixtures and temporary Git setup, not a private upstream checkout. Add correction records only from 081's actual contract. Include complete/incomplete rooms, no canonical report designation, missing and tampered report, a tampered Briefing, unsupported reference, and multi-stage/multi-round linked/unresolved/unrelated-response cases. Record expected question/report/relationship/diagnostic outcomes for each fixture.
6. **Executable after behavior lands — docs and integration.** Update README browser guidance around lines 109 and 345 and `docs/gate-room-browser.md`, including the direct report key, canonical report convention, honest trust labels, missing/unknown relationships and response/back navigation. Replace the current correction-resolution exclusion only after AC-3 works. Update browser footer/help strings and their app/UI expectations together; change global key registration/help only if the existing ownership requires it. Preserve `lib.rs` watcher invalidation integration, and reuse 081's refresh dependencies for correction files rather than introducing a competing watcher. No new crates, approval controls, network fetching, worker routing, or workflow writes.

### Acceptance evidence and falsification

| AC | Lowest-layer proof to add/run during implementation | Required actual terminal observation |
| --- | --- | --- |
| AC-1 | Core selector tests prove report identity does not depend on order or a reference's name. App tests start at the selected attempt, open its report directly, visit supporting items and return to the same context. Blank/missing question, absent report designation and absent report file are explicit. TestBackend checks first visible question/absence at 80x24, 40x12 and 20x6, plus long Unicode. Fails if navigation opens the first artifact instead of the canonical report or diagnostics hide the question. | Complete room: B shows question; r opens the bound report; Esc, Tab, selection and Enter reach supporting evidence. Incomplete rooms: question/report absence is understandable with decisions still reachable. |
| AC-2 | Extend `recorded_fields_remain_visible_without_room_and_cover_decisions_application_states` and `reload_preserves_identity_focus_and_invalidates_preview`. Cover missing/tampered item and tampered room separately, all typed problem explanations, and non-UTF-8 text limitations. Fails if failed bytes retain a verified preview/banner, a missing item invalidates unrelated recorded decisions, or a verified Briefing is presented as verification of every item. | Open missing and tampered fixtures and inspect explanation, effect, exact detail and recorded decision. Replace a report in a separate refresh run; the open preview disappears and the failure is visible. |
| AC-3 | Consume 081 fixture/query tests, then app/Ratatui integration over at least two stages and three rounds: one correction with an explicit later response, one without a recorded response, an unrelated response with similar prose, an unknown relationship and a dangling target. Test source-to-target-to-source navigation, multiple targets, stable refresh and current/retained labels. Fails if unrelated/adjacent records become linked, absent evidence is called resolved, or Back loses the source correction. | Select a correction, follow only its recorded target, inspect response provenance, and return. Show unresolved/unknown cases and the unrelated response without a false link. This observation cannot be claimed until 081 supplies the records/relationships. |
| AC-4 | Retain core format/origin, precedence/binding, path/scheme/size/digest, symlink, replacement-race and exact-Git tests. Exercise new shortcuts and correction navigation through those same boundaries. Keep `b_navigation_preview_back_and_mutation_keys_are_inert`, `fake_git_audit_uses_only_exact_local_read_plumbing`, and repository no-write/core-dependency guardrails. Fails if a shortcut bypasses reverification, reads current dirty bytes for an exact Git link, fetches an object, or queues a workflow action. | Run inspection only with isolated absolute config/state roots, no Y sync, and compare the complete workflow file inventory and byte digests before/after, including untracked/archive/round files. Recorded approval must provide no approval/execution action. |

### Verification commands and terminal procedure

Implementation owner runs focused checks first: `cargo test -p spacetop-core --test gate_rooms`, `cargo test -p spacetop-core gate_room`, `cargo test -p spacetop gate_room`, `cargo test -p spacetop-core --test no_write_git_calls`, and `cargo test -p spacetop-core --test no_terminal_deps`. Run the actual 081 test targets once known; do not invent their names. Final required checks are `cargo fmt`, `cargo test`, and `make lint`. For changed refresh subscriptions or real refresh proof, run `cargo test -p spacetop-core --test gate_rooms room_artifact_changes_removal_creation_and_state_replacement_refresh -- --ignored`; run `cargo test -- --ignored` when watcher backend behavior changes.

The planned fixture does not exist yet. After adding it, document reproducible setup in its README: create a disposable directory with `mktemp -d`, copy `tests/fixtures/review-evidence` into its `workflow` subdirectory, initialize and commit a local Git repository there with explicit fixture author identity, and keep absolute `XDG_CONFIG_HOME`/`XDG_STATE_HOME` outside that workflow. Launch the real UI with `cargo run -p spacetop -- --workflow-dir "$proof_root/workflow"` in an interactive terminal; run the AC table's complete, incomplete, missing, tampered and multi-round scenarios at the stated sizes. Exact-Git variants must construct their full commit IDs from that disposable repository, never hard-code a developer's object database.

Capture command, fixture revision, dimensions, key sequence and observed text/screens for every AC. Snapshot all workflow files and their digests before and after an inspection-only pass, excluding only Git internals; compare file additions/deletions as well as contents. Deliberate mutations for refresh evidence belong in a separate run, not the no-write comparison. Missing-object cases stay offline. No terminal walkthrough, Rust check, or product behavior proof was run in this planning stage.

## Stage Report: plan

- DONE: Map current gate-browser question/report/evidence surfaces and plan question-first navigation and honest failure explanations while preserving verified evidence and read-only boundaries.
  Implementation plan maps actual domain/parser/index/app/UI code, canonical report fixtures, explicit failure effects, preserved verified readers, and README/browser guidance.
- DONE: Confirm task 081 prerequisite against current source: identify existing versus absent typed correction-round facts and relationship APIs, and record concrete blockers without duplicating its scanner, inventing links, shrinking AC-3 or implementing task 081.
  081 remains shape; current includes are opaque IDs, Entity has no round facts, and no correction-to-response API exists; the readiness section records the exact upstream contract needed.
- DONE: Record a full implementation path and acceptance evidence for all four ACs, including linked/unresolved/unrelated multi-round responses, missing or tampered evidence and actual terminal scenarios; explicitly distinguish executable work from the dependency hold.
  Ordered steps, owned files, falsifiable AC matrix, focused/full commands and isolated real-terminal procedure cover all four ACs; correction integration/proof remain held.
- SKIPPED: Implementation checks and terminal walkthrough.
  Planning-only assignment: no product changes, builds, tests, lint or formatter runs; required implementation proof is specified, not claimed.

### Summary

Completed the source-grounded implementation and proof plan for all of task 088, including direct canonical-report navigation and honest evidence-failure presentation. Full implementation is not ready until task 081 supplies typed correction-round records and authoritative response relationships; AC-3 remains intact and no 081 work was performed.

## Stage Report: implement

- DONE: AC-1 — The review question and main report are easy to reach.
  Code commit `65093ee`: question/explicit absence leads gate and round content; `r` selects only the canonical `artifact:gate-review` Artifact, preserves supporting selection on Back, and revalidates the exact opened identity on refresh. Core `main_report` and app `direct_report_preserves_support_selection_and_refreshes_exact_preview_identity` fail if order, a filename, a Reference or a replacement item becomes the report.
- DONE: AC-2 — Evidence failures explain their effect.
  Exhaustive typed room explanations retain exact diagnostic detail and recorded decisions; failed/non-UTF-8 bytes have no verified preview. App failure cases cover missing/tampered/blank/unidentified/binary/unsafe/unsupported/oversized evidence; Ratatui and actual terminal runs show the question first and recorded Approve independently. Invalid review logs explain prefix limitations without discrediting an independently bound question.
- DONE: AC-3 — Corrections lead to recorded responses.
  Browser-local `f` filters detail to task081's typed incoming FollowUp edges; Enter follows disposition/closing records and Back restores the full source identity, filter and selected edge. Response tests cover multiple targets, refresh, unrelated cycle-2 prose/IDs, partial cycle-10 evidence and a dangling validation target; they fail if reviewer evidence, another Briefing or a nearby record becomes a response. No response, unknown evidence and recorded response remain distinct; none means fixed/approved.
- DONE: AC-4 — Existing evidence safety stays intact.
  Existing core safe-root/path/size/digest/replacement/exact-Git and no-write/core-dependency tests pass; new report opens still call `gate_room_item`, round artifacts still call `correction_history::read_item`. Actual inspection with isolated absolute XDG roots left all 72 workflow files unchanged, including archive, round and untracked files: zero additions, removals or digest changes; no sync/action was invoked.
- DONE: Implement question-first review navigation and direct canonical-report access with clear missing/invalid evidence explanations, preserving exact recorded decisions and verified reader boundaries.
  README, gate-room guidance, footer, app/render tests and reusable `tests/fixtures/review-evidence` setup/expectations accompany `65093ee`; no new dependency, scanner, write control or task089 work.
- DONE: Integrate all AC-3 correction-to-recorded-response navigation using merged task081 APIs and full identities; distinguish no recorded response from unknown evidence, preserve Back/refresh provenance, and never infer cross-Briefing links or completion.
  Readiness: the historical planning hold is satisfied by merged task081 PR89 (`dc8589a`). Implementation consumes `WorkflowIndex::correction_rounds`, `CorrectionHistory` record/relationship queries and the owning round reader, preserving opaque Briefing IDs, selected-edge identity and current/retained provenance. Historical plan/report text is unchanged.
- DONE: After edits run final required tests/formatting/lint, actual terminal complete/missing/tampered/multi-round/narrow scenarios and full unchanged-file inventory proof; update docs and record DONE/FAILED AC-n evidence bullets and committed stage report.
  Final `cargo fmt`, `cargo test` (749 passed, 6 ignored), `make lint` and `cargo build -p spacetop` passed. Explicit real-notify test `cargo test -p spacetop-core --test gate_rooms room_artifact_changes_removal_creation_and_state_replacement_refresh -- --ignored` passed; actual separate replacement run invalidated the preview and refused tampered bytes while retaining recorded Approve.

### Terminal proof

Fixture setup: `python3 tests/fixtures/review-evidence/setup.py <canonical-temp-root>/workflow`; disposable fixture commit `eb70b27443f34ea8f369c03c2d70d3ab6a56c0fd`. Actual debug binary ran under a macOS PTY with `TERM=xterm-256color`, `SPACETOP_ASCII=1`, isolated XDG config/state, and pyte 0.8.2 screen capture (local proof-only dependency).

Complete: `B,r,Esc,Tab,j,Enter` showed question, canonical report, then entity-snapshot bytes. Missing/blank/tampered/no-designation/unsupported entities showed distinct failures and exact recorded Approve. History: `B,h,Tab,Enter,f,Enter,Enter,Esc,Esc` followed job-592 → decline disposition → closing Resolution → source; cycle 2 had no linked response despite repeated prose, cycle 10 retained known links with unknown additional responses, and validation showed the non-navigable dangling ID.

Actual runs at 150x50, 100x40, 80x24, 40x12 and 20x6 retained usable question/absence and Back navigation; current/retained labels stayed visible. All terminal processes exited 0. Noncanonical macOS `/var` was correctly refused as a symlink; the documented setup canonicalizes to `/private/var` without weakening readers.

Operational evidence is local, not product documentation: `/private/var/folders/h_/tb6bynfj41g1dcvpb2472vrh0000gn/T/task088-proof-3mtkv0uz/` contains final screen/key JSON and ANSI captures, run metadata, before/after complete inventories, `no-write-result.json`, `proof-summary.json`, and the separate refresh-replacement capture. The no-write comparison preceded deliberate fixture mutation.

### Summary

Implemented all four acceptance criteria in product commit `65093ee`. Reviewers can reach the canonical report directly and follow only recorded same-Briefing responses without losing context or confusing evidence failure, log closure, gate approval or execution authority; required checks and actual read-only terminal proof passed.

## Stage Report: verify

- DONE: AC-1 — The review question and main report are easy to reach.
  Reviewed `65093ee`; selector/app tests in final check output `artifact://405` reject first-item/Reference substitution. Independent 40x12 PTY opened the canonical report with a different supporting item selected, preserved the canonical preview through inventory reorder/rebinding, and opened the original distinct supporting bytes after Back/Enter.
- FAILED: AC-2 — Evidence failures explain their effect.
  F1 below reproduces a false whole-Briefing trust failure when only the previously opened report identity disappears from a newly verified inventory; the same screen says “Bound Briefing verified.” Existing missing/tampered/binary cases and recorded decisions are covered, but do not exercise this distinction.
- DONE: AC-3 — Corrections lead to recorded responses.
  Existing response tests and wide terminal captures prove same-Briefing linked/unresolved/unrelated/invalid-prefix cases. Independent 40x12 PTY added a second typed incoming response: selection, Enter, Back and a watched refresh retained its exact identity; paging exposed its body and edge. Filter toggles were exercised: they reset selection to the first eligible edge, but did not infer a response or cross a Briefing boundary.
- DONE: AC-4 — Existing evidence safety stays intact.
  Inspected final exact-Git, safe-read, no-write and terminal-free test output; shortcuts still use the owning core readers. Compared the actual saved before/after inventories: all 72 path/digest entries match, including archive/round/untracked files. New PTY removal scenario invalidated the preview instead of substituting support bytes; no approval, sync or workflow action was invoked.
- DONE: Independently verify all four ACs for question-first/canonical-report navigation, honest failures, recorded correction responses and existing safety boundaries; report concrete material defects only.
  Verdict: request changes for F1, proposed Medium/material because it contradicts the evidence-trust contract in AC-2; no safety-boundary bypass found. Standards axis: typed ownership/read-only boundaries retained. Spec axis: AC-2 is not complete.
- DONE: Inspect recorded 749-test/lint/build/notify and actual terminal/72-file proof without repeating green checks; exercise missing claims, especially exact selection identity across report open/Back/refresh and response filter changes in narrow layouts.
  Read actual final output `artifact://405` (749 passed, six ignored, clippy with `-D warnings`, build) and `artifact://392` (real notify test passed); inspected final narrow/wide screens, replacement capture and inventory files, then exercised only the additional scenarios below.
- DONE: Record verdict and DONE/FAILED AC-n bullets with evidence citations for structured scanning, plus checklist accounting. No code fixes or gate approval; preserve no-inferred-response/approval and typed task081 ownership.
  This body-only report records the verification outcome; no product files or entity frontmatter changed, no merge or gate action performed, and no second correction scanner introduced.
- SKIPPED: Re-running already-green formatting, full tests, lint, build and notify checks.
  Assignment explicitly requires reuse of existing command evidence; the independent work used the built binary and new terminal scenarios, not duplicate suite runs.

### Findings

F1 — **Medium; proposed material; AC-2.** `crates/spacetop/src/app/gate_room.rs:425-426` maps every `BindingMismatch` to “The Briefing does not match the recorded request; its question and inventory cannot be trusted.” However, `crates/spacetop-core/src/gate_room.rs:182-187` also emits that kind for an absent item *after successfully verifying the Briefing*. Refresh of an open report consequently displays this false explanation beneath “Bound Briefing verified,” the verified question, and “Main report not identified; supporting items remain available.” Reviewers cannot tell whether only the report disappeared or the whole evidence room failed trust.

Reproduce with the committed fixture setup: add a valid distinct supporting Artifact and rebind the compact sorted-JSON digest; open `complete` with `B,Tab,r`; remove only `artifact:gate-review` from the inventory and update the recorded Briefing digest, retaining the supporting Artifact; wait for refresh. The captured “Removed canonical identity invalidates preview without substitution” screen shows the contradiction and exact detail `BindingMismatch: item disappeared`. This requires a typed/context-aware distinction between missing selected-item identity and failed room binding, plus an app/render regression asserting that a valid question/inventory is not discredited. Do not parse diagnostic prose or weaken the reader.

### Evidence and limitations

Original implementation proof: `/private/var/folders/h_/tb6bynfj41g1dcvpb2472vrh0000gn/T/task088-proof-3mtkv0uz/`; actual final command logs are retained as session artifacts 405 and 392. Narrow 20x6 captures show question/provenance prefixes, not a complete response body; the new paged 40x12 capture supplies that missing observation.

Independent proof: `/private/var/folders/h_/tb6bynfj41g1dcvpb2472vrh0000gn/T/task088-independent-verify-o1fp19w8/`, files `report-identity-40x12.{json,ansi}` and `response-identity-40x12.{json,ansi}`. Command: worktree `target/debug/spacetop --workflow-dir <proof>/workflow`, actual macOS PTY, pyte 0.8.2, ASCII mode, isolated absolute XDG roots. These are deliberate fixture-mutation scenarios, not a new no-write claim. Report run exited 0; the response harness terminated its still-open nested browser during cleanup (exit -15), not an application crash or a successful graceful-exit claim.

### Summary

Independent verification of `65093ee` is complete; request changes for one reproduced material failure-explanation defect. AC-1, AC-3 and AC-4 have supporting evidence; AC-2 needs the selected-item/whole-Briefing distinction corrected and specifically verified before acceptance.
