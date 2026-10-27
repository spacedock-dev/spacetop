---
title: Generate human-readable task summaries on demand
status: verify
source: "Captain selected Option A and requested an entity file, 2026-10-23"
kind: feature
risk: high
milestone: usability
proof: Human comprehension walkthrough, snapshot/race scenarios, and bounded trusted-CLI adapter/adversarial checks; see amended acceptance criteria.
started: 2026-10-23T09:22:08Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-on-demand-task-summaries
issue:
pr:
mod-block:
id: 090
review-round:
    id: round:090:verify:1
    stage: verify
    cycle: 1
    briefing:
        id: briefing:090:verify:round-1
        digest: sha256:d176682d755213bf67eabdc2c39dd1c8ce766ebe045c6268ca154595e5c430dc
        room-ref: ./review/verify/round-1
gates:
    version: 1
    records:
        - id: gate:090:verify
          stage: verify
          attempts:
            - id: gate-attempt:090-verify-1
              briefing:
                id: briefing:090:verify:attempt-1:revision-1
                digest: sha256:1ea833e319dacab62168e6897ea4953c2070be555f6d7c5fe57e7896f929ae24
                room-ref: ./review/verify/briefing-1
              resolution:
                type: Resolution
                id: resolution:spacedock:090:verify:1
                briefing: briefing:090:verify:attempt-1:revision-1
                by: person:captain
                at: "2026-10-27T03:47:03.080092Z"
                decision: approve
                reason: Actual Kent decision, not FO or Navigator judgment. Parent71fac6c1-962e-4f9f-abf3-34a04cc7c166 messagebb0cda50-2771-41be-85f9-3d4a0ce85676 asked:「你是要接受『尚未做真人閱讀測試』的缺口，先批准090建立PR嗎？若是，我會保留『AC-1未驗證』的紀錄，不把approve當成測試通過；這次批准也不包含合併。」 Kent answered「對」following「approve」; Navigator relay68864c3b-33cf-4b40-8ad0-97390003e631. Supplementary Kent quote “checked the screenshot approve this design”, parent messagee1c08260-34b8-4e56-bd0e-90d198d25c8d, relay0590a9e1-7542-477a-b446-de5da72884ab; images technically remain terminal capture renderings. Accepts direction and AC1 NOT PERFORMED/UNVERIFIED exception for unchanged63b31f16b4f962f88f665f356c51a1ee21a2b953 and frozen acceptance9d1aa4d77882f78fd1786dabbdb7c7d3db888a26af99c8e85a7ecdde692c3f3a. PR publication only; no merge,cleanup,additional provider calls or changed-candidate approval. AC2–5 retain scoped PASS and documented limits.
              application:
                target-stage: done
                state: pending
---

Help a person understand a long agent-authored task file without reading its full history. Add an explicitly requested AI summary in the preview while retaining the original text as the source of truth. The captain selected Option A: on-demand generation, not automatic generation while browsing.

## Scope

- Kind: feature.
- Risk: high, because launching coding-agent CLIs adds a new side-effect boundary.
- Milestone: usability.
- Touches: app-state / UI / configuration / subprocess execution / docs / policy.
- Provide Summary and Original views. Preserve recorded metadata, gate/attention facts, source access, and existing worktree-diff behavior. Choose the shortcut against the existing key map; S is a proposal, not a reserved binding.
- Aim for 150–250 words explaining purpose, recorded progress, remaining work/decisions, and important limitations or missing evidence. Say not recorded when the source does not answer. Distinguish reported verification from independently checked evidence; expose conflicting reports rather than inventing reconciliation.
- Select Claude or Codex through user-level configuration, using non-interactive adapters for `claude -p` and `codex exec`. Claude uses a verified available Haiku-family model; Codex uses a verified available Luna-family model. Confirm exact model IDs and syntax from installed CLI metadata rather than guessing aliases or silently selecting another family. OMP is outside this task.
- Trust the selected installed CLI and its existing auth, helpers and configuration. The app must not read/copy secrets, query Keychain items, own credentials, add an auth bridge, or require new accounts/API keys. The CLI may use its own credentials/network/configuration. A user needs only one available provider. Missing executable, login or requested model produces an actionable error with Original still usable; no silent provider/model fallback. Use verified compatible native no-tools/no-hooks/read-only controls without claiming a complete sandbox.
- Capture exact task input and source-copy identity. Bind results to workflow/task/source copy/input digest/provider/model/prompt version and request identity. Keep an in-memory cache only. Refresh outdated summaries only on explicit request.
- Run asynchronously with a visible indeterminate spinner and truthful short state such as `Generating summary…`; elapsed time is optional, but no invented percentage or ETA. Use the existing TUI tick/render loop without extra polling or network requests. Bind the indicator to the exact request/task; keep Original readable, navigation responsive and cancellation available. Stop animation on success, error, cancellation or timeout, and reject late results for a different selection. Bound input/output/time. Cancel/timeout/quit terminate and reap the tracked process/group and discard late results; escaped or untracked descendants may survive. Explain selected CLI trust, data transmission, possible usage charges and this cleanup limit before first generation.
- Amend the product read-only policy to distinguish app-owned effects from explicitly requested trusted-CLI execution. The app does not sandbox all CLI effects or promise the CLI cannot read credentials. Keep app-authored writes out of projects/workflows, use a private cwd outside the project, captured task data on bounded stdin, and executable/argument arrays without shell interpolation. Apply only native restrictions verified for the installed version and compatible with existing auth/config. Disclose residual CLI effects; do not claim empty cwd or prompt text protects the computer.
- Non-goals: automatic generation on navigation, background batches, persistent summary caches, changes to canonical task files, workflow dispatch, autonomous task actions, linked-evidence crawling, or inferred status/approval changes.

## Acceptance criteria

**AC-1 -- A reader unfamiliar with a task can identify its purpose, recorded progress, and outstanding decision or limitation from the summary.**
Verified by: a planned walkthrough with an identified unfamiliar reader and recorded answers against source material. Agent-generated judgments do not substitute for this human evidence; any missing proof remains explicitly unmet unless the captain approves a documented scope decision.

**AC-2 -- Generated prose never substitutes for recorded workflow facts or hides missing proof.**
Verified by: tasks with rejected reviews, contradictory reports, and unperformed human checks retain those distinctions. Typed status/gate facts remain authoritative; original text and worktree-diff views remain reachable. Proposed source links, if included, resolve only to validated sections of the captured input.

**AC-3 -- Results remain bound to their exact task snapshot.**
Verified by: switch tasks/workflows/source copies and change input while generation runs; no result appears on the wrong selection or as current for changed input. Matching requests reuse cached results; stale summaries are visibly marked and require explicit regeneration.

**AC-4 -- Summary generation is explicit and failure does not impair inspection.**
Verified by: browsing/opening previews causes zero provider requests; the first generation explains the selected provider, model, data transfer and possible usage charges. Missing executable/credentials/model, nonzero exit, malformed or empty output, oversized input/output, timeout, and cancellation leave Original usable. Oversized input is not silently truncated. Actual in-progress and terminal-state frames prove the request-bound spinner runs only while generation is active, with no invented percentage/ETA or extra requests. Navigation/late-result scenarios cannot attach progress to the wrong task. The UI remains responsive. Cancellation, timeout and application exit terminate/reap the tracked process/group and discard late results. Tests distinguish tracked-process cleanup from escaped/untracked descendants, which may survive under the explicitly accepted trusted-CLI contract; no complete descendant-cleanup claim is permitted.

**AC-5 -- Both adapter paths enforce app-owned generation boundaries and disclose trusted-CLI limits.**

Verified by: both Claude/Haiku and Codex/Luna paths use executable-plus-argv invocation, bounded task DATA input, a cwd outside the project, verified compatible native no-tools/no-hooks/read-only controls, and no app-authored project/workflow writes. Trust the selected CLI's existing auth/helpers/config; this is not full OS isolation, credential non-access, inference-only egress or complete descendant containment. Before explicit generation disclose that trust/data/cleanup contract. Adversarial task text tests observed edits, shell/file/tool actions and each claimed native control; retain failures and report dangerous results beyond the accepted boundary rather than calling them PASS. Real synthetic E2E uses locally available configured providers; audit project/workflow files and record exact controls/effects and unavailable executable/login/model limits. One selected available provider suffices, both adapters require behavior coverage, and there is no silent fallback. Original stays usable. Sanitize untrusted provider output against terminal control effects. The app itself never reads/copies secrets, queries Keychain, brokers auth, performs linked-evidence crawling or creates background generations.

## Proof plan

- Lowest test layer: typed request/result and cache state tests; subprocess lifecycle tests for real failure/race boundaries; UI rendering/navigation checks.
- Required command: make lint plus relevant established tests; record commands actually run.
- Manual checks: actual debug-build TUI generation, an in-progress spinner frame and success/failure/cancel frames, navigation during generation, source changes, Original/diff switching, cancel and quit. Real provider smoke uses synthetic data with existing logged-in Claude/Haiku or Codex/Luna where available; both adapter paths require behavior coverage. Navigator retains illustrated acceptance and an immutable debug executable outside the worktree.
- Human check: identify the unfamiliar reader during planning, before declaring a verification-ready task.
- Docs/policy update needed: README/help/configuration guidance, narrow read-only-policy allowance for provider execution, provider prerequisites, native controls and trusted-CLI limitations. Do not add persistent storage roots in this task.

## Commit Discipline

- Commit status changes at dispatch and merge boundaries.
- Commit task body updates when substantive.
- Commit implementation work on the task worktree branch before signaling completion.

## Captain scope clarifications — 2026-10-23

These explicit decisions amend the original adapter scope and, in the latest decision below, the security contract. AC-1 human evidence and Navigator/FO runtimes are unchanged. Earlier strict-isolation failures remain historical failures, not converted passes.

- Existing-login decision: Kent said 「直接用 claude -p, codex exec , omp -p 就好了 不要管 API key 使用者的 claude, codex, or omp 應該至少有一個可以用的」. Source: parent `71fac6c1-962e-4f9f-abf3-34a04cc7c166`, message `0d79fe0c-98a6-440b-bc4a-040a2d5e6f04`, relayed by Navigator message `b5a93336-737c-41f2-b75a-c5e6ad7b3118`. Preserve existing CLI login/configuration; no new API keys/accounts or credential inspection.
- Superseding adapter/model decision: Kent said 「簡化一點好了 用 claude 或 codex 就好 然後選小 model Haiku 或 Luna」. Source: same parent, message `8e8b06b4-8642-4a13-80da-cb3ae4320bd7`, relayed by Navigator message `ab160757-06de-4878-8191-1b49fe4b1bb8`. Only Claude/Haiku and Codex/Luna remain; prior OMP findings are historical, not an implementation obligation. Do not substitute another model family if the requested one is unavailable.
- Progress decision: Kent said 「對了，在 agent 處理 summary 的期間，可以顯示動畫或進度」. Source: same parent, message `78640aa0-e894-472b-b818-2b6045cb852c`, relayed by Navigator message `91dac744-db43-42c5-a01d-96546f30960a`. Use a standard indeterminate spinner and truthful status within the existing render loop; preserve responsive Original/cancel and exact request binding.
- Authorized live-smoke ceiling: at most **six actual CLI launches total**, normally normal/adversarial/cancel for each of the two adapters, after local app-boundary readiness under the latest trusted-CLI decision. Each uses at most 8 KiB synthetic input, 45 seconds, 64 KiB stdout and 16 KiB stderr; no automatic retry. Prior launches count toward this ceiling. **Zero actual provider launches used at this amendment.** This is not a dollar hard cap; report observable usage without inventing a cost. Use existing available login/configuration and the verified requested small model; no new model/account/budget questionnaire is required.
- Missing runtime/login/model is an explicit availability/evidence limit, not a reason to silently choose another provider or claim a live pass. The historical process-group detached-child gap remains proved. The latest decision accepts that residual risk while requiring tracked-process/group termination and reaping. Verify compatible native controls without claiming complete containment; snapshot races, zero navigation requests and unfamiliar-human proof remain required.

- **Superseding trusted-CLI security decision:** Kent said 「直接用 CLI」. Source: parent `71fac6c1-962e-4f9f-abf3-34a04cc7c166`, message `b833a4b2-e3bb-43a3-9a66-a959060a40ff`, relayed by Navigator `ea139e78-eaeb-43b0-98f6-b7c15427b1a1`. The preceding choice explicitly explained existing login, no auth bridge/key management, native controls plus adversarial/cancel tests, and no full-isolation or escaped-descendant termination promise. This is actual human approval to narrow AC-4/AC-5 and resume direct CLI implementation. Trust installed CLI/auth/helpers/config, retain all app-owned limits, stop strict OS-sandbox/Keychain research. No credential owner, new account, model/provider fallback, blanket safety PASS, gate/PR/merge approval or AC-1 waiver is granted. FO is explicitly authorized to persist this task scope/AC/proof/plan amendment before resuming the same worker. All previous negative reports stay intact.

