---
id: 079
title: Support durable gate status
status: verify
source: "Captain-requested Spacedock release compatibility survey, 2026-10-01"
kind: feature
risk: medium
milestone: spacedock-0.27-compatibility
proof: "Gate parsing, query/export, app state, and TestBackend fixtures; deleting gate parsing or conflating consumed approval with pending readiness must fail the checks."
started: 2026-10-01T10:07:39Z
completed:
verdict:
score: 0.95
worktree: .worktrees/spacedock-ensign-support-durable-gate-status
issue:
pr: "#83"
mod-block: merge:pr-merge
review-round:
    id: round:079:verify:1
    stage: verify
    cycle: 1
    briefing:
        id: briefing:079:verify:round-1
        digest: sha256:75e3f47c27d8eb8b45182080ba5b2cf101f178917be5fc602250229344c68069
        room-ref: ./review/verify/round-1
gates:
    version: 1
    records:
        - id: gate:079:verify
          stage: verify
          attempts:
            - id: gate-attempt:079-verify-1
              briefing:
                id: briefing:079:verify:attempt-1:revision-1
                digest: sha256:e16427fcfa5944eff14475a658d47b9aa0a6d9e06e1e8f8f042b3b220fd8f2cc
                room-ref: ./review/verify/briefing-1
              resolution:
                type: Resolution
                id: resolution:spacedock:079:verify:1
                briefing: briefing:079:verify:attempt-1:revision-1
                by: person:captain
                at: "2026-10-01T15:14:01.885972Z"
                decision: approve
                reason: Captain approved task 079 verify gate in chat; publish PR and handle review comments, with merge reserved for a separate decision.
              application:
                target-stage: done
                state: pending
---

Show durable gate state even when the originating agent session is no longer available. Spacedock 0.27 stores gate attempts, resolutions, and approval application state in entity frontmatter; Spacetop currently discards these fields.

## Scope

- Recommended build order: 1 of 5. Scores express the survey recommendation, not an approved schedule.
- Touches: parser / domain / query / app-state / UI / export.
- Non-goals: Gate preparation, approval, consumption, merge, and all workflow-state writes.

## Acceptance criteria

**AC-1:** Parse versioned gates.records and their attempts, briefing bindings, withdrawals, resolutions, and application states into typed core data. Existing entities without gates remain readable; unsupported or invalid records produce diagnostics rather than approval claims.

**AC-2:** Show current-stage gate status in the task list and detail view, distinguishing awaiting captain, approved awaiting advance, and approved awaiting merge. Historical attempts must not be mistaken for current authority.

**AC-3:** Expose durable gate data through headless JSON export and query filters. Gate readiness remains separate from runtime activity; no session log is required to display a recorded gate.

**AC-4:** Cover pending, consumed, superseded, withdrawn, historical-stage, malformed, and legacy records with parser/query/app tests and TestBackend assertions. Use upstream readiness semantics as the compatibility reference, including preparation and terminal-merge boundaries.

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


## Implementation plan (Spacedock 0.27.3)

### Compatibility decision and exercised risk

Use the tagged v0.27.3 contract at commit `29da151096c1f2b7291a3adafa0f12a762bad78f`, not the pre-release dispatch schema. Canonical references: `internal/gates/{model,io}.go` and `internal/status/{discover,entered_stage}.go` in https://github.com/spacedock-dev/spacedock/tree/v0.27.3. This is an inspection projection of recorded state, not a claim that Spacetop has authenticated approval or can execute it.

The riskiest path is selecting authority after tolerant decoding: permitting application extensions must not permit an invalid binding, older approval, or unknown canonical state. Exercised the upstream parser/reducer before choosing the design. Both packages passed on 2026-10-01 using a checkout verified by `git describe --tags --exact-match` as v0.27.3:

```sh
git clone --branch v0.27.3 --depth 1 https://github.com/spacedock-dev/spacedock.git /tmp/spacedock-0273-proof
cd /tmp/spacedock-0273-proof
go test ./internal/gates ./internal/status -run 'Test(CurrentStageReadiness|StatusProjectsSharedGateReadinessReducer|GateReadiness|InitialGatedSeed|NonInitialGated|PrototypeAndUnknownGateShapes|RetiredProviderEvidence|ReadDiagnostics|ApplicationExtensionShapes)' -count=1 -v
```

Actual exercise used `/private/tmp/spacetop-spacedock-0273-survey`, the already provided tagged checkout; no upstream files changed. Initial sandbox run could not download Go modules; retry with approved network access passed. These tests prove the reference behavior, not a Rust implementation. Go, Git, and module-download access are needed only to repeat this reference check; normal Spacetop builds will not require Go or the local checkout.

### Typed data and authority rules (AC-1, AC-2)

- Add a typed `GateData` on `Entity`: absent, validated v1 document, or invalid/unsupported with path-based diagnostics. Keep warnings alongside valid data. Do not make a gate schema error erase an otherwise readable entity; irrecoverable whole-frontmatter YAML keeps the existing broken-row path.
- V1 data models `GateDocument { version, records }`, `GateRecord { id, stage, attempts }`, attempt ID, briefing ID/digest/optional request-digest/room-ref, optional withdrawal (by/at/reason), resolution (type/id/briefing/by/at/decision/reason/conn/includes), and application (target-stage/state). Use enums for decisions and application states; derive open/withdrawn/closed from the mutually exclusive records instead of trusting an `attempt.state` field. Retain all attempts for detail/export, but select only the last attempt in the unique record matching current status.
- Mirror v1 validation: version 1, nonempty records/attempts for actionable authority, globally unique IDs and briefing bindings, one logical gate per stage, sha256 lowercase digest shape, exact resolution-to-briefing binding, only approve/revise/hold, no withdrawal+resolution, no application on open/withdrawn/non-approve attempts, at most one pending application per record, nonblank application target, withdrawal attribution and UTC timestamp/reason. Match upstream resolution validation: its `at` is required text, not a new stricter timestamp rule. Validate `conn` actor/quote/source disjointness; never treat a citation as authenticated delegation.
- Reject unknown canonical keys, duplicate mapping keys, prototype pointers, unsupported versions, null/scalar/sequence application shapes, and conflicting canonical data. Match the two explicit compatibility exceptions: discard retired attempt `provider-evidence` silently; tolerate unknown fields only inside an application mapping, retaining sorted/de-duplicated path/field warnings. Do not use serde defaults to convert malformed authority into absent gates. Existing flat nonstandard-YAML fallback must never fabricate a valid gate document.
- `GateReadiness` has NotApplicable, Validating, NeedsPreparation, AwaitingCaptain, WithdrawnAwaitingPrepare, FeedbackPending, Held, ApprovedAwaitingAdvance, ApprovedAwaitingMerge, Consumed, Superseded, Invalid. Serialize using upstream vocabulary; Held maps to `not-applicable` in gate readiness, while ungated/terminal current stages have no readiness. Preserve typed distinction internally. Invalid gates remain diagnostic even at stages with no readiness.
- Ordinary, terminal, or unknown current stages do not gain current gate authority. A gated nonterminal stage without a current attempt is validating. Selected open means awaiting captain; withdrawn means withdrawn-awaiting-prepare; revise without application means feedback-pending; hold without application means not-applicable. Approve without application is invalid. Pending approve requires a different existing target: terminal target means approved-awaiting-merge; other target means approved-awaiting-advance. Consumed/superseded remain historical application outcomes, never queued approval. Earlier attempts and other-stage records cannot override the selected state.
- Preparation is a separate read-only proof input to the pure reducer. Promote only validating/no-current-authority to needs-preparation when the actual authority-source entity is tracked and byte-clean against its local HEAD. Initial gated stage needs the clean seed alone; later gated stages need the latest exact-stage report with a nonempty DONE/SKIPPED checklist, nonblank item texts and evidence/rationale, no FAILED, and nonempty Summary. Dirty/untracked/non-Git/unreadable proof stays validating with proof diagnostics; malformed gates stay invalid. Empty explicit records is invalid, not absent. Sibling dirt must not block path-scoped proof.

### Implementation sequence and owned files