## Implementation plan — scoped 2026-10-23

This current plan supersedes the isolation-only sequence after Kent's trusted-CLI decision. Preserve all five currently amended ACs, Claude/Haiku and Codex/Luna, existing CLI login and both released platforms. First implement the app-owned subprocess/adapter boundary and run bounded real summarization E2E; then build the responsive Summary UI. Do not resume strict sandbox, auth bridge or Keychain research.

### Goal, confidence, risk and next decision

The goal remains readable, explicit task summaries with Original and recorded facts authoritative. Native adapter compatibility and actual model output need live evidence; typed request/state/rendering behavior has a clear test path. The accepted risk is trusted CLI/config/helper side effects and potentially surviving escaped descendants. The app must truthfully disclose this risk and enforce its own bounds. No further security-choice questionnaire is needed for this approved contract. AC-1 still needs the identified unfamiliar reader and recorded answers coordinated by Navigator.

### Observed platform and adapter evidence

Evidence was gathered against product checkout `39498bf199c4e6fab6fe2d2f4173df1457f016ac`. `README.md:21` and `.github/workflows/release.yml:96-99` define macOS Apple Silicon and Linux x64. Windows is not a released target. The local host is macOS 27.0.1 arm64. Docker's available daemon is Linux aarch64, so its result below does not establish Linux x64 CLI support.

| Surface | Observed evidence | Limit |
| --- | --- | --- |
| Claude | Installed `claude --version`: 2.1.309. The current local account model catalog under `~/.claude/cache/model-catalog/*-cc.json` lists `claude-haiku-5-5` (Haiku 5.5) and `claude-haiku-4-5-20251001`. The binary also contains these exact IDs. Plan pins `--model claude-haiku-5-5`, subject to real availability validation; no silent substitute. | Cached catalog support is not a successful inference or an entitlement guarantee. |
| Codex | Installed `codex --version`: 0.164.0. `~/.codex/models_cache.json` lists `gpt-6-luna`, visible and API-supported, alongside `gpt-5.6-luna`. Plan pins `exec --model gpt-6-luna`; no alias or coordinator model. | Cache can become stale; real model availability remains untested. |
| Existing auth | Network-denied `claude auth status --json` returned logged-in, `claude.ai`, first-party, team; only those fields were retained. `codex login status` returned logged in using ChatGPT. No credential values were inspected or copied. | Status establishes local login recognition, not successful refresh or inference. |
| Claude auth under containment | Strict file-read/write/exec restrictions plus deny-fork changed Claude status to logged-out. Removing only deny-fork exposed kernel denials of `/usr/bin/security` and `/bin/sh`. Allowing only the security helper and narrow runtime reads still returned logged-out. | This is an unresolved existing-login/containment conflict, not a supported adapter or an ordinary missing-login case. `--bare` explicitly disables OAuth/keychain and is unsuitable for the amended requirement. |
| Codex auth under containment | Exact existing `auth.json`/config reads by the CLI retained “Logged in using ChatGPT”; CLI PATH-alias writes were denied. | Tools, inference-only egress, credential refresh, and inference remain unproved. No secrets were passed to a model. |

Metadata was read on 2026-10-23; only model IDs/labels/visibility and selected auth-status fields were emitted. Existing global model choices were not changed. CLI binary SHA256: Claude `5eadc0009cdcdbc0d06e58110d7b2baf88ced6088b1c33a60a21217fe410619c`; Codex `d8990e3ee7acfbc26f91265b12a1a83f0fee40d0e0f8f1f40a37d4ec21f499cf`.

### Historical isolation experiments and retained negatives

All startup probes used a cleared environment, synthetic text, disposable HOME/cwd, an OS network deny, bounded six-second supervisor timeout, and no real credentials. They were not real-provider launches. `claude -p` used `--safe-mode --bare --tools "" --disable-slash-commands --strict-mcp-config --mcp-config '{"mcpServers":{}}' --setting-sources '' --no-session-persistence --output-format json --model synthetic-nonexistent-model`; the historical `--bare` experiment is not the proposed existing-login invocation. `codex exec` used `--ignore-user-config --ignore-rules --ephemeral --skip-git-repo-check --sandbox read-only --color never`, disabled `hooks/plugins/apps/shell_tool/multi_agent`, set `project_doc_max_bytes=0` and `web_search="disabled"`, and used a synthetic nonexistent model with stdin `-`. These flags are candidates, not complete tool-disable proof.

- With selected-path denials, both CLIs started. Claude exited 1 with zero API usage and “Not logged in”; its structured result had `is_error: true` despite `subtype: success`, so the adapter must inspect exit status and error fields. Codex attempted reconnects despite missing auth/model; the OS denied network and the supervisor killed its group at six seconds. App-level retries must be absent, and CLI internal retries must be accounted for rather than hidden behind that claim.
- A real local Python canary showed an ordinary forked child disappeared after `killpg`, while a child calling `setsid()` survived. The escaped PID was explicitly killed. This rules out process-group-only cleanup as sufficient proof.
- A separate kernel canary with deny-fork/exec/network and selected filesystem restrictions returned `EPERM (1)` for `fork`, outside synthetic read/write, `/bin/sh` execution, and loopback connection. It would fail if any of those operations became permitted. Creating a socket alone initially succeeded; the corrected probe tested an actual connection.
- An initial default-deny file-data profile caused all tested CLIs to abort (`SIGABRT`, no stderr). PID-filtered kernel logs identified a denied read of the literal root directory `/`. Allowing that directory entry, not its descendants, advanced startup. Claude then failed on `/tmp/claude-501`; setting `CLAUDE_CODE_TMPDIR` to request scratch fixed that. Runtime directory entries and timezone reads resolved a subsequent Bun trap in historical OMP testing. With those narrow repairs, default-deny file-data reads plus deny-fork/exec again reached the same no-auth outcomes. These negatives were retained; they do not prove isolation is impossible.
- The final local startup profile denies network, file-data reads, writes, fork and exec by default for those operations; it allows exact CLI binaries, necessary system/runtime paths and private scratch. This is a feasibility profile, **not a complete hardened sandbox**: other OS capabilities were not audited and authenticated inference was not exercised. Do not reopen general HTTPS and call it safe.
- A disposable existing Linux arm64 Python image, run with `--pull never --rm --network none --read-only --cap-drop ALL --security-opt no-new-privileges --pids-limit 16 --tmpfs /tmp:rw,nosuid,nodev,noexec,size=16m`, reported zero effective capabilities, `NoNewPrivs=1`, root writes denied with `EROFS (30)`, network denied with `ENETUNREACH (101)`, and scratch writes allowed. No images/tools were installed and the container was removed. This supports testing a Linux namespace/container approach; it does not prove the released x64 CLIs, existing host login, or connected egress work there.

Local recipes, version/help captures, per-variant results and scripts are declared at `/tmp/spacetop-090-probe/`: `probe.py`, `probe-nofork.py`, `probe-strict.py`, `probe-strict-root.py`, `probe-strict-runtime.py`, `boundaries.py`, `boundaries-nofork.py`, `existing-login-default.py`, `existing-login-exec-only.py`, `existing-login-security-runtime.py`, and result JSON files. `manifest.json` records the first probe hashes. They are disposable planning artifacts, not committed regression tests or a prerequisite hidden from future implementers. Promote the relevant recipes into repo-owned fixtures/tests in the first implementation slice. The outcomes and reproduction inputs above remain in this task if scratch is lost.