1. **Fixtures and parser first.** Add `tests/fixtures/durable-gates/README.md` and representative entity fixtures, recording source tag and adaptations. Add tests in `crates/spacetop-core/src/parser/tests.rs`. Add gate types in `crates/spacetop-core/src/domain/gates.rs` (new), re-export from `domain/mod.rs`, and decode/validate in `parser/gates.rs` (new), wired by `parser.rs` and `parser/item.rs`. Decode raw gates separately from base entity fields using existing serde_yaml; strict key handling must catch duplicates before any map conversion loses them. Keep helpers small and pure. Update Entity constructors in affected test modules through compiler errors without unrelated cleanup. No dependency addition is planned.
2. **Pure reduction and preparation proof.** Add `crates/spacetop-core/src/gates.rs` (new) for readiness reduction and `gate_proof.rs` (new) for report parsing and narrow GitRunner probes. Wire in `lib.rs`, `parser/snapshot.rs`, and `sources.rs`: calculate proof from the physical authority-source file, before merged worktree bodies replace main prose. `parser/worktree.rs` must copy main gate facts and proof when matching slugs merge; worktree-only items use their own path. No git probing, room traversal, or YAML inference in the UI. Use existing `StageDefinition.initial/gate/terminal`; no schema change to README stage flags is needed.
3. **Query and export.** Update `query.rs` with `FieldFilter::GateReadiness(GateReadiness)`; `index.rs` owns current readiness/details query methods and filters active rows from typed facts. Archived rows retain records and diagnostics but have no active readiness; consumed/historical filters must not queue approval. Extend `relations.rs::EntityDetails` with typed gate details. Keep `headless.rs` export additive: preserve existing entity fields, include typed gates/warnings and per-entity current readiness. Add a narrow `list --gate-readiness VALUE` option in `crates/spacetop/src/cli.rs`, translated to the typed filter by `headless.rs`, with clap and headless integration tests in `crates/spacetop/tests/durable_gates.rs` (new). Invalid filter values fail clearly.
4. **App and TUI.** Update `crates/spacetop/src/app.rs` and `app/tests.rs` to read gate projections through WorkflowIndex, refresh them on entity/definition reload, and retain selection/archive behavior. Render a compact gate readiness label in `ui/list.rs`; show the full readiness, selected stage/gate/attempt, target, decision/application state, briefing binding, warnings, and clearly labeled historical attempts in the existing preview metadata/detail path (`ui/preview.rs`; shared typed details may also be used by `ui/relations.rs`). Add TestBackend checks in `ui/tests/task_list.rs` and `ui/tests/preview.rs`. No new input mode or approval key. Keep runtime activity badges as an independent fact, including Idle + AwaitingCaptain without session logs.
5. **Docs and final gates.** Update `README.md` for durable recorded-state labels, preparation proof, historical/invalid behavior, additive JSON and filter examples; update `AGENTS.md` Code Map/Product Shape and `docs/development-policy.md` architecture snapshot only where new module ownership requires it. Preserve existing read-only language and help/footer keys. Record any implementation limitation rather than claiming upstream execution eligibility.

### Acceptance checks and falsifying changes (AC-1 through AC-4)

- Parser fixture matrix: absent legacy gates, valid pending/consumed/superseded, withdrawn, hold/revise, multiple attempts/stages, optional conn, retired provider-evidence, application extension warnings, unsupported version, duplicate IDs/keys/stages, wrong briefing/digest, unknown canonical fields and malformed application. Removing validation or widening unknown-field tolerance beyond application must fail. Valid entities remain selectable even when gate data is invalid.
- Reducer table: replay upstream selected-current-stage cases; newer rejection defeats older approval; historical-stage terminal approval gives no current queued authority; unknown/self target refuses; consumed/superseded never match pending readiness. Removing gates or treating any recorded approve as ready must fail.
- Preparation tests: temporary Git repos for tracked clean initial seed, dirty/untracked seed, later clean complete exact-stage report, missing/FAILED/blank evidence or Summary, newer incomplete report, prior-stage report, non-Git/probe failure, sibling dirt, split-root checkout, and merged worktree body. Promoting from mere report text or from the displayed worktree body must fail.
- Index/query/export tests: active gate filter results and full JSON round-trip; archived records retained without current readiness; invalid diagnostics and tolerated warnings visible. Run headless CLI against fixtures without agent logs. Deleting parser projection or export serialization must fail the assertions.
- App/TestBackend: Idle + AwaitingCaptain, pending advance versus pending merge, consumed/superseded and historical labels, invalid gate diagnostics, narrow/wide terminals, selection/preview scrolling, archive toggle, and definition terminal-flag/entity application edits followed by reload. Deleting gate rendering or equating activity with readiness must fail.
- Read-only regression: compare workflow markdown bytes before/after load/query/export/render. Existing `no_write_git_calls`, `no_terminal_deps`, config/session path, split-root sync, and worktree frontmatter tests remain gates. New proof probes may only read (`rev-parse`, literal pathspec `ls-files`, `diff --quiet HEAD`); never prepare, approve, consume, merge, repair checkout, or open room-ref paths. No watcher backend change is planned; existing event reload handles entity writes. HEAD-only changes may require explicit reload; do not silently claim automatic preparation refresh from Git events excluded by the current watcher.

Required implementation commands: targeted parser/gates/gate_proof/index/app/UI and `cargo test -p spacetop --test durable_gates` during development, then `cargo fmt --check`, full `cargo test`, and `make lint` (`cargo clippy --all-targets --all-features -- -D warnings`). Run `cargo fmt` before the final format check. Run `cargo test -- --ignored` only if implementation changes watcher backend behavior. This plan stage changes only this entity body; Rust gates are owed by implementation, not claimed as run here.