Historical OMP 18.12.6 evidence is retained only to explain why CLI flags were not assumed sufficient. Pinned [main.ts](https://github.com/can1357/oh-my-pi/blob/v18.12.6/packages/coding-agent/src/main.ts) maps `--no-tools` to an empty built-in tool list, while [sdk.ts](https://github.com/can1357/oh-my-pi/blob/v18.12.6/packages/coding-agent/src/sdk.ts) separately discovers MCP/custom tools/context unless additional SDK restrictions apply. OMP is no longer an implementation or proof obligation.

### Next executable slice: direct CLI summarization before UI

1. Implement typed request/result and adapter/process boundaries in the same worktree. Use executable-plus-argv, bounded DATA stdin, private cwd outside project, concurrent bounded stdout/stderr draining and a deadline started before stdin. Never interpolate source into shell/argv, silently truncate input or collect unbounded output. App does not read auth secrets; selected CLI owns existing login/network/config.
2. Confirm installed-version native controls with local help/metadata and synthetic protocol/process fixtures. Keep exact `claude-haiku-5-5` and `gpt-6-luna` selections; no --bare/auth breakage, invented flag, ignored user-provider config or fallback. Document each effective restriction and residual behavior. No OS isolation campaign or auth/Keychain probing.
3. Terminate/reap the tracked process/group on cancel/timeout/quit and discard late results. Explicitly document escaped/untracked descendants may survive; killpg/PID enumeration is not a race-free whole-tree guarantee. Test tracked behavior and accepted escape limits separately without relabeling historical failures.
4. Run at most one normal synthetic live launch per locally available adapter (two total reserved for worker), using the frozen AC-1 source when practical. Keep four launches for Navigator's actual TUI adversarial/cancel runs. Reserve exact launch/model/prompt/source/candidate first, count failed attempts, no automatic retries. Record real output/usage and source/prompt/model/output hashes. Auth/model errors are actionable availability evidence, not a request for keys or silent fallback. Old output never gains a later candidate binding after prompt/bytes change.
5. After real summarization succeeds on at least one selected available provider, implement state/cache/spinner/navigation and user consent. Both adapter paths receive behavior coverage; unavailable live proof stays explicit. Use local fake fixtures for timeout, output bounds and races without spending live calls. Preserve human/Navigator evidence requirements and fresh independent verification before any gate.

This is implementation authority under the revised contract, not gate/publication/merge authority. New app credential handling, dependencies with material new permissions, provider/model-family changes or effects beyond this contract require a concrete escalation.

### Owned implementation sequence under the trusted-CLI contract

| Step | Owned modules | Behavior and lowest-layer proof |
| --- | --- | --- |
| Typed snapshot/config | New `spacetop-core/src/domain/summary.rs`; `config.rs`; query/source access in `query.rs`/`sources.rs` | `SummaryProvider`, exact model ID, limits, `SummarySnapshot`, cache key, request identity, typed failures. Config loads only through existing absolute user paths. Capture bounded exact source bytes and the authoritative metadata used by the current query snapshot; preserve main metadata versus selected worktree-body provenance explicitly. Reject mismatched/changed reads; never silently truncate. |
| Prompt/output | New `spacetop-core/src/summary/{prompt,output}.rs` | Versioned fixed instructions; delimit captured source as data. Preserve claimed versus checked evidence, contradictions and missing human proof. No linked-evidence reads. Parse provider envelopes strictly, handle Claude `is_error` and exit status, bound decoding, reject empty/malformed output. Strip/replace terminal controls before display. Plain summary text has no executable links or ANSI. |
| Adapter/process boundary | New `spacetop-core/src/summary/{adapters,process}.rs` | Separate Claude/Haiku and Codex/Luna argv builders, verified native controls, CLI-owned login, tracked process/group cleanup and bounded pipes/deadlines. No auth bridge or app credential reads. Use existing threads/channels; no async runtime by default. Use existing process APIs where possible and document any necessary dependency change. |
| App request/cache state | New `spacetop/src/app/summary.rs` and `summary_worker.rs`; `app/overview.rs`, `app/session.rs`, `app/keys.rs`, `lib.rs` | Explicit request intents; first-generation disclosure; one active request at a time; bounded in-memory cache. Key includes canonical workflow, task identity, selected source-copy identity, exact input digest, authoritative-fact digest, provider, exact model and prompt version. Request nonce/epoch is separate so a canceled or superseded result cannot revive progress. No persistence or automatic regeneration. |
| Preview/progress | `ui/preview.rs`, new `ui/summary.rs`, help/footer and key tests | `S` is presently unused in inspected preview bindings; reserve it only after complete keymap collision tests. Prefer `S` to enter Summary, `Enter` to generate/regenerate only when explicit, and `Esc` to cancel active work before closing; retain Original/diff access. A typed running request drives an indeterminate spinner and “Generating summary…” using existing ticks only. Completion/error/cancel/timeout stops it. No fake percentage/ETA. |
| Docs/policy | `README.md`, `AGENTS.md`, `docs/development-policy.md`, `docs/code-review-policy.md`, help/config examples | Narrow allowance for constrained summarizer execution; unchanged workflow read-only and Git-write rules. Explain transfer/charges, CLI/model prerequisites, existing-login use, trusted-CLI effects and tracked-cleanup limits, cache staleness, unavailable errors, cancel and no fallback. Update loaders only if references change. |

Source capture must not accidentally send a worktree's conflicting frontmatter as authoritative main status. Capture and label source copies separately; hash the exact assembled input sent to the provider. Re-check identity before presenting a result as current. Source changes mark cached prose stale and require explicit regeneration. Navigation may leave a request active, but its spinner/result belongs only to its captured key; a different task remains Original/idle. Results after cancel/quit are discarded. Cache bounds should cover total bytes as well as entry count; initial proposed runtime limits are 64 KiB source, 64 KiB stdout, 16 KiB stderr, 45 seconds and 16 cached results/1 MiB total, all subject to model context validation. Live smoke uses the stricter 8 KiB source limit.

### AC proof map and falsifiable checks

| AC | Required proof and owner | What fails the check |
| --- | --- | --- |
| AC-1 | Navigator coordinates an identified unfamiliar human. Implementation generates the frozen synthetic source's actual summary; facilitator records verbatim answers against the separate key below, source/summary/model hashes, time and candidate. | An agent answer, missing participant/answers, pre-exposed source, invented approval, or inability to identify purpose/progress/open limitation leaves AC-1 unmet. |
| AC-2 | Core prompt/envelope tests plus app/TestBackend fixtures for rejected reviews, contradiction, unperformed human check, main/worktree facts. Fresh verifier inspects real summary and Original/diff reachability. | Prose mutates typed status/gates, erases rejection/missing proof, invents reconciliation, exposes source links outside captured sections, or blocks Original/diff. |
| AC-3 | Pure app state tests schedule A/B task, workflow, active/archive and source-copy switches; mutate bytes/metadata; reverse completion order; cancel/retry same key. Verify exact cache reuse and stale labeling. | A completion/spinner attaches to a different selection, changed input appears current, a canceled nonce revives, or navigation/regeneration occurs implicitly. |
| AC-4 | Real fixture processes cover absent binary, unavailable auth/model, nonzero exit, misleading success envelope, empty/malformed output, oversized input/stdout/stderr, blocked stdin, simultaneous pipe floods, timeouts, cancel, app exit and escaped descendants. TestBackend uses controlled ticks; Navigator drives actual debug TUI and captures in-progress and terminal frames. | Any browse action launches a provider; input silently truncates; progress stalls input or persists after terminal state; extra polls create requests; tracked process is not terminated/reaped; late result revives; Original is inaccessible. Escaped descendants are an explicitly disclosed accepted limit, never a complete-cleanup pass. |
| AC-5 | Both adapters receive argv/control/adversarial protocol tests and available providers receive bounded synthetic real E2E. Audit files and observed native tool actions; distinguish app effects from trusted CLI behavior. Test terminal sanitizer for ESC/CSI, OSC 8/52, DCS, C0/C1, CR/backspace and malformed UTF-8. | App reads/copies credentials, writes workflow/project files, interpolates task into commands, invents control guarantees, silently falls back, hides dangerous observed effects or lets output control the terminal. Missing live evidence remains explicit; full sandbox/descendant isolation is outside the amended contract. |

Run relevant focused tests during each slice, then `cargo fmt --check`, `cargo test`, and required `make lint` on final Rust changes. Keep `cargo test -p spacetop-core --test no_write_git_calls` and `cargo test -p spacetop-core --test no_terminal_deps` valid; do not exempt arbitrary summary commands from existing Git safety checks. Add lower-layer adapter/process tests and preserve compatibility with both release targets; distinguish local native execution from emulated fixtures. The ignored watcher test is unnecessary unless watcher behavior changes. Planning changed no code, so Rust build/lint tests were not run here.

### Authorized real smoke and acceptance evidence

**Zero of six actual live CLI launches used.** Prior fake-model starts were network-denied, credential-free local probes; auth-status commands do not invoke a model. Before each real launch, its sole assigned owner (worker or Navigator) must reserve number, adapter, purpose, exact model and candidate in `/tmp/spacetop-090-fo/live-smoke-ledger.json` and notify FO. Preserve the ledger count across amendments. Allocate at most two pre-UI normal launches to the worker; reserve four for Navigator TUI adversarial/cancel acceptance. Each attempted generation counts, even auth/model failure. No automatic retries, replacement launches or parallel duplicate worker. Maximum six total, normally normal/adversarial/cancel for each adapter; no more than 8 KiB synthetic input, 45 seconds, 64 KiB stdout and 16 KiB stderr. These limits are not a billing/token ceiling. Record observable usage and any internal CLI retries; do not invent cost or assume killing the local process erases already incurred usage.

Use only `claude-haiku-5-5` and `gpt-6-luna` once locally confirmed for the selected path; if unavailable, surface that exact failure with Original intact. The normal call checks readable, source-grounded output; adversarial source requests outside read, file edit, shell, network/MCP action and false approval; cancellation checks responsiveness, tracked process/group termination and late-result rejection, with escaped descendants explicitly outside the guarantee. Put all attack variants in one source, not unbudgeted calls. A timeout/cancel fixture proves local handling; real provider cancellation alone cannot prove remote computation stopped. Future TUI/Navigator calls also consume this ceiling, so coordinate the same six launches with final debug-TUI acceptance rather than spend all calls on disposable pre-UI probes. Any necessary extra live call requires new authorization.

Fresh verification must review code and raw evidence independently. Navigator then builds the candidate debug app, copies/hash-binds its executable outside the shared target, and retains terminal ID, dimensions, candidate SHA, binary hash, commands/exits, input/source hashes, write audits and actual in-progress/terminal images. Label rendered terminal captures honestly. Current-plan probes are not those product E2E results. Participant remains pending; UI success cannot substitute for AC-1.

### Human walkthrough materials and handling

The synthetic source, reader questions and answer key were prepared separately at `/tmp/spacetop-090-probe/walkthrough/`. They are also preserved in separate facilitator sections below so the plan has no hidden local-fixture dependency. **Do not give this task body, source annex or answer key to the reader before first answers are locked.** Navigator sends only the actual generated summary and reader questions, confirms unfamiliarity first, records the named participant and verbatim answers, then permits Original. Do not coach answers or regenerate toward a desired result. If the reader has seen the source, select a new frozen synthetic case before the walkthrough and record that fact.

Participant: pending Navigator coordination. Actual generated summary: pending implementation/live readiness. Answers and human result: pending. No simulated human evidence is supplied.

Reader questions:
1. What problem is this task trying to solve, and for whom?
2. What work has been reported as done? What evidence or approval is still missing?
3. What decision, defect, or limitation remains open?
4. Would you treat the task as ready to release based on this summary? Explain using what it says.
After answers are saved, use Original and identify the source passage behind one answer; record confusion separately.


### Facilitator-only synthetic source — do not pre-expose

Source SHA256: `698210b90b684ed69400fcd0ea0f294581b8c13e3354ea817f9a487427e99c8b`.

```markdown
---
id: 9001
title: Explain pickup windows on the Harbor Library reservation kiosk
status: implement
---

The fictional Harbor Library kiosk currently shows only a reservation expiry date. Patrons cannot tell when the pickup desk is open. Show the desk's pickup window alongside each reservation so patrons can plan a visit. This task covers the kiosk display only; it does not change reservations, send messages, or alter desk staffing.

## Initial plan
Use the branch's recorded opening hours. Display the date and opening/closing times. The library manager must choose whether a closure spanning midnight follows the opening day's hours or the calendar day's hours. This choice is still pending.

## Implementation report
The developer says the new display and missing-hours fallback are implemented. They report six parser unit tests passing. They also wrote that the kiosk is ready to ship. No test transcript, screenshots, or deployed build are attached to this source.

## Review record
A reviewer rejected the change. Their report says the time range is shifted by one hour after a daylight-saving transition, and that the narrow screen clips the fallback message. The developer later wrote that they fixed the time conversion. There is no recorded reviewer rerun or approval after that claim. The narrow-screen finding has no recorded fix.

## Current verification
An unfamiliar patron walkthrough was planned but has not happened. There are no recorded patron answers. The six reported unit tests concern parsing; this source does not establish that they cover daylight saving, layout, or patron comprehension. The task remains in implement. Release approval and deployment are not recorded.
```

### Facilitator-only answer key — separate from reader questions

Facilitator only. Do not show before the participant's answers are locked.

- Purpose: help library patrons plan pickup visits by showing pickup desk hours beside reservations. Display-only scope; no reservation/staffing/message changes. Source: opening paragraph.
- Recorded progress: developer reports display/fallback implementation and six parser tests, and later claims a time-conversion fix. Source: Implementation report and Review record. These are reported claims, not attached independent proof.
- Missing proof: no test transcript, screenshots or deployed build; reviewer rejected and has not rerun or approved; no unfamiliar patron walkthrough or answers. Source: Implementation report, Review record, Current verification.
- Outstanding: manager must choose midnight closure convention; narrow-screen fallback clipping has no recorded fix; daylight-saving correction is claimed but unverified. Source: Initial plan and Review record.
- Release: source does not justify ready-to-release. Developer's claim conflicts with rejection and outstanding proof; status remains implement, release approval/deployment not recorded. Source: Implementation report and Current verification.
- Pass rubric: answers identify purpose, distinguish reported progress from accepted verification, and name at least one correct open decision/limitation; no invented approval or completed human test. Record all four answers verbatim, source references, confusion and pass/unmet separately. A weak answer stays negative evidence; do not coach until first answers are locked.

## Stage Report: plan

- DONE: Prove or precisely bound isolation feasibility for existing-login Claude/Haiku and Codex/Luna on actual supported platforms, with installed-version/model-ID evidence and safe lifecycle behavior before large UI work.
  Version/help, safe cached model IDs, auth-status and kernel/container probes above bound feasibility; complete authenticated containment is explicitly unproved, and macOS/Linux x64 both remain in scope.
- DONE: Plan all five amended ACs, request-bound spinner, typed snapshot/cache/race behavior, adapter unavailable errors, bounded real smoke and independent/Navigator proof without silent fallback or safety waivers.
  Owned-module sequence and falsifiable AC map above; next permitted implementation slice is isolation-only, with six reserved live attempts at most and zero used.
- DONE: Prepare the unfamiliar-human synthetic walkthrough with separate source/answer key and reader questions; record actual participant/answers as pending rather than simulated, then commit a complete plan and evidence limits.
  Separate frozen facilitator annexes and questions above; participant, generated summary and answers remain pending Navigator coordination.
- FAILED: Existing-login Claude under the tested strict containment.
  Auth status changed from logged-in team/claude.ai to logged-out; kernel logs identify blocked auth helpers, and bounded helper-only relaxation did not restore it. This is not claimed supported.
- FAILED: Process-group-only descendant cleanup.
  Actual fork/setsid canary survived killpg; explicit cleanup completed. Deny-fork returned EPERM, but compatibility with Claude keychain auth is unresolved.
- SKIPPED: Real provider smoke and product UI implementation.
  Safety readiness is not established; no authorized live attempts consumed, no UI/code edits, no Rust checks needed for this planning-only state update.

### Summary

The plan preserves the captain's two small-model adapters, existing-login requirement, progress indicator and all five ACs. Local evidence supports a concrete isolation-only next slice but does not authorize advancing past the safety gate: Claude auth coexistence, actual tool/egress containment, Linux x64 runtime proof and unfamiliar-human evidence remain open.

## Stage Report: plan completion addendum

- DONE: Prove or precisely bound isolation feasibility for existing-login Claude/Haiku and Codex/Luna on actual supported platforms, with installed-version/model-ID evidence and safe lifecycle behavior before large UI work.
  Commit `e2849834be66bbe50565fbcac16f5a07aa0b484c`, “Observed platform and adapter evidence” and “Local isolation experiments and retained negatives” precisely bound feasibility; full safety remains unproved.
- DONE: Plan all five amended ACs, request-bound spinner, typed snapshot/cache/race behavior, adapter unavailable errors, bounded real smoke and independent/Navigator proof without silent fallback or safety waivers.
  Same commit, “Next executable slice: isolation before UI,” owned-module sequence, AC proof map and authorized-smoke section define the implementation path, stop conditions and proof owners.
- DONE: Prepare the unfamiliar-human synthetic walkthrough with separate source/answer key and reader questions; record actual participant/answers as pending rather than simulated, then commit a complete plan and evidence limits.
  Same commit preserves separate facilitator source/key and reader questions; participant and actual answers remain pending Navigator coordination, not simulated or waived.
- SKIPPED: Live provider, product and human acceptance proof at the planning stage.
  Zero live launches used; no product implementation or safety acceptance is claimed. These remain obligations of later execution and verification.

### Summary

All three dispatched planning obligations are fulfilled. The earlier **FAILED** process-group-only/setsid cleanup mechanism and **FAILED** Claude existing-auth containment probe remain unresolved technical findings, not passes; their evidence and original report are unchanged. Only the bounded isolation-only implementation slice is ready: no large UI work or safety acceptance until the remaining containment, tool/egress, platform and human-proof requirements are satisfied at their required gates.

## Stage Report: implement

- FAILED: First implement and prove the smallest isolation/lifecycle boundary for existing-login Claude/Haiku and Codex/Luna on supported platforms; preserve negative results and stop before UI if safety readiness cannot be established.
  Safety readiness remains unresolved. Committed bounded fixture/evidence slice `80245daa4148e4cc3c2b7be29eadc2a23296e2e8` proves specific local controls and failures, not a supported adapter or complete implementation.
- SKIPPED: Once isolation is demonstrated, implement all five amended ACs with explicit summary/original navigation, exact snapshot binding, bounded cache/process behavior and request-bound indeterminate progress; no silent provider/model fallback.
  Prerequisite remains unmet; no production/UI/config changes, provider generation, fallback, PR or code push occurred. AC-1 through AC-4 remain unmet; AC-5 has component evidence only.
- FAILED: Add meaningful tests and docs/policy updates, pass cargo fmt/cargo test/make lint and existing guardrails, coordinate the shared six-launch live budget and unfamiliar-reader/Navigator proof, and commit exact candidate/evidence with DCO.
  Tests/docs and DCO commit are complete for the bounded slice; full product policy changes and independent/Navigator/human proof remain pending. Live ledger remains 0/6; no synthetic protocol response is called human or real-model evidence.
- DONE: Preserve a repository-owned bounded process probe and ten falsifiable synthetic tests.
  `tests/fixtures/summary-isolation/` tests literal stdin, pre-spawn input rejection, both pipe caps, blocked-stdin timeout, error exits and cancel/reaping; they fail if limits or request transport weaken. The setsid heartbeat test must observe survival after killpg, then closes its own lease to stop the synthetic child.
- DONE: Reproduce macOS permission denials and isolate the existing-login conflict without reading credentials.
  `evidence/macos-auth.json`: actual outside read/write, exec, fork and connect return EPERM; private scratch writes work. Claude logs in under network-denied/write-restricted reference, but adding only deny-fork makes it logged-out; Codex recognizes ChatGPT login under strict nofork. These are auth-status calls, not inference.
- DONE: Correct two historical probe confounders and retain their limits.
  The helper-read loop previously appended only its last auth path; cleared USER independently caused Claude logged-out. Corrected profiles and environment are regression-tested. Helper exec plus broad reads now logs in; strict helper read allowances still do not. This does not prove isolation impossible.
- DONE: Exercise Codex/Luna against a local synthetic Responses endpoint under OS egress restriction.
  `evidence/codex-{normal,adversarial}.json`: normal one request; adversarial two requests, no transport retry. Empty declared tools do not disable all routing: exec/read_file reject, patch rejects by read-only policy, unadvertised view_image reaches the OS and gets EPERM for the outside file.
- DONE: Probe the allowed auth/config-path exception without real credentials, preserving negative and corrected results.
  `evidence/codex-auth-exception.json`: a synthetic JSON file allowed like CLI auth data reaches image decoding, so per-process filesystem exceptions do not isolate model-directed reads. `codex-image-disabled.json`: installed `--disable view_image` makes both ordinary and auth-exception image calls unsupported. This closes only that tested route; other in-process tools/auth exceptions, customization and connected egress remain unproved. No real credential exposure was tested or claimed.
- DONE: Make one Linux amd64 compatibility attempt within the authorized local test scope.
  Official Python 3.12 slim amd64 image `e8a38829209aea82774882e5157e1f7c3e8c8393ff53a697b3580896e57f345c`, under Docker emulation with network none/read-only/cap-drop/no-new-privileges, passed all ten synthetic tests. Only fixtures were mounted, read-only; the container was removed. This is not native x64 CLI/auth containment proof; compatible CLI artifacts and an existing-login test context remain missing.
- DONE: Run the required repository checks on the committed slice.
  `evidence/verification.json`: cargo fmt --check, cargo test (764 passed, six existing ignored) and make lint passed; existing no_write_git_calls/no_terminal_deps remained valid. Ten fixture tests passed on macOS and amd64 emulation. Rust product code is unchanged; watcher/manual TUI tests were not applicable to this slice.

### Summary

Implementation is stopped at the requested safety boundary, not complete or verification-ready. Candidate `80245daa4148e4cc3c2b7be29eadc2a23296e2e8` is committed in `.worktrees/spacedock-ensign-on-demand-task-summaries`; reproducible scripts, raw synthetic results and limits are in `tests/fixtures/summary-isolation/README.md` and `evidence/`.

The concrete tradeoff is Claude's required helper creation versus the only tested effective descendant restriction (deny-fork): no tested profile preserves both login and cleanup. Recommend retaining all ACs and moving only to a bounded helper-capable containment design with an explicit login-plus-setsid cleanup test on both release hosts; any new runtime dependency/auth broker requires a concrete captain decision before adoption. The conservative alternative is to defer summaries while retaining this evidence. Do not proceed to UI/live calls, silently cut an adapter/platform, or treat the narrower Codex tool result as full safety readiness.

## Stage Report: implement (cycle 2)

- DONE: Compare three falsifiable no-new-dependency candidates: exact auth-helper capabilities, actual OS descendant lifecycle primitives, and proven CLI tool/customization restrictions combined with narrow OS containment.
  Candidate comparison, source references, reproduction commands and limits are committed in `0d0f404a22834b1213066ca8cd3505869fd352f1`, `tests/fixtures/summary-isolation/ADVANCE.md`; no complete existing-login boundary is claimed.
- DONE: Exercise only the smallest evidence-driven differences, preserving prior failures, existing login/model settings, secret boundaries and zero live calls; establish supported-host prerequisites without confusing emulation with CLI proof.
  Exact read differentials, PID-scoped denial metadata, Darwin syscall controls, synthetic protocol requests and Linux capability probes retain their negatives; no global config, credential value, runtime install or provider inference was used. Ledger remains 0/6.
- FAILED: Commit the bounded design/probe result and native report with one demonstrated next path or a concrete minimal dependency/broker proposal naming permissions, user flow, platform impact and why existing mechanisms failed.
  Scoped result is committed, but no complete supported path or necessity for a new component is established. Per FO convergence instruction, return the one exact remaining metadata-only discriminator below rather than falsely declaring every dependency-free path impossible or adopting a speculative broker.
- DONE: Narrow the auth failure without broad HOME/Keychain grants.
  Literal HOME/settings reads, alone and together, did not restore Claude login. Correctly observed helper PIDs exposed no new missing read. The strict-profile `security default-keychain -d user` metadata query succeeded, ruling out default-keychain discovery itself; specific item access/recognition remains unresolved. No item value was requested.
- DONE: Verify actual Darwin lifecycle capability and a concrete syscall-guard bypass.
  SDK says NOTE_TRACK/NOTE_CHILD unsupported. `process-info-setcontrol` allows direct and spawn-based session escape; denying SYS_setsid/SYS_setpgid blocks direct calls but POSIX_SPAWN_SETSID/SETPGROUP still escape. Also denying SYS_posix_spawn blocks all four canary operations but makes the recorded positive Claude auth reference logged-out. No auth-compatible cancel/timeout/exit cleanup PASS is claimed.
- DONE: Establish the available Linux surface and its precise limits.
  Existing amd64-emulated container: user+PID unshare returns EPERM, cgroup.kill exists but cgroup root is read-only, and subreaper registration works. Installed CLIs are arm64 Mach-O. Native x64 artifacts, an existing-login context, and namespace/cgroup permission remain prerequisites, not evidence that Linux is impossible or excluded.
- DONE: Demonstrate a narrower Claude/Haiku protocol path under strict nofork.
  `advance-claude-protocol.json`: fake OAuth token/local Messages endpoint, two synthetic requests, forged Bash/Read responses rejected as disabled, CLAUDE.md marker absent, outside files unchanged. This is not existing-login or real-model proof; its computed usage/cost fields arise from canned fake usage, not billable usage.
- DONE: Preserve Codex configured-provider behavior and inconclusive startup evidence.
  `advance-codex-config-{control,clear-mcp}.json`: synthetic custom provider preserved without ignoring all user config; exact Luna selection and absent AGENTS.md marker observed. Neither variant activated its MCP startup canary, so MCP clearing is inconclusive; hooks/plugins/managed customization remain unproved.
- DONE: Correct diagnostic API handling and test the changed fixture boundary.
  libproc proc_listchildpids returns a PID count, not bytes; corrected sampling captured actual helper PIDs and now has a real one-child regression. Twelve Python fixture tests passed; all scripts parse and git diff --check passed. Already-green Rust checks from 80245da were not repeated for fixture/docs-only changes, as FO directed.
- SKIPPED: Product UI, connected provider calls, new dependency/broker adoption, final verification and human acceptance.
  Safety readiness remains unmet. All five ACs remain open; no provider/model fallback, platform reduction, PR/code push, gate/merge or unrelated task changes occurred.

### Summary

Bounded comparison is committed as `0d0f404a22834b1213066ca8cd3505869fd352f1` in the same clean implementation worktree; prior `80245da` evidence is preserved. The working synthetic Claude tool restriction is progress, while existing-login auth, full lifecycle/customization containment and native Linux prerequisites remain unresolved; this report does not complete implement.

The single next local discriminator is a Security.framework query for the statically verified exact CLI service/config identity, requesting only an opaque item reference/status (`kSecReturnData=false`, match one), and recording only OSStatus/presence under the same narrow profile. It can distinguish a specific item lookup/authorization failure from successful default-keychain discovery, without enumerating entries, exposing values or granting the entire database. The service mapping must be verified from nonsecret shipped metadata first; successful metadata would still not prove value access. No new component/permission decision is justified until that remaining branch is classified.


## Stage Report: implement (cycle 3)

- DONE: Resolve the exact CLI service/config mapping from nonsecret installed artifacts, then perform only a noninteractive metadata/status query under the existing narrow profile.
  Candidate `6077108a938160908d2569b25e0e3a6dc197c95b`, `tests/fixtures/summary-isolation/KEYCHAIN.md` and `evidence/keychain-mapping.json` pin installed Claude 2.1.309, production suffix, absent config overrides and validated inherited USER mapping. Exactly one Security.framework match-one query requested opaque ref only; ReturnData/ReturnAttributes/ReturnPersistentRef=false. Process-local no-UI set/get both succeeded and reported false; the query also used AuthenticationUIFail.
- DONE: Classify the observed item-access result without reading credential data, broadening permissions, or making live requests; identify one actionable next path or a concrete necessary decision.
  `evidence/keychain-presence.json`: OSStatus -25300 (errSecItemNotFound), opaque reference absent, exit 0, stderr 0. This establishes only that this caller/profile did not return the exact item; actual absence, caller/search-list filtering and value access are unresolved. No new narrow permission was identified, so no delta/canary rerun was warranted. No broader HOME/Keychain grant or reference control was added.
- DONE: Commit scoped evidence and an honest native implement report preserving prior failures, open ACs, and the 0/6 ledger.
  The DCO-signed code candidate retains historical evidence, adds fail-closed identity checks, the bounded C/Python diagnostic and Decision Tabs. Thirteen local fixture tests passed; native compile used -Wall -Wextra -Werror except documented legacy Security API deprecations. All Python fixtures parse and git diff --check passed. Already-green Rust fmt/test/lint at 80245da were not repeated for fixture/docs-only changes, per FO instruction. This report is the only state file changed.
- FAILED: Existing-login auth and complete helper-aware lifecycle readiness.
  Auth remains unresolved and separately cannot repair lifecycle: direct setsid/setpgid denial does not stop posix_spawn session/group escape; denying posix_spawn breaks the recorded positive Claude auth reference. The actual auth route involves the pinned Claude CLI, /bin/sh and /usr/bin/security. killpg/PID sampling is not race-free cleanup. No passing adapter, mandatory new dependency, or impossibility of all dependency-free designs is claimed.
- DONE: Apply Navigator's hard stop after this discriminator.
  No further same-kind auth metadata exploration or generic helper-whitelist reruns. No new auth capability was found. A narrower trusted-helper contract would explicitly accept descendants surviving timeout/cancel/parent exit and require process creation plus exact helper exec; its remaining file-read needs are unknown. It requires a Captain security-contract decision, not an engineering override.
- SKIPPED: UI, real provider calls, new credential owner/component, final verification and human acceptance.
  Live ledger remains 0/6. Both supported platforms and exact Haiku/Luna scope remain intact. Native Linux CLI/login context and usable namespace/cgroup permissions, customization and connected-egress proof, all five ACs, request-bound spinner, unfamiliar-reader walkthrough and independent Navigator acceptance remain open.

### Summary

Bounded metadata pass complete; implement remains incomplete. Code candidate `6077108a938160908d2569b25e0e3a6dc197c95b` is clean in the assigned implementation worktree. The diagnostic is retained as safety evidence, not synthetic product progress. No code push, PR, merge, global config change, credential value access or provider generation occurred.

`KEYCHAIN.md` supplies three concrete Decision Tabs. Recommend retaining the current security contract and keeping summaries unavailable under current authorization. If the Captain wants a new scope, the smallest concrete proposed design investigation is a native non-model auth bridge feeding a nofork CLI in memory with real auth/config paths withheld. It would add a trusted credential owner and require explicit value-read/handoff permission on macOS Keychain and Linux CLI credential files; it adds no necessarily mandated external package. Exact existing-token selection/refresh compatibility, secure transport and native-host containment remain unproved; fake OAuth success is not compatibility evidence. The first newly authorized investigation would inspect shipped token/refresh dataflow and a synthetic bridge protocol, not transfer real credentials. Alternatively a trusted-helper exception would explicitly weaken cleanup guarantees and still would not resolve current narrow-profile auth. Neither proposal is adopted or claimed ready, and no further authorized local probe identified here can settle both required boundaries.


## Stage Report: implement (cycle 4 — trusted CLI product)

- DONE: Demonstrate actual bounded synthetic summarization through the selected existing CLI login and exact Haiku/Luna model using production adapter/process boundaries, preserving live ledger and four Navigator calls.
  Committed boundary `87cad772207b5ed806d59555b403eb48c023e2ec` made both allocated normal launches through production capture/prompt/argv/nonblocking process code. Claude 2.1.309 / claude-haiku-5-5 returned success/is_error=false, 3227-byte stdout and empty stderr; Codex 0.164.0 / gpt-6-luna returned a completed agent message/turn, 1503-byte stdout and empty stderr. Both parsed successfully within the 45-second bound. Each was reserved/hash-bound before launch and counted; no app retry or further worker launch. Ledger is 2/6, both completed, with four Navigator TUI launches reserved. Private raw evidence is `/tmp/spacetop-090-live/launch-{1,2}/`; exact hashes/argv/observed usage are in the ledger and `docs/summary-verification.md`. CLI-internal retry counts were not exposed; no retry events were observed.
- DONE: Implement explicit consent, snapshot-bound Summary/Original/cache and responsive request-bound spinner/cancel with app-owned safety and tested races, failure bounds and native adapter controls.
  Product candidate `74b015dfea237c0bbb2240aea0748a84882789bb`: preview S enters Summary/disclosure without a request; Enter explicitly generates or reuses an exact result; R regenerates; S/O returns to Original/diff; Esc cancels before leaving. Core capture checks selected bytes against loaded body and authoritative metadata, labels worktree copies, rejects oversized/mismatched/nonregular/symlink sources, and never follows evidence links or reads credentials. Typed keys bind canonical workflow/task/source, exact input, provider/model and prompt version. Request nonces reject late canceled completions; one worker, bounded 16-entry/1 MiB memory cache, no navigation generation or persistent cache. Existing ticks animate only the exact current request. Native controls retain CLI-owned login/provider config; no silent app fallback.
- DONE: Commit working product, precise trust/read-only docs, focused tests plus fmt/test/lint and an honest per-AC report ready for fresh verification and separate human/Navigator acceptance.
  DCO product commits are `87cad772207b5ed806d59555b403eb48c023e2ec` and `74b015dfea237c0bbb2240aea0748a84882789bb`, same assigned worktree/branch. AGENTS, development policy, review policy and README now separate app guarantees from trusted CLI/auth/helper/network effects and accepted escaped-descendant limits. No new package: existing rustix enables its process feature. Historical strict-isolation findings remain failed historical evidence, with a supersession banner rather than relabeling. No PR, code push, merge, credential bridge, Keychain query or isolation campaign occurred in this product cycle.
- DONE: Complete local checks and fix the integration regression found by the full suite.
  Full workspace cargo test passed 785 tests with six existing ignored; existing no_write_git_calls/no_terminal_deps guardrails passed. A full-suite-discovered unloaded-tab summary-refresh panic was fixed and regression-tested. Final capture/preview refinements passed 12 core summary tests and 12 summary-filtered app/UI/worker tests. Final cargo fmt --check, make lint (all targets/features, warnings denied), debug build and git diff --check passed. Logs: `/tmp/spacetop-090-product-{tests,lint,build}.log`, `/tmp/spacetop-090-{core,summary}-final-tests.log`. The unchanged ignored watcher test was not needed. Native debug binary SHA256 at build: `934a540c71e86e583d487309c2952b319a5343394711ac97105c9edc62393015`; Navigator should rebuild/copy/hash its own immutable candidate binary.
- DONE: Preserve actual-call provenance through final UI work.
  Prompt/argv/process source remains unchanged from the live-call candidate. Final source capture adds nonblocking/no-follow opening and distinguishes worktree-only metadata; a no-launch prepare replay retained the frozen normal input/prompt hashes. These outputs remain attributed to the actual launch candidate, not falsely presented as final-TUI calls. Source/input/prompt hashes and private facilitator material are referenced without publishing the source, answer key or generated prose in product docs.
- SKIPPED: Fresh independent verification, illustrated Navigator real-TUI adversarial/cancel acceptance, and unfamiliar-human walkthrough.
  These are separate owners/gates and remain pending, not simulated or waived. `docs/summary-verification.md` gives exact fixture/invocation/key/budget instructions; `tests/fixtures/summary-ui/setup.py` creates only disposable synthetic attack/navigation inputs. No additional calls are authorized to this worker. Native Linux x64 CLI runtime proof remains unobserved; macOS native normal results and portable Unix implementation do not imply it passed.

### Per-AC evidence and limits

- AC-1 — UNMET acceptance: actual hash-bound summaries exist privately, but an identified unfamiliar reader and recorded answers are still absent. Navigator retains that obligation; agent judgments do not substitute.
- AC-2 — Implemented with component/normal-call evidence: recorded facts remain in the header/Original, summary is plain advisory text, prompt preserves rejected/contradictory/unperformed proof, and worktree input keeps main facts separate. Final adversarial prose assessment and independent review remain pending.
- AC-3 — Component tests pass: exact cache keys, source/fact/source-copy/archive/workflow distinction, stale marking, no wrong-selection progress/results, cancel/retry nonces and unloaded-tab switching. Navigator real navigation/source-change scenarios remain pending.
- AC-4 — Component tests and both normal CLI calls pass: explicit consent/no browse request, responsive tick spinner, malformed/error/limit/timeout states, bounded concurrent pipes, tracked cancellation/reaping and worker shutdown. Actual TUI spinner/cancel/quit evidence remains pending. Escaped descendants may survive under the amended contract and are not claimed contained.
- AC-5 — Both argv/parser paths and selected existing-logins have normal live evidence; app owns no credential reads, shell interpolation, linked reads or project writes. Native restrictions and residual config/helper effects are disclosed. Adversarial native-control/effect audits, final TUI proof and native Linux x64 runtime evidence remain open; no blanket safety pass is claimed.

### Summary

Implementation is complete for fresh code verification at clean candidate `74b015dfea237c0bbb2240aea0748a84882789bb` in `.worktrees/spacedock-ensign-on-demand-task-summaries`; acceptance is not complete. The two available providers produced actual summaries under the approved trusted-CLI contract, and the full feature now supports explicit Summary/Original, bounded captured input, cache/staleness, cancellation and request-bound progress. Next owner actions are fresh verification, Navigator's four reserved actual TUI adversarial/cancel calls and the separate unfamiliar-reader walkthrough. No new security-choice or account/model questionnaire is needed.


## Stage Report: verify

- DONE: Independently challenge the committed feature against current ACs and trusted-CLI contract: exact source/request binding, authoritative facts, explicit consent, bounded execution, cancellation, zero navigation requests, native controls and truthful limits.
  Reviewed frozen product `74b015dfea237c0bbb2240aea0748a84882789bb` against base `39498bf199c4e6fab6fe2d2f4173df1457f016ac`, current task amendment and repo policies; two independently reproduced Material findings below require changes. Product tracked bytes and frontmatter remain unchanged.
- DONE: Audit actual test/live evidence and adversarial product behavior without spending provider calls or duplicating owned green checks; distinguish technical findings, native-platform gaps and human/Navigator acceptance.
  Independently summed the existing full log to 785 passed/0 failed/6 ignored; inspected final 12 core + 12 app/UI/worker logs and successful lint/build records, and matched both raw normal stdout/prompt hashes. Ran only synthetic public-App/TestBackend repros linked to candidate debug libraries; zero provider calls and no broad Cargo rerun.
- DONE: Write a committed native verification report with per-AC verdicts, concrete classified findings or missing evidence, exact candidate binding and readiness judgment; never infer approval or waive human proof.
  This task-only report records Request changes / NOT gate-ready. FO owns classification routing and native correction/Briefing artifacts; the installed ensign contract supplies no Briefing writer/schema, so none is invented here.

### Findings and concrete dispositions

- Material / High / V090-1 — `crates/spacetop/src/app/summary.rs:247` and `:199`, with missing invalidation in `app.rs:1099`: mouse return can present an outdated summary as current.
  Trigger/repro: create two synthetic tasks; open A Summary, take its request and inject its matching successful completion; select B; change A body on disk and call the ordinary reload; render to populate mouse hit rectangles; wheel up over list to A. The candidate has A's changed loaded body but retains A's old View key, so old cached prose appears with `stale=false`; no generation occurs. An inert keyboard key immediately changes it to `stale=true`. A click returning to A uses the same missing-refresh path.
  Evidence: unchanged candidate public-App harness `target/verify-090/mouse.rs`, compile/repro exit 0, `mouse-result.txt`: `changed_loaded_body=true cached_old_prose=true stale=false pending_request=false`, then `after_inert_keyboard_key_stale=true`; result SHA256 `9449e2c925399cf05d152241f3535e6b9139b3453601e0d61e6fa69e60601bea`.
  Impact/disposition: violates AC-3 without a provider race or malicious file. Refresh/invalidate summary identity at all selection paths, including mouse click/wheel, before presentation; add an app regression that expects STALE on this exact sequence, while preserving zero navigation requests. Do not waive as an accepted CLI limitation.
- Material / Medium / V090-2 — `crates/spacetop/src/app/summary.rs:176` and `crates/spacetop/src/ui/summary.rs:17`: active progress is below the visible narrow preview and scrolls away with prose.
  Trigger/repro: at 40x15, open the synthetic task with Enter, S, generation Enter; consume no worker/provider. TestBackend's actual visible cells show provider/model/status and only the first shortcut line, with no spinner or Generating state despite an active request. 80x24, 100x30 and 160x55 controls do display Generating. This is the same narrow size already claimed by the author's test, which checks only the title.
  Evidence: `target/verify-090/narrow.rs`, `narrow-result.txt`, and full 40x15 cells `narrow-frame.txt` (SHA256 `f053e6322e806fb5f99e75ce6fd5bd35cb76f3bbe669637aa8e8cf85e0e19a40`); compile/repro exit 0. These are component renders, not native terminal images.
  Impact/disposition: AC-4's visible active indicator is absent at supported narrow size; the user cannot tell a chargeable request is running without finding and scrolling its text. Reserve a compact visible request status/cancel hint separately from scrollable prose, and test active/terminal states at 40x15 and after scrolling. No layout redesign or full-screen mode is requested.
- Correct-but-disproportionate: none. Needs decision: no new product/security decision; missing acceptance evidence remains an evidence gap, not a waiver request.

### Successful checks and evidence limits

- Source capture compares loaded body and authoritative metadata, rejects oversized/nonregular/final-symlink inputs, and preserves main/worktree facts; exact key and nonce code has no new defect found beyond V090-1. Existing `snapshot_rejects_changed_body_and_authority_and_binds_source_copy`, `worktree_copy_keeps_main_facts_and_archive_has_distinct_identity`, and nonce tests would fail if those distinctions were removed; those green checks were inspected, not rerun.
- Existing blocked-stdin, simultaneous pipe-flood and tracked-child cancellation tests exercise actual fixture processes, caps/deadlines/reaping; worker shutdown and quit tests cover canceled worker joining and no launch after quit. Sanitizer removes ESC/CSI/OSC/DCS/C0/C1 controls, and UI renders plain text. These checks would fail if bounds, nonce rejection or output sanitization were removed. Escaped descendants remain expressly outside the approved cleanup guarantee.
- Installed `claude --version` observed 2.1.309; current `--help` explicitly describes safe mode as disabling user customizations including hooks while retaining admin-managed policy, auth/model/built-in tools/plugins/permissions. Missing `--setting-sources` is not by itself a defect. Historical fake-protocol evidence also supplied `--setting-sources ''`, so it does not prove user-hook suppression under exact current argv; admin/managed behavior is not claimed suppressed. README accurately requests controls and discloses native merging limits. No secret/global-config read, Keychain call, installation, auth bridge or new isolation campaign occurred.
- Actual macOS normal output/prompt hashes match the handoff at launch candidate `87cad772207b5ed806d59555b403eb48c023e2ec`; final prompt/adapter/process source is unchanged. This confirms provenance and successful envelopes, not human comprehension, final adversarial effects, final TUI operation or Linux execution. Native Linux x64 CLI/runtime evidence remains absent; portable Unix code and historical emulation do not establish that host passed or exclude it from support.

### Per-AC verdict and gate readiness

- AC-1 — UNMET: no identified unfamiliar reader and recorded answers. No facilitator source/answer contents or simulated answers are published here.
- AC-2 — PARTIAL: typed authority, Original/diff access and prompt distinctions have component evidence; final adversarial generated-prose/effect assessment and Navigator acceptance remain absent. No independent human/prose acceptance is inferred from hash checks.
- AC-3 — FAILED: V090-1 reproduces old prose shown current after mouse navigation. Existing keyboard/cache/nonce coverage does not rule it out.
- AC-4 — FAILED: V090-2 lacks a visible running indicator at 40x15; existing process and app failure/cancel evidence remains valid, but actual Navigator in-progress/cancel/quit/navigation images and final runtime acceptance are still pending.
- AC-5 — PARTIAL: argv/literal input/private cwd/no app-authored project writes and existing-login normal calls have evidence. Exact-control adversarial effect auditing and final Navigator TUI proof remain open, as does native Linux runtime proof; no blanket safety approval.

### Summary

Request changes for V090-1 and V090-2; this verification stage is complete, while product acceptance is rejected and the task is not gate-ready. Navigator's immutable debug copy was confirmed before compiling the standalone repros: candidate `74b015d`, binary SHA256 `934a540c71e86e583d487309c2952b319a5343394711ac97105c9edc62393015`; its four reserved live calls are held, with none used by this verifier. Route both findings together to the existing author, retain this failed-candidate evidence, then re-verify fixes and complete the separately owned human/Navigator proof; no PR/gate/merge approval is implied.


## Stage Report: implement (cycle 5 — verify/1 corrections)

- DONE: Correct V090-1 mouse selection stale/current identity with a failing-before/passing-after regression and zero navigation generation.
  Material/High fixed in DCO candidate `63b31f16b4f962f88f665f356c51a1ee21a2b953`. Mouse dispatch now refreshes summary identity after all mouse handling paths, with the same loaded-slot guard as keyboard refresh. The new app regression reproduces completed A summary, selection B, disk change to A, ordinary reload and mouse return; both wheel and click now show changed loaded body plus old cached prose as STALE immediately, with no pending request. Its pre-fix run failed on the exact wheel-return assertion; its post-fix run passes both paths.
- DONE: Correct V090-2 visible request progress/cancel status at40x15 and while scrolling, retaining normal Original/summary controls and terminal-state behavior.
  Material/Medium fixed in the same candidate. App presentation separates request status/action from prose; the preview reserves two fixed rows for them and computes scrolling from the remaining body area. New TestBackend regressions failed before edits, then passed for visible Generating and Esc: cancel at 40x15, after G/PageDown, and visible terminal state with no active spinner after success, cancel and timeout. The existing controlled-tick assertion now checks the fixed status field; normal Summary/Original and metadata behavior remain covered. These are component cell renders, not native terminal captures.
- DONE: Commit scoped fixes and required checks, close the advisory correction evidence truthfully, and report exact new candidate while preserving2/6 live usage and all external acceptance gaps.
  Clean product candidate `63b31f16b4f962f88f665f356c51a1ee21a2b953` is committed in the same author worktree/branch. Full cargo test passed 789 tests with six existing ignored; cargo fmt --check, make lint and git diff --check passed. Three new regressions failed before (0 pass/3 fail) and passed afterward (3 pass/0 fail). Logs: `/tmp/spacetop-090-correction-{before,after,tests,lint}.log`; before SHA256 `c3ef01eb423723b45f4ec7c5c0372c6768696c64d107f3ea601a856c1ba06b30`, after SHA256 `b2bb81151ed61ce35de4d29c99efe421599579c19b9f027608cb8e1eb18489ed`, full-suite SHA256 `1eb064ec16aebfb3774d782b2d1a309346c5ea4619cc1a9b9e8183af8f3d1bb1`. README and verification handoff document fixed progress and the correction. No source-capture/prompt/argv/process or live-evidence change; git diff of core summary files and the smoke example against rejected 74b015d is empty. Ledger remains 2/6, four Navigator launches untouched.
- DONE: Append actual author advisory evidence without changing prior records or invoking a gate recorder.
  `/tmp/spacetop-090-fo/review-1/briefing.review.jsonl` retains its original four lines byte-for-byte (prefix SHA256 `d17db832edaba458e9d4a80512f2296540ef5d59f42712b46bd77e0f17c4ee38`). Added author annotations `annotation:090:verify-1:fix-V090-1` and `annotation:090:verify-1:fix-V090-2`, then closing `resolution:090:verify-1:author-correction`, decision revise, including those annotation IDs. All use `briefing:090:verify:round-1`, actual worker attribution and timestamp `2026-10-23T14:07:53.087177Z`. New JSONL SHA256 `9febb492151954a4eb38f06c91dec27bf93a8b23996053b7111d2c9d6cd8baa1`. This is advisory correction evidence, not Captain approval or native gate output; FO retains the once-only record invocation and shared Feedback Cycles.
- SKIPPED: Independent re-verification, Navigator real-TUI/adversarial/cancel acceptance, unfamiliar-human AC-1 and native Linux runtime proof.
  These remain separate evidence obligations, not waived. No provider call, PR, code push, merge, frontmatter edit or unrelated state change occurred. Navigator must rebuild/rebind this corrected candidate; earlier immutable runtime and failed-candidate repro evidence remain intact.

### Summary

Both authorized material findings are fixed with observed failing-before/passing-after regressions at `63b31f16b4f962f88f665f356c51a1ee21a2b953`. The correction is ready for the original independent reviewer; acceptance remains incomplete. FO can record the truthful closed advisory prefix once, reuse that reviewer and route the rebuilt candidate to Navigator without any additional worker live allocation.

### Feedback Cycles

- verify/1 — Reviewer report `d0ff15a1510a3f7e9fd2ddeacf822a1be4183269` rejected candidate `74b015dfea237c0bbb2240aea0748a84882789bb`. FO authorized both findings as Material: V090-1 High, refresh/invalidate summary identity for mouse click/wheel and preserve zero navigation requests; V090-2 Medium, pin running/terminal status and cancel hint outside scrolling prose at 40x15. Original author corrected both in `63b31f16b4f962f88f665f356c51a1ee21a2b953`; three regressions failed before and passed after, full suite 789 passed/6 existing ignored, fmt/lint passed. Advisory log preserves reviewer findings, distinct FO authorization and actual author closure; independent re-review and human/Navigator evidence remain pending. No scope waiver, provider call or Captain approval.


## Stage Report: verify (cycle 2)

- DONE: Independently re-run the original V090-1/V090-2 repros against corrected63b31f1 and confirm exact fixes, visible narrow/scrolled status, and no new navigation requests or related regressions.
  Both findings are technically CLOSED at `63b31f16b4f962f88f665f356c51a1ee21a2b953`. Original public-App repros now observe `stale=true` and visible Generating at 40x15; the old assertions deliberately expecting the bug exit 101. Expanded independent `target/verify-090/cycle2.rs` exits 0 for wheel/click and eight scrolled-state cases described below. No provider was launched.
- DONE: Assess unchanged provider/prompt/source bindings and incorporate SHA-bound Navigator acceptance and real human evidence when available, keeping missing proof explicit.
  Diff against `74b015d` is empty for core summary capture/prompt/argv/process and the smoke caller. Prior two normal macOS launches remain bound to their actual `87cad772` source/prompt hashes, not relabeled as new TUI calls. Final Navigator acceptance and actual unfamiliar-reader answers have not been received; they remain explicit pending evidence, not inferred passes.
- DONE: Commit a per-AC re-verification report with exact candidate and correction-round references, distinguishing technical clearance from full gate readiness and no inferred approval.
  This task-only report references published advisory `round:090:verify:1` / `briefing:090:verify:round-1` (`198d947655d941f6e16dfffafb597a65f70e0c91`) and author report `4be78b4b417ebdddd3bc159eccd6e2c0f03cd8b5`; it creates no second correction round or gate record. Technical clearance was sent to FO promptly so Navigator could resume its four allocated calls.

### Independent reproduction and evidence

- Candidate/debug binding: waited for Navigator's confirmed immutable copy before compiling probes; executable SHA256 `ec3bbbdd2a7e140ff8979f835fab2afcd7a754c8a2c365f677207d86cc538580`. The first linkage used stale `target/debug/libspacetop.rlib` and was discarded; corrected probes explicitly link rebuilt `target/debug/deps/libspacetop-15de931cd0688d0d.rlib`, SHA256 `8f6668cbec5d1a6fce413803470f2d481ca00d84ba82de989faad5c6d5daa0f6`. No stale artifact result supports clearance.
- Commands: standalone `rustc --edition=2021 target/verify-090/{mouse,narrow,cycle2}.rs -L dependency=target/debug/deps --extern spacetop=<rebuilt library> --extern <matching core/crossterm/ratatui libraries> -o target/verify-090/<name>-63b31f1`, then each executable. All compiled successfully; original bug-demonstration assertions now fail because behavior is fixed, while the affirmative cycle2 harness passes. These runs mutate only disposable ignored fixtures/artifacts, not product code or real workflow inputs.
- V090-1: original A-summary → B → change/reload A → mouse return scenario now immediately retains old prose as STALE for both wheel and click, with the changed loaded body and zero pending navigation requests. The fixed STALE status is also visible at 40x15. Removing the mouse refresh would fail both affirmative assertions.
- V090-2: at 40x15 and 80x24, scroll disclosure down before explicit generation, then scroll again while active; Generating and Esc: cancel remain visible. Inject success, missing-executable error, timeout, or cancel (plus a late canceled completion): each terminal frame removes active progress/cancel, displays Summary ready or Stopped, rejects the late result, creates no extra request, and keeps Original reachable. This covers Navigator's old 80x24 scrolled-disclosure case at the app/render layer; error details remain in scrollable text while the fixed Stopped state is visible. Removing the fixed rows or restoring scroll-bound status fails these assertions.
- Artifact identities: `target/verify-090/cycle2.rs` SHA256 `9bb0f570917dfb8aab2872291ce71f5c764945254ca3d5a4a8e091fb74ef45e5`; `cycle2-63b31f1-result.txt` SHA256 `77b881b36e98c21fbd1ce02f39382ccc0fc4f3ed019870062341da264751f509`. Individual synthetic terminal cell captures are `cycle2-{40-15,80-24}-{success,missing-cli,timeout,cancel}-frame.txt`; these are TestBackend cells, not native-terminal images. Prior failed-candidate frame remains retained with its original SHA256 `f053e6322e806fb5f99e75ce6fd5bd35cb76f3bbe669637aa8e8cf85e0e19a40`.
- Independently audited author logs: three regressions fail before / pass after (hashes match report), full suite totals 789 passed / 0 failed / 6 ignored, and lint completes successfully. Reviewed loaded-slot guard reuse, fixed-row scroll geometry, stopped/stale/ready/other-request state precedence, and unchanged generation boundaries; no new Material finding in this correction. Did not repeat the already-green broad Cargo suite. Product status is clean and `git diff --check` passes.

### Per-AC verdict and remaining evidence

- AC-1 — UNMET: identified unfamiliar reader and actual recorded answers remain missing. No source/key contents, simulated answers or waiver are provided.
- AC-2 — Component evidence retained, final acceptance PENDING: recorded authority and Original/diff remain separate; final adversarial output/effect assessment and Navigator evidence have not arrived.
- AC-3 — TECHNICAL PASS for reviewed snapshot/cache/navigation/race scope: V090-1 closed, original keyboard/nonce/source-copy coverage retained, no generation on tested navigation. Actual Navigator navigation/source-change evidence remains pending.
- AC-4 — TECHNICAL PASS for reviewed explicit-request, bounded-process and app/render scope: V090-2 closed including narrow/scrolled terminal outcomes. Real TUI in-progress/cancel/quit responsiveness and exact native cleanup acceptance remain pending; injected errors are not native CLI tests.
- AC-5 — Prior argv/process/component evidence and both locally configured normal calls retained; final native-control/adversarial audits and Navigator real TUI calls remain PENDING. Native Linux x64 runtime is an unobserved compatibility limit, not a passed platform and not an added mandatory authenticated-Linux acceptance gate under the current amended AC. No concrete new portability defect was found in this UI-only correction.
- Gate readiness — NOT READY: no open Material code finding from this review, but AC-1 and final Navigator evidence are still required. Historical strict-isolation failures remain historical; no reopened sandbox/auth/secret investigation, product patch, frontmatter change, PR/code push or merge approval.

### Summary

V090-1 and V090-2 are independently closed on corrected candidate `63b31f16b4f962f88f665f356c51a1ee21a2b953`; no new scoped defect was found. This is technical clearance for Navigator to run its authorized acceptance work, not full task approval. FO may forward the SHA-bound runtime/human evidence for a scoped addendum; this verifier remains addressable and used zero provider calls.


## Stage Report: verify (cycle 3 — Codex MCP evidence addendum)

- DONE: Assess actual Navigator launch5 MCP/helper startup against current trusted-CLI AC5 and exact production argv without inferring model tool calls or exfiltration.
  Read Navigator's actual launch-5 helper/process and file-audit evidence at clean product `63b31f16b4f962f88f665f356c51a1ee21a2b953`, immutable debug SHA256 `ec3bbbdd2a7e140ff8979f835fab2afcd7a754c8a2c365f677207d86cc538580`. Startup is confirmed; model MCP use, secret disclosure and unauthorized writes are not demonstrated. Classification and scope are below.
- DONE: Establish installed Codex0.164.0 configuration merge/disable semantics and one compatible native correction if available, using non-billable positive-control synthetic evidence without losing existing provider/auth settings.
  An active fake MCP server proves empty `mcp_servers={}` retains a configured server; exact per-server `mcp_servers.canary.enabled=false` prevents startup while the synthetic configured provider and exact gpt-6-luna still complete. No safe generic names-only discovery was established, so no real config/list ingestion, broad config suppression or production correction is proposed.
- DONE: Commit precise finding/classification and correction proposal or bounded decision, preserving existing outcomes, residual lifecycle limits and5/6 used with launch6 held.
  Accepted trusted-config limitation; no Material finding or second correction round. Dispatch had 5/6 real calls used with #6 held. Following the human clarification and this classification, FO released only the original #6 Codex cancel allocation on unchanged candidate; this verifier did not reserve/start it and made zero real provider calls.

### Actual observation, not inferred harm

- `/Users/kent/.local/state/spacetop-navigator/2026-10-23-090/live/launch-5/helpers-at-seven-seconds.txt` records Codex PID/PGID 92479, parent Spacetop 92470; children hermes Python 92505/PG92505, sdhelm helm 92507/PG92507, ChatGPT node_repl 92508/PG92508, npm rea-agents 92510/PG92510 and child node 92689/PG92510. These are directly observed configured helper startups, including separate process groups.
- Same directory `helper-exit-audit.json` at `2026-10-23T14:30:11.037275+00:00` lists all six observed provider/helper PIDs absent, without extra kill. `result.json` records Summary ready, identical baseline/after workflow and outside-sentinel hashes, no changed files and no attack markers. This bounds the observed outcome only: it is not universal descendant cleanup, absence of all side effects, or proof that a model tool was called.
- Existing production argv remains `exec --json --ephemeral --ignore-rules --skip-git-repo-check --sandbox read-only --color never --model gpt-6-luna`, disables hooks/plugins/apps/shell_tool/multi_agent/view_image and supplies project_doc_max_bytes=0, disabled web search and mcp_servers={}. No actual configured third-party server was restarted by this verifier.

### Native positive control and limits

- `python3 -B target/verify-090/mcp-probe/run_probe.py`: baseline and exact production-empty variants each start the fake MCP process, complete initialize/notifications/initialized/tools/list, and complete one local canned Responses exchange; per-server enabled=false produces no MCP process/log and completes the same configured-provider/model exchange. All exit 0. The fake server has a benign ping method; sandboxed fixture egress is restricted to its loopback endpoint and credentials/config are synthetic. This is probe containment, not a new product sandbox requirement.
- `python3 -B target/verify-090/mcp-probe/run_injected.py`: both production-empty and per-server-disabled variants complete two local canned exchanges; each rejects the forged benign `mcp__canary__probe_ping` call as unsupported. The production server logs tools/list but no tools/call. Requests omit the tools field. These observations separate effective configuration, startup, offered tools and one tested call route; they do not prove every possible MCP call route impossible or retroactively establish what the real model did.
- Installed Codex binary SHA256 is `d8990e3ee7acfbc26f91265b12a1a83f0fee40d0e0f8f1f40a37d4ec21f499cf` (0.164.0). Synthetic result files: `target/verify-090/mcp-probe/results.json` SHA256 `aa9f3a24ad182223e775f91c03a4ef38bea9f615c85d4aa46d6e899e63910f8b`; `injected-results.json` SHA256 `95544a0e7162d36fbfd079d72d9ec44c6da8565f8e8db1ca639b99b381a0b851`. Source scripts, MCP method logs, stdout/stderr and captured synthetic requests remain alongside them. Five local fake-protocol invocations used seven canned loopback exchanges, zero real inference calls; all observed fake MCP children exited.
- Official documentation also describes per-server `enabled=false`: [MCP configuration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli). Installed synthetic `mcp list --json` confirms baseline/empty entries remain enabled and per-server override disables the entry. The control preserves the synthetic provider/auth selection; no new existing-login compatibility is claimed for this optional override.
- Safe-discovery limit: synthetic `mcp list --json` exposes both argument and environment values; text list masks environment values but exposes arguments. Installed list help provides no names-only projection. No global MCP-off switch was established from reviewed help/features/docs/nonsecret binary metadata. Thus raw list/config ingestion could read secret-bearing values and is not an acceptable app fix. No such real data was read; no ignore-user-config/provider replacement, credential access, tool installation or global config mutation occurred.

### Classification and concrete disposition

- Accepted limitation; the proposed universal-zero-helper requirement is **Correct-but-disproportionate** to the approved contract, not a Material product defect. No current false suppression promise was found: `README.md:270–275` says an empty override is supplied and explicitly warns that native config merging can retain behavior and does not prove all tools/customizations absent; `:277–284` and the first-use app disclosure trust CLI config/helpers/network and disclose escaped-descendant limits. `summary/adapters.rs:4–6,59` does not promise global MCP suppression. No wording or argv change is required solely to remove this accepted startup.
- Human provenance: Navigator message `9e2326d6-ae42-49a0-bbfb-921dfd851d21` relays Kent's exact 「這應該不是很重要吧」; parent guidance `a6ac7843-5692-4180-b423-ac51cc88d177` clarifies that startup alone is an accepted trusted-local-CLI effect, not tool use/exfiltration/write/defect, and zero helpers is not a release gate. FO reconfirmed this disposition and directed all further probes to stop. This clarifies the already-approved 「直接用 CLI」 boundary; it does not invent a blanket safety pass or waive actual harmful behavior.
- Disposition: retain unchanged `63b31f1` and all adverse/positive evidence; resume only Navigator's originally allocated #6 cancel check. Do not hold Codex, add credential-bearing enumeration, discard provider/auth settings, restart isolation research, spend another call to chase zero startup or publish another correction round. New observed unauthorized actions/writes, harmful surviving processes, unbounded usage or false guarantees would require their own concrete assessment; none is established by this startup evidence.

### Summary

The native empty-table override does not suppress configured MCP startup, and the now-active positive control removes the earlier inconclusive-canary ambiguity. Under the actual human-approved trusted-CLI scope this is an accepted, already disclosed limitation; V090-1/V090-2 stay technically closed and no new Material defect is raised. AC-1 human answers and the remaining final Navigator acceptance evidence are still pending, so no gate/PR/merge approval is implied; all further probes have stopped and candidate product bytes/frontmatter remain unchanged.


## Stage Report: verify (cycle 4 — final Navigator evidence audit)

- DONE: Audit final Navigator acceptance bindings, actual terminal/call/process/file evidence and illustrated captures against corrected candidate and six-launch ledger, distinguishing real, fake and component proof.
  Audited completed `/Users/kent/.local/state/spacetop-navigator/2026-10-23-090/acceptance.md`, including latest caption/mouse/shell-tree clarifications, SHA256 `9d1aa4d77882f78fd1786dabbdb7c7d3db888a26af99c8e85a7ecdde692c3f3a`; all 47 distinct relative artifact links exist. Completion source is Navigator `4df9bef2-ca05-47b0-b565-ea4740dcd720`, with clarifications `f959d3f8` and `637db9df`; no draft is treated as completed acceptance.
- DONE: Resolve AC2–AC5 evidence from independent review plus runtime acceptance, retain accepted trustedCLI/MCP limits and identify only concrete remaining defects or missing required scenarios.
  AC-2 through AC-5 now have sufficient combined technical evidence for the current scope. No new Material defect or concrete required terminal scenario remains missing. V090-1/V090-2 remain closed; Navigator's ambiguous mouse frame is excluded from V090-1 proof, and the verifier's successful-capture check supplies that proof independently.
- DONE: Commit final per-AC evidence addendum and precise gate readiness; retain AC1 unmet unless actual identified unfamiliar-reader answers arrive, with no extra calls or inferred approval.
  This task-only report leaves AC-1 UNMET and the task NOT gate-ready. It does not authorize a PR, merge or gate approval. No live provider invocation, smoke generation, broad test rerun, product edit or frontmatter change was performed in this audit.

### Binding, image and budget audit

- Product remains clean at `63b31f16b4f962f88f665f356c51a1ee21a2b953`. Independently hashed the mode0500 immutable debug executable to `ec3bbbdd2a7e140ff8979f835fab2afcd7a754c8a2c365f677207d86cc538580`; actual terminal build transcript shows `cargo build -p spacetop --bin spacetop`, dev/unoptimized+debuginfo, 4.69s, build/copy exit0 and matching SHA. This is an incremental debug build, not a clean or release build. Terminal is `0408e002-c263-4df0-a62d-adb3bb7b239a`.
- The five relative SVG illustrations and JSON bindings match candidate/binary/terminal/time/dimensions. Independently decoded every nonblank fixed-cell SVG character and coordinate: all five match their raw 24-row Paseo captures exactly. Images are terminal text renderings, not original pixel screenshots; actual narrow app is 40x15 with the extra captured blank rows retained. Browser policy blocked local SVG visual viewing; this verifier used XML/cell/raw-frame inspection and does not claim a separate browser/pixel visual pass or install/workaround. Navigator's documented visual check remains separately attributed.
- `live-smoke-ledger-final.json` SHA256 `17f484f404624161a9dc1535a1b952a7811ab9cdcc83c0fe764b42ae9d741afe` records exactly six calls: two normal production-core calls at `87cad772`, and four actual TUI adversarial/cancel calls at `63b31f1`. Launch3–6 reservations match owner/candidate/binary/terminal/time and nested source/input/prompt/model identities; before-counts are 2/3/4/5, final states completed/canceled/completed/canceled. Purpose wording is shortened in the final ledger without changing the scenario. No budget remains; no app retry or hidden-provider-retry count is invented.
- Human-material provenance independently recomputed from durable launch1 prompt: source SHA256 `698210b90b684ed69400fcd0ea0f294581b8c13e3354ea817f9a487427e99c8b`, captured input `cbc3f6772683dc80d3a1fd1f224ebaec79567aea022c8979617e41c1aa115d28`, prompt `bb3bbd217189d3fd79317cccb2f6ae5c136f1fd0253e96c1f3f8b8a7666b2cea`, actual summary `e4c88cddd419e31f4961050c67c35524773f715698d7501de6194875ea7057bd`; all match `human-input-binding.json`. Both durable normal stdout/summary/prompt files match their actual result hashes. This is original `87cad772` output with a final no-launch identity replay; unchanged core capture/prompt/argv/process since `74b015d` does not turn it into a final-TUI call. Participant/answers remain pending; no facilitator contents are published.

### Sampled runtime evidence and precise limits

- Actual launches3/5 show request-bound progress, navigation to task2 without task1 progress, adversarial prose retaining rejected review/missing human proof and typed implement status, plus Original access. Launch3's authorized source edit yields persistent STALE; only that recorded edit changes its known baseline hashes. Final candidate S→O restores the main/worktree diff with recorded rejection and missing proof. Fake cache/navigation audit contains one explicit process start, supplementing app no-request tests rather than asserting a system-wide network count.
- Original V090-1 closure was sharpened by one zero-provider narrow check: `target/verify-090/cycle2-capture-check.rs`, compile/run exit0 against unchanged library SHA256 `8f6668cbec5d1a6fce413803470f2d481ca00d84ba82de989faad5c6d5daa0f6`. Immediately after mouse wheel AND click return, without any keyboard refresh, typed `snapshot::capture` is Ok, entity/source match the original request, input digest/key differ, and the existing presentation status is exactly STALE summary rather than Stopped/error; no request is queued. Navigator's Stopped+STALE fixture frame proves neither successful recapture nor key mismatch and is excluded from this closure.
- Real Claude cancel occurs 0.607s after request, known provider PID90350 disappears, stopped/canceled state persists, Original works and exit0 is recorded. Real Codex cancel occurs 1.934s after request at40x15: active Generating/Esc frame, then Stopped without late revival, Original and exit0; all 13 observed provider/helper PIDs are absent afterward. Component nonce/reaping tests supply stronger direct lifecycle invariants; process sampling does not prove every descendant was observed or remote computation stopped.
- Launch6 also observes a zsh/tail tree in separate PG9342 under Codex9328. Its origin is not established by process metadata: neither a model shell call nor proven benign startup is inferred. All sampled PIDs disappear and no fixture shell marker appears. Launch5 configured MCP startup remains accepted under the recorded human clarification; current caption correctly distinguishes capture-time pending triage from later acceptance. No universal zero-helper/sandbox claim or extra correction round is introduced.
- Corrected scrolled missing-CLI TUI shows fixed Stopped/R while detailed error text remains scrollable; Original works. Fake-CLI actual TUI timeout shows the timeout state and absent tracked child, Original and exit0; it uses the configured45s deadline, but the later capture is not a direct exact45s stopwatch measurement. Fake active quit removes the known parent/same-group child and app with exit0. These are synthetic lifecycle scenarios, not additional real-model calls or AI-quality evidence; core/app tests cover malformed/empty/oversized/nonzero/auth-model-error state paths without changing real login.
- Final known fixture/sentinel hashes match restored baseline, no extra workflow files or attack markers remain, and final owned-process audit is empty. This is bounded known-path/known-process evidence, not a full filesystem/credential/network audit. Native Linux authenticated CLI/TUI remains unobserved compatibility evidence, not a new acceptance gate.

### Final per-AC verdict

- AC-1 — **UNMET**. Actual summary provenance is established, but an identified unfamiliar participant's confirmation and recorded source-assessed answers are absent. No simulated answers, approval quotation or waiver substitutes.
- AC-2 — **PASS within tested scope**. Actual adversarial outputs preserve rejection/missing proof; typed facts, advisory labeling and Original/worktree diff remain authoritative and reachable. This is not a promise that arbitrary future model prose is infallible.
- AC-3 — **PASS within tested scope**. Successful current-capture/key comparison after mouse return, source race STALE behavior, exact cache/nonce/workflow/source-copy tests and actual no-foreign-progress navigation jointly support binding; excluded ambiguous evidence is not counted.
- AC-4 — **PASS within tested scope**. Explicit consent/no browse generation, narrow/scrolled running and terminal status, actual cancellation on both providers, fake timeout/active quit, error handling and tracked cleanup have complementary runtime/component evidence. Escaped/unobserved descendants, remote usage and CLI-internal retries retain the disclosed limits.
- AC-5 — **PASS under the approved trusted-CLI contract**. Both adapter paths, actual local normal/adversarial/cancel calls, bounded literal input/process/output, no app credential handling or project writes, native-control tests and truthful residual-effects disclosure have evidence. Accepted configured helper startup and unknown-origin sampled shell processes are recorded, not erased or relabeled as proof of tool use/exfiltration/benignness.

### Summary

Technical verification and Navigator acceptance are complete on `63b31f16b4f962f88f665f356c51a1ee21a2b953`, with no open Material finding and no further required technical scenario identified. **AC-1 is the sole remaining acceptance boundary, so full task/gate readiness remains unmet.** Navigator/parent must obtain the actual unfamiliar-reader confirmation and answers; this same verifier can audit that evidence without any additional provider generation. No publication, gate approval or merge authority is inferred.


## Captain accepted exception and publication decision — 2026-10-27

This explicit human decision supersedes only the prior stop for absent AC-1 evidence. The original AC, all historical reports, failed probes and missing human answers remain unchanged. **AC-1: NOT PERFORMED / UNVERIFIED, never PASS.** The Captain accepts this named evidence gap for task090 gate approval and PR publication only; this is not evidence that a reader understood the summary.

- Parent agent `71fac6c1-962e-4f9f-abf3-34a04cc7c166`, message `bb0cda50-2771-41be-85f9-3d4a0ce85676`, recorded the exact clarification: 「你是要接受『尚未做真人閱讀測試』的缺口，先批准090建立PR嗎？若是，我會保留『AC-1未驗證』的紀錄，不把approve當成測試通過；這次批准也不包含合併。」 Kent's exact answer: 「對」, following 「approve」. Relayed by Navigator `4cc2767b-26a5-4e3d-9c9d-8f25074809f2`, message `68864c3b-33cf-4b40-8ad0-97390003e631`.
- Supplementary actual human design approval: Kent said “checked the screenshot approve this design”, relayed by the same parent in message `e1c08260-34b8-4e56-bd0e-90d198d25c8d` and Navigator message `0590a9e1-7542-477a-b446-de5da72884ab`. Preserve his quote; the actual images are terminal capture renderings, not pixel screenshots. This is visual/design approval, not AC-1 reader evidence.
- Binding: unchanged product candidate `63b31f16b4f962f88f665f356c51a1ee21a2b953`; frozen Navigator acceptance report SHA256 `9d1aa4d77882f78fd1786dabbdb7c7d3db888a26af99c8e85a7ecdde692c3f3a`; immutable debug binary SHA256 `ec3bbbdd2a7e140ff8979f835fab2afcd7a754c8a2c365f677207d86cc538580`; technical evidence in verify cycle4 and prior reports. AC-2–AC-5 retain their tested-scope PASS and documented trusted-CLI/native Linux evidence limits.
- Authority: record the actual Captain decision against the prepared native Briefing and publish one PR. No blanket conn, merge, cleanup, changed-candidate approval or extra provider launch is granted. Six of six actual calls remain exhausted. Preserve terminal, worktree, evidence and unrelated entities. Run one normal FO-owned review follow-up after publication.
- FO is explicitly instructed to persist this exception before evidence-only readiness reconciliation. No product bytes, AC wording, historical results or frozen acceptance bytes were changed by this record. Private provenance also exists in Navigator's mission journal outside the repository.


## Stage Report: verify (cycle 5 — accepted AC-1 publication exception)

- DONE: Reconcile the explicit actual-human AC-1 exception and design approval against the unchanged candidate and frozen technical evidence.
  The retained original verifier handle read cycle4 and the appended Captain decision. Product HEAD remains `63b31f16b4f962f88f665f356c51a1ee21a2b953` with a clean code worktree. This is evidence-only reconciliation, not a fresh technical test or replacement reviewer.
- DONE: Record per-AC readiness for native gate and PR publication only, preserving technical limits and prior findings.
  The Captain's explicit acceptance removes the publication hold for the named missing human test. It does not supply the missing answers or convert AC-1 to PASS. No open Material technical finding is introduced or erased.
- DONE: Preserve the operational publication limit separately from product acceptance.
  State commit `167fbe0f8179cbe525af0174d492368ff391e982` was published without Signed-off-by because the temporary hook was absent. Signed-off follow-up `d3491b0` adds `publication-exception.md` attestation; it does not retroactively sign or rewrite the original commit. This report uses explicit `git commit -s` and task-path-only state sync.
- SKIPPED (explicit scope): New provider calls, technical reruns, product edits, human-answer simulation, gate/PR/frontmatter writes and facilitator-source disclosure. Six of six calls remain exhausted.

### Authority and evidence binding

- The recorded exact question asks whether to accept the absent human reading test, approve task090 PR creation, retain AC-1 unverified and exclude merge. Kent answered 「對」 following 「approve」. Source: parent `71fac6c1-962e-4f9f-abf3-34a04cc7c166`, message `bb0cda50-2771-41be-85f9-3d4a0ce85676`; Navigator relay `68864c3b-33cf-4b40-8ad0-97390003e631`. This verifier relies on that attributed human decision, not an inferred waiver.
- Kent's “checked the screenshot approve this design” is separately recorded visual/design approval (parent message `e1c08260-34b8-4e56-bd0e-90d198d25c8d`, Navigator `0590a9e1-7542-477a-b446-de5da72884ab`). The images remain terminal capture renderings, not pixel screenshots; the quote is not reader-comprehension evidence.
- Frozen evidence remains cycle4's Navigator acceptance SHA256 `9d1aa4d77882f78fd1786dabbdb7c7d3db888a26af99c8e85a7ecdde692c3f3a`, immutable debug binary SHA256 `ec3bbbdd2a7e140ff8979f835fab2afcd7a754c8a2c365f677207d86cc538580`, and six-call ledger SHA256 `17f484f404624161a9dc1535a1b952a7811ab9cdcc83c0fe764b42ae9d741afe`. These are retained audited bindings; this continuation does not claim to regenerate or retest them.
- All prior limits remain: trusted installed CLI/auth/config/helpers rather than universal isolation; accepted MCP startup; unknown-origin sampled shell processes neither proven model calls nor proven benign; bounded known-path/process effects, not complete filesystem/network/descendant proof; native Linux authenticated runtime unobserved. Real, synthetic and component evidence remain distinct.
- V090-1/V090-2 remain closed by the recorded corrected-candidate evidence. Mouse closure uses successful typed capture/key checks after click and wheel without keyboard refresh; the ambiguous Navigator Stopped+STALE frame remains excluded. Fixed/scrolled status, cancel and cleanup conclusions retain cycle4's exact scope.

### Current per-AC disposition and readiness

- AC-1 — **NOT PERFORMED / UNVERIFIED; Captain-accepted exception for native gate and PR publication only.** No identified unfamiliar-reader answers have arrived. User comprehension remains unproven.
- AC-2 — **PASS within tested scope.** Existing advisory/typed-fact separation, adversarial output and Original access evidence remains unchanged.
- AC-3 — **PASS within tested scope.** Existing successful capture/key, stale/race, cache/nonce/source-copy and navigation evidence remains unchanged.
- AC-4 — **PASS within tested scope.** Existing explicit generation, fixed running/terminal status, real cancellation and synthetic/error lifecycle evidence remains unchanged.
- AC-5 — **PASS under the approved trusted-CLI contract.** Existing both-adapter/local-provider evidence and disclosed platform/helper/process limits remain unchanged.
- **Ready for FO native gate decision recording and one PR publication under the explicit exception.** This is scoped publication readiness, not all-AC PASS, a recorded gate action, PR creation, merge permission or approval of another candidate. No further generation is authorized.

### Summary

The sole previous publication hold is now an explicitly accepted evidence gap. Technical findings and evidence remain unchanged on `63b31f16b4f962f88f665f356c51a1ee21a2b953`; AC-1 stays NOT PERFORMED / UNVERIFIED. FO may record the actual Captain decision and publish one PR within the stated scope, preserving the evidence and existing review-follow-up requirement. No merge or cleanup is authorized by this report.