## Stage Report: plan

- DONE: Choose typed gate records and current-stage readiness semantics anchored to Spacedock 0.27.3; exercise the riskiest parse/reduction path before finalizing the design.
  Pinned tag/commit and both upstream Go packages passed the selected parser/readiness tests; unknown canonical bindings or older approval overriding the current attempt would fail those tables.
- DONE: Define a narrow implementation plan across core parsing/query/export, app state, and TUI list/detail with acceptance criteria and negative cases for invalid or historical authority.
  Plan above maps AC-1–AC-4 to parser, reducer, proof, query/export, app, and TestBackend checks; it separates runtime activity from durable readiness and preparation from approval authority.
- DONE: Name exact owned files, test-first checks, required cargo test and make lint gates, docs updates, and read-only boundaries; record the plan and checklist accounting in the task stage report.
  File ownership and falsifying changes are explicit above; only this entity body changed. Full cargo test/make lint are required for implementation and were not needed for this documentation-only stage.

### Summary

Defined a test-first Rust plan against Spacedock v0.27.3 and exercised its strict gate decoding and current-stage readiness reference before finalizing the design. The plan preserves main-frontmatter authority, records invalid/historical state without claiming queued approval, and adds only read-only proof probes and inspection surfaces.

## Stage Report: implement

- DONE: Implement typed durable gate parsing, current-stage readiness, and read-only preparation proof against Spacedock 0.27.3 with negative tests before production changes.
  Code commit `088710b`; initial CLI tests failed on missing readiness projection and unsupported filter before production edits. Strict parser/reducer/proof matrices now pass; allowing malformed binding or dirty/untracked proof would fail them.
- DONE: Expose gate status separately from runtime activity through index queries, list filtering, JSON export, and list/preview details; prove current, historical, invalid, consumed, and superseded cases with app and TestBackend tests.
  Headless export/filter/archive tests and app/UI durable_gates tests pass; omitting labels, using runtime activity as gate authority, or queuing consumed/historical approvals would fail their observed output assertions.
- DONE: Update nearby docs, preserve workflow/git/config/session safety, run cargo fmt --check, full cargo test and make lint, commit deliverables on the implementation branch, and record evidence for AC-1 through AC-4 plus DONE/SKIPPED/FAILED checklist accounting.
  README, AGENTS and development policy updated; `cargo fmt --check`, full `cargo test` (662 passed, 3 ignored), `make lint` and `git diff --check` pass on `088710b`; all git/config/session/core guardrails remain passing.
- SKIPPED: Run ignored notify backend smoke tests.
  No watcher backend or event filter changed; the three existing real-backend checks remain explicitly ignored by the ordinary suite.

### AC evidence

- AC-1: `strict_gate_parser_matrix_refuses_invalid_authority_and_retains_extensions_only_at_application`, `duplicate_stage_attempt_briefing_resolution_and_pending_applications_are_refused`, and parser unit negative fixtures cover canonical fields, binding/digests, duplicate identities/keys/stages, malformed applications, legacy/prototype/version refusal, conn attribution, retired provider-evidence and sorted application warnings. Relaxing canonical validation or losing typed data breaks assertions.
- AC-2: `readiness_uses_only_last_attempt_in_current_gated_nonterminal_stage` covers selected versus earlier attempts, revise/hold/withdrawal, self/unknown targets, terminal merge versus advance, consumed/superseded, other-stage and terminal records. App reload and narrow/wide TestBackend tests independently assert displayed classifications; treating any approve as current readiness fails.
- AC-3: `query_keeps_runtime_and_archive_authority_separate` and all three headless durable_gates tests exercise JSON without session logs, readiness filtering, retained warnings and archived records with null readiness. Removing query/export projection or letting archived pending records match the filter fails.
- AC-4: Preparation tests exercise clean initial seed, dirty/untracked/non-Git/probe failure, complete later report, missing/FAILED/blank checklist evidence/Summary, newer incomplete and other-stage reports, literal filename, sibling dirt, split-root local HEAD and main-versus-worktree body proof. External unverified state stays readable with validating readiness and no new preparation probes; taking proof from displayed prose or an escaped checkout fails.

### Commands and reference

- `cargo test -p spacetop-core --test durable_gates` (7 tests in final suite); `cargo test -p spacetop durable_gates` (app/TestBackend); `cargo test -p spacetop --test durable_gates` (3 CLI tests in final suite). Full `cargo test` also covers existing regression/guardrail suites.
- Spacedock v0.27.3 reference checkout at `29da151096c1f2b7291a3adafa0f12a762bad78f`: `GOPROXY=off go test ./internal/gates ./internal/status -run 'Test(CurrentStageReadiness|StatusProjectsSharedGateReadinessReducer|GateReadiness|InitialGatedSeed|NonInitialGated|PrototypeAndUnknownGateShapes|RetiredProviderEvidence|ReadDiagnostics|ApplicationExtensionShapes)' -count=1` passed both packages. Go/reference checkout are only repeat-reference requirements, never Spacetop runtime dependencies.
- Code branch: `spacedock-ensign/support-durable-gate-status`; commit: `088710b`. Build/test logs: `/tmp/spacetop-079-cargo-test.log`, `/tmp/spacetop-079-lint.log`; reproducible checks and fixtures are committed.

### Summary

Implemented typed v1 recorded gates, current-stage readiness and narrow read-only preparation proof, with independent query/export, compact list labels and scrollable preview history. Invalid metadata remains diagnostic and readable; room refs and conn citations remain recorded facts rather than authenticated execution authority. HEAD-only changes may require explicit reload; no gate preparation/approval/consumption/merge or workflow-state writer was added, and FO JEV assessment plus fresh verification remain separate next-stage work.

## Stage Report: verify

- DONE: Independently assess AC-1 through AC-4 against committed code, upstream v0.27.3 semantics, fixtures and observed output; challenge invalid/historical gate authority, read-only preparation proof, and separation from runtime activity.
  Reviewed `088710b25f0d5f9fccec83bc281cd5efdc8e0f26` against upstream `29da151`; focused parser and tracked-seed CLI probes reproduced invalid gates becoming preparation-ready (finding below).
- DONE: Review parser/query/app/UI boundaries, split-root/worktree containment, export/filter/archive behavior, narrow terminals and docs; reproduce any material defect with the lowest practical test and record concrete findings and proposed dispositions.
  Typed core/query/UI ownership and retained main proof are sound in reviewed paths; one High parser defect is reproduced at the parser boundary and observed through export. Product code remains unchanged.
- DONE: Confirm required format, full cargo test and make lint evidence for the reviewed commit, avoid redundant reruns of already-green checks unless a new change or unresolved concern warrants them, and write a verify report with verdict, AC evidence and checklist accounting; retain JEV assessment provenance for FO gate review.
  Confirmed full logs: 662 passed, zero failed, three unchanged notify tests ignored; clippy all targets/features with `-D warnings` passed. Fresh `cargo fmt --check` passed; clean worktree remains at reviewed commit. Green full gates were not redundantly rerun.

### Findings and judgment

High — `crates/spacetop-core/src/parser/gates.rs:70-78`: on any decoding/validation error, a source-line heuristic decides whether gates were present. Valid YAML flow mappings do not start with `gates:`, so unsupported versions and malformed canonical data silently become `GateData::Absent`. A tracked clean initial seed then gains `needs-preparation`, contradicting AC-1 and the rule that malformed gates never promote.

Judgment: Request changes. Replace the line-presence fallback with structural gate-presence/error handling that preserves invalid diagnostics for valid YAML flow/explicit-key/alias forms, while retaining the intended legacy flat-frontmatter fallback. Add parser and clean-seed query/export regressions; rerun required gates after correction. FO owns semantic/AC adjudication and correction routing.

### Reproduction and observed output

- Minimal parser probe retained at `/tmp/spacetop-079-review-flow-probe.rs`; log at `/tmp/spacetop-079-review-flow-probe.log`. It writes `---\n{id: flow, title: Flow invalid gates, status: seed, gates: {version: 99, records: []}}\n---\nSeed\n`, calls `parse_work_item` with allowed status `seed`, and asserts `GateData::Invalid`.
- Exact parser command from the code worktree: copy that probe to `crates/spacetop-core/tests/review_flow_probe.rs`, then run `cargo test -p spacetop-core --test review_flow_probe`. Exit 101; assertion failed with `actual: Absent`. Temporary probe was removed after execution; no product/test commit was made.
- CLI fixture retained at `/tmp/spacetop-079-review-fixture/{README.md,invalid-flow.md,invalid-block.md}`; observed JSON at `/tmp/spacetop-079-review-flow-output.json`. Original fixture was a fresh nested Git repo in the code worktree, initialized with `git init -q`, test-only local name/email, `git add .`, `git commit -qm 'Review fixture'`; command: `target/debug/spacetop export --json -w gate-review-t9kerq2w`.
- Flow entity observed: `gates={"kind":"absent"}`, `gate_readiness="needs-preparation"`, `gate_preparation={"proven":true,"diagnostics":[]}`. Equivalent block entity observed: `gates.kind="invalid"`, diagnostic `gates: version must be 1 and records nonempty`, readiness `invalid`, preparation false. Expected flow result equals block result. Both contained identical gate values; neither required session evidence.
- Fresh reproduction: copy the retained fixture files to a fresh Git directory under the code worktree, run the same init/config/add/commit sequence, then export it with the reviewed binary. The files/probe fully describe synthetic input; retained `/tmp` paths are evidence conveniences, not runtime dependencies.

### AC evidence and residual boundaries

- AC-1: NOT SATISFIED. Strict binding/identity/digest/conn and application-extension matrices otherwise pass in committed evidence; flow-map invalid metadata escapes that validation result. Accepting unsupported data as absent fails the new parser assertion.
- AC-2: Supported for canonical valid inputs. The selected last current-stage attempt reducer matches upstream `internal/gates/model.go`; unknown/self targets refuse, terminal target selects merge, spent/other-stage records never queue pending approval. Existing app reload and TestBackend narrow/wide assertions pin these classifications. The finding prevents unconditional approval.
- AC-3: Supported for canonical valid inputs. Query/filter/export tests retain typed records and warnings without sessions; archived readiness is null and excludes queued filters. Durable readiness and runtime activity are independent projections; the fresh export exercised this without agent logs. Invalid-flow diagnostics remain blocked by AC-1.
- AC-4: NOT SATISFIED. Existing negative matrices cover block malformed/version/legacy records, pending/consumed/superseded/withdrawn/history, physical main-versus-worktree proof, split-root local HEAD and unverified escape refusal, dirty/untracked/report-negative cases, and narrow/wide UI. The missing flow-frontmatter negative case demonstrates a real gap.
- Read-only proof: only `rev-parse`, literal `ls-files`, and path-scoped `diff --quiet HEAD` were added; no retained room opening or new Git writes. Existing guardrails remain green. Report completion mirrors upstream exact-stage/checklist/Summary selection. Watcher/backend unchanged; HEAD-only explicit reload caveat is documented.

### FO JEV provenance

FO's implementation-readiness request `/private/tmp/spacetop-079-jev-implementation-request.json` and actual response `/private/tmp/spacetop-079-jev-implementation-response.json` used `jev-1.13.0`: 9329 input tokens, 147 output tokens, USD 0.000391818. Three implementation checklist answers were supported at confidence 0.88/0.86/0.85. This is an earlier FO semantic judgment, not worker verification evidence or final AC approval; the fresh counterexample above was not part of that assessment. No reviewer JEV call was made.

### Summary

Independent verification found one High defect that loses invalid durable gate data and incorrectly promotes a clean initial entity to preparation-ready. The existing tests and lint are green, but AC-1 and AC-4 require a focused correction and regression; FO JEV review and captain approval remain pending.

## Stage Report: implement (cycle 1)

- DONE: Fix the structurally reproduced invalid-flow-gates to absent defect; prove flow/explicit-key/alias invalid authority remains diagnostic and cannot promote a clean seed.
  `eb8e940` separates structural YAML gate presence from canonical validation; parser and tracked-clean-seed CLI regressions first failed with Absent / needs-preparation / proven=true, then passed with invalid diagnostics / invalid readiness / proven=false and zero preparation filter matches.
- DONE: Run required format, full cargo test and make lint after the narrow fix; commit deliverables on the existing implementation branch and report AC-1/AC-4 evidence.
  `cargo fmt --check`, full `cargo test` (664 passed, 3 unchanged notify ignored), `make lint`, and `git diff --check` passed after correction; only parser and its two regression test files changed at `eb8e940659c3a5420743ef60fa751bbbf58ae5e3`.
- DONE: Return a closed canonical Briefing/review log for verify/1 that retains reviewer finding, JEV-authorized disposition and completed correction response; FO records the round and reuses the reviewer.
  Inputs: `/private/tmp/spacetop-079-feedback-round-1/briefing.json` and `briefing.review.jsonl`; five attributed entries preserve reviewer revise, FO disposition and closing worker correction response. Source-based upstream schema/includes/closed-tail checks passed; the worker did not record the round or rerun JEV.

### Feedback Cycles

- Cycle 1: High invalid-flow-gates finding on `088710b` — Material / Fix, as authorized by FO's actual `jev-1.13.0` finding_disposition=material_fix, confidence 1.0, probability 1.0; no declined findings or new product decisions.
- JEV provenance supplied by FO: `/private/tmp/spacetop-079-jev-finding-{request,response}.json`, 6225 input / 67 output tokens, USD 0.00026145. This is the recorded FO judgment, not a new worker model call. Original reviewer report and rejection remain unchanged.
- Concrete correction: present structural gates never fall back to absent after schema errors. Legacy absence is permitted only when the existing flat-scalar parser succeeds and YAML-decoded scalar keys exclude gates; quoted gates cannot escape refusal.

### AC evidence and checks

- AC-1 evidence: `structural_gate_presence_refuses_invalid_flow_explicit_key_and_alias_forms` covers unsupported flow/explicit-key/value-alias/key-alias gates, empty-record flow gates, quoted gates under legacy syntax, ungated flow and legacy unquoted-colon metadata. Restoring the source-line heuristic makes invalid input Absent and fails the test.
- AC-4 evidence: `clean_seed_with_invalid_structural_gates_never_promotes_in_query_or_export` initializes and commits real Git seeds, exercises export and readiness filters without session logs, asserts invalid diagnostics and no proven preparation, and checks byte-clean HEAD. Reclassifying any invalid structural gates as absent or promoting them breaks the observed JSON/filter assertions.
- Targeted red/green commands: `cargo test -p spacetop-core --test durable_gates structural_gate_presence`; `cargo test -p spacetop --test durable_gates clean_seed_with_invalid_structural`. Final full-check logs: `/tmp/spacetop-079-cycle-1-cargo-test.log`, `/tmp/spacetop-079-cycle-1-lint.log`.
- Briefing ID: `briefing:079:verify:round-1`; raw artifact revisions bind reviewed and corrected Git blobs, plus the original counterexample. First reviewer Resolution remains revise; closing worker Resolution is a correction response awaiting fresh verification, not approval. FO alone records `verify/1` and routes the existing reviewer.

### Summary

Corrected the original fallback that lost invalid gates in valid structural YAML, while preserving legacy gate-free flat frontmatter and all existing strict checks. Added parser and real clean-seed query/export regressions and passed all required implementation checks. The closed correction inputs retain the original rejection and actual FO JEV disposition; round recording and reviewer acceptance remain FO next-stage work.

## Stage Report: verify (cycle 1)

- DONE: Independently assess AC-1 through AC-4 against committed code, upstream v0.27.3 semantics, fixtures and observed output; challenge invalid/historical gate authority, read-only preparation proof, and separation from runtime activity.
  Independently reviewed corrected commit `eb8e940659c3a5420743ef60fa751bbbf58ae5e3`; the unchanged original parser probe and fresh tracked-seed export now refuse invalid flow gates. AC evidence below supports all four criteria.
- DONE: Review parser/query/app/UI boundaries, split-root/worktree containment, export/filter/archive behavior, narrow terminals and docs; reproduce any material defect with the lowest practical test and record concrete findings and proposed dispositions.
  Correction is limited to parser and two regression files; original core/query/app/UI, containment, archive and docs review remains applicable. No remaining material finding; propose accepting the correction and returning to FO final AC judgment.
- DONE: Confirm required format, full cargo test and make lint evidence for the reviewed commit, avoid redundant reruns of already-green checks unless a new change or unresolved concern warrants them, and write a verify report with verdict, AC evidence and checklist accounting; retain JEV assessment provenance for FO gate review.
  Confirmed correction logs: 664 passed, zero failed, three unchanged notify tests ignored; required clippy all-targets/all-features `-D warnings` passed. Fresh format/diff checks pass; only targeted counterexample/regression checks were rerun.

### Findings and judgment

No remaining material findings. Judgment: Approve corrected implementation for FO gate review. The original High finding is resolved; this worker judgment does not mark captain approval, merge, workflow completion, or FO final AC adjudication.

### Independent reproduction and retained evidence

- Original probe: copied `/tmp/spacetop-079-review-flow-probe.rs` unchanged to `crates/spacetop-core/tests/review_flow_probe.rs`; ran `cargo test -p spacetop-core --test review_flow_probe`. Now passes (previously failed with Absent). Log: `/tmp/spacetop-079-reverify-original-probe.log`; temporary test removed afterward.
- Added parser shape check: `cargo test -p spacetop-core --test durable_gates structural_gate_presence` passed. It refuses unsupported flow/explicit-key/value-alias/key-alias gates, malformed flow records and quoted legacy gates; gate-free flow/legacy inputs remain absent. Restoring the old line heuristic fails it. Log: `/tmp/spacetop-079-reverify-shapes.log`.
- Added query/export check: `cargo test -p spacetop --test durable_gates clean_seed_with_invalid_structural` passed. Four tracked clean structural seeds export invalid diagnostics, invalid readiness and false proof; preparation filter is empty, invalid filter returns four; Git byte cleanliness is retained. Promoting invalid metadata fails it. Log: `/tmp/spacetop-079-reverify-seed.log`.
- Fresh original CLI fixture: copied retained `/tmp/spacetop-079-review-fixture` files into a new nested Git directory under the code worktree; initialized/configured/added/committed the synthetic seeds, then ran `target/debug/spacetop export --json -w <fixture>`. Flow and block now both export `gates.kind=invalid`, diagnostic `gates: version must be 1 and records nonempty`, readiness invalid, preparation false. `git diff --exit-code HEAD` passed. JSON: `/tmp/spacetop-079-reverify-flow-output.json`; temporary fixture removed.
- Final worktree remains clean at `eb8e940`; fresh `cargo fmt --check` and `git diff --check` pass. Required owned full logs: `/tmp/spacetop-079-cycle-1-cargo-test.log` (18 result groups, 664 passed, zero failed, three ignored) and `/tmp/spacetop-079-cycle-1-lint.log` (successful required clippy). No watcher/backend change justifies ignored smoke reruns.

### AC assessment

- AC-1: Supported. Structural gate presence is established before schema decoding; every present-invalid shape retains diagnostics. Legacy gate-free fallback remains scoped to successfully parsed scalar entries whose YAML-decoded keys exclude gates. Retained strict parser matrices pin typed attempts, bindings, withdrawal/resolution/application, canonical refusal and extension warnings; widening authority acceptance fails them.
- AC-2: Supported. Pure current-stage selection and latest-attempt ordering remain aligned with upstream v0.27.3 `internal/gates/model.go`; app reload/TestBackend coverage retains captain/advance/merge/spent/historical/invalid labels at narrow and wide widths. Treating old or spent approval as pending breaks those assertions.
- AC-3: Supported. Fresh exports require no session log; clean invalid seeds cannot enter preparation filters. Retained query/headless/archive tests preserve typed records with null archived readiness and separate runtime activity. Removing projection, merging activity with readiness or queueing archived approval fails those checks.
- AC-4: Supported. Original malformed-authority gap now has parser and real Git query/export regressions. Retained pending/consumed/superseded/withdrawn/legacy/history matrices, report-negative/dirty/untracked proof, split-root local HEAD, external-unverified refusal, main-versus-worktree source, reload and TestBackend assertions cover the stated boundaries. No new probe, write, dependency, terminal-core coupling or UI schema inference was introduced.
- Residual boundary: HEAD-only changes still may require explicit reload; documented and unchanged. Room refs/conn citations are recorded inspection facts, not authenticated execution permission. No material residual risk found beyond these declared product limits.

### FO JEV provenance

FO finding assessment `/private/tmp/spacetop-079-jev-finding-response.json`: `jev-1.13.0`, material_fix, confidence/probability 1.0, 6225 input / 67 output tokens; reported cost USD 0.00026145. Repair-route response `/private/tmp/spacetop-079-jev-round-route-response.json`: same model, convert_then_record, confidence 0.83 (probability 0.88), 803 input / 50 output tokens. These are earlier FO semantic judgments; this worker independently exercised the correction and made no JEV call. Existing `verify/1` remains the sole correction round; no second round was published. FO final AC assessment is recorded below; captain approval remains pending.

### FO final JEV AC assessment

Actual FO `jev-1.13.0` calls against `eb8e940` are retained in `evidence/jev/`: compact response JSON, original input request SHA256, model, explicit source commit references, usage and cost; no raw input source snapshots are published.
Initial core assessment: AC-1 satisfied 0.95; AC-4 insufficient_evidence 0.33. Initial surface assessment: AC-2 satisfied 0.53; AC-3 satisfied 0.87. Follow-up supplied actual app/TestBackend/upstream evidence: AC-2 satisfied 0.84; AC-4 satisfied 0.52.
Final selected AC-1/2/3/4 confidence: 0.95/0.84/0.87/0.52. AC-4 is the least certain and retains initial insufficient-evidence provenance; the outputs give no additional rationale. Observed six-call ledger total: USD 0.002453976.
These FO model judgments supplement worker verification; captain gate approval remains pending. This provenance-only update made no model call, test rerun, product edit, or second correction round.

### Summary

The original invalid-flow-authority counterexample is fixed, independently reproduced green, and covered by structural YAML parser and clean-seed export/filter regressions. All four ACs now have supported evidence; the corrected implementation is has actual FO semantic assessment recorded above and remains ready for captain gate review.
