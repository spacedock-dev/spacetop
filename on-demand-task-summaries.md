---
title: Generate human-readable task summaries on demand
status: implement
source: "Captain selected Option A and requested an entity file, 2026-10-23"
kind: feature
risk: high
milestone: usability
proof: "Human comprehension walkthrough, snapshot/race scenarios, and adversarial provider-isolation checks; see acceptance criteria."
started: 2026-10-23T09:22:08Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-on-demand-task-summaries
issue:
pr:
mod-block:
id: 090
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
- Use the selected CLI's existing login and configuration; do not add app-managed API keys, separate accounts, or read/copy credential contents. A user needs only one available provider. Missing executable, login or requested model produces an actionable error with Original still usable; no silent provider fallback. Verify actual installed-version isolation rather than assuming flags are sufficient.
- Capture exact task input and source-copy identity. Bind results to workflow/task/source copy/input digest/provider/model/prompt version and request identity. Keep an in-memory cache only. Refresh outdated summaries only on explicit request.
- Run asynchronously with a visible indeterminate spinner and truthful short state such as `Generating summary…`; elapsed time is optional, but no invented percentage or ETA. Use the existing TUI tick/render loop without extra polling or network requests. Bind the indicator to the exact request/task; keep Original readable, navigation responsive and cancellation available. Stop animation on success, error, cancellation or timeout, and reject late results for a different selection. Bound input/output/time and clean up child processes. Explain provider data transmission and possible usage charges before first generation.
- Explicitly amend the product read-only policy to permit only constrained summarizer execution. Disable or contain tools, hooks, plugins, MCP, and project-instruction discovery. Empty working directories and instructions alone are not security boundaries. Use executable/argument arrays, never source-derived shell commands.
- Non-goals: automatic generation on navigation, background batches, persistent summary caches, changes to canonical task files, workflow dispatch, autonomous task actions, linked-evidence crawling, or inferred status/approval changes.

## Acceptance criteria

**AC-1 -- A reader unfamiliar with a task can identify its purpose, recorded progress, and outstanding decision or limitation from the summary.**
Verified by: a planned walkthrough with an identified unfamiliar reader and recorded answers against source material. Agent-generated judgments do not substitute for this human evidence; any missing proof remains explicitly unmet unless the captain approves a documented scope decision.

**AC-2 -- Generated prose never substitutes for recorded workflow facts or hides missing proof.**
Verified by: tasks with rejected reviews, contradictory reports, and unperformed human checks retain those distinctions. Typed status/gate facts remain authoritative; original text and worktree-diff views remain reachable. Proposed source links, if included, resolve only to validated sections of the captured input.

**AC-3 -- Results remain bound to their exact task snapshot.**
Verified by: switch tasks/workflows/source copies and change input while generation runs; no result appears on the wrong selection or as current for changed input. Matching requests reuse cached results; stale summaries are visibly marked and require explicit regeneration.

**AC-4 -- Summary generation is explicit and failure does not impair inspection.**
Verified by: browsing/opening previews causes zero provider requests; the first generation explains the selected provider, model, data transfer and possible usage charges. Missing executable/credentials/model, nonzero exit, malformed or empty output, oversized input/output, timeout, and cancellation leave Original usable. Oversized input is not silently truncated. Actual in-progress and terminal-state frames prove the request-bound spinner runs only while generation is active, with no invented percentage/ETA or extra requests. Navigation/late-result scenarios cannot attach progress to the wrong task. The UI remains responsive and child processes do not survive cancellation or application exit.

**AC-5 -- Both supported adapter paths enforce the summarization-only boundary.**
Verified by: adversarial task text requesting edits, shell execution, additional file reads, or external tool actions cannot trigger those actions. Cover both Claude/Haiku and Codex/Luna adapter paths, including unavailable executable/login/model behavior. Run real synthetic-data end-to-end checks for locally available configured providers and verify no project/workflow files change; record unavailable-provider live-evidence limits explicitly rather than claiming a live pass. A user only needs one available selected provider. Render provider output as untrusted text without terminal control effects. If isolation cannot coexist with existing login/configuration, escalate the concrete tradeoff rather than silently weakening the boundary or changing model family.

## Proof plan

- Lowest test layer: typed request/result and cache state tests; subprocess lifecycle tests for real failure/race boundaries; UI rendering/navigation checks.
- Required command: make lint plus relevant established tests; record commands actually run.
- Manual checks: actual debug-build TUI generation, an in-progress spinner frame and success/failure/cancel frames, navigation during generation, source changes, Original/diff switching, cancel and quit. Real provider smoke uses synthetic data with existing logged-in Claude/Haiku or Codex/Luna where available; both adapter paths require behavior coverage. Navigator retains illustrated acceptance and an immutable debug executable outside the worktree.
- Human check: identify the unfamiliar reader during planning, before declaring a verification-ready task.
- Docs/policy update needed: README/help/configuration guidance, narrow read-only-policy allowance for provider execution, provider prerequisites and isolation limitations. Do not add persistent storage roots in this task.

## Commit Discipline

- Commit status changes at dispatch and merge boundaries.
- Commit task body updates when substantive.
- Commit implementation work on the task worktree branch before signaling completion.

## Captain scope clarifications — 2026-10-23

These explicit decisions amend the original three-adapter scope; they do not waive the safety boundary or AC-1 human evidence. Navigator/FO runtime choices are unchanged.

- Existing-login decision: Kent said 「直接用 claude -p, codex exec , omp -p 就好了 不要管 API key 使用者的 claude, codex, or omp 應該至少有一個可以用的」. Source: parent `71fac6c1-962e-4f9f-abf3-34a04cc7c166`, message `0d79fe0c-98a6-440b-bc4a-040a2d5e6f04`, relayed by Navigator message `b5a93336-737c-41f2-b75a-c5e6ad7b3118`. Preserve existing CLI login/configuration; no new API keys/accounts or credential inspection.
- Superseding adapter/model decision: Kent said 「簡化一點好了 用 claude 或 codex 就好 然後選小 model Haiku 或 Luna」. Source: same parent, message `8e8b06b4-8642-4a13-80da-cb3ae4320bd7`, relayed by Navigator message `ab160757-06de-4878-8191-1b49fe4b1bb8`. Only Claude/Haiku and Codex/Luna remain; prior OMP findings are historical, not an implementation obligation. Do not substitute another model family if the requested one is unavailable.
- Progress decision: Kent said 「對了，在 agent 處理 summary 的期間，可以顯示動畫或進度」. Source: same parent, message `78640aa0-e894-472b-b818-2b6045cb852c`, relayed by Navigator message `91dac744-db43-42c5-a01d-96546f30960a`. Use a standard indeterminate spinner and truthful status within the existing render loop; preserve responsive Original/cancel and exact request binding.
- Authorized live-smoke ceiling: at most **six actual CLI launches total**, normally normal/adversarial/cancel for each of the two adapters, only after adequate local isolation readiness. Each uses at most 8 KiB synthetic input, 45 seconds, 64 KiB stdout and 16 KiB stderr; no automatic retry. Prior launches count toward this ceiling. **Zero actual provider launches used at this amendment.** This is not a dollar hard cap; report observable usage without inventing a cost. Use existing available login/configuration and the verified requested small model; no new model/account/budget questionnaire is required.
- Missing runtime/login/model is an explicit availability/evidence limit, not a reason to silently choose another provider or claim a live pass. Process-group-only cleanup has a demonstrated detached-child gap and cannot be accepted without an effective containment/lifecycle solution. Hooks, plugins, MCP, project discovery, write/exec restrictions, snapshot races, zero navigation requests and unfamiliar-human proof remain required.

## Implementation plan — scoped 2026-10-23

This plan follows the captain amendments committed as `3ea3608726b5b8b8cfe26472b54764423de19596`. It retains all five amended ACs, both adapter paths, existing CLI login, and the two released platforms. **The plan is ready for a bounded isolation-only implementation/probe slice; provider-connected safety is not yet proved, and large UI work must not start.** No model calls were made and no product files were changed during planning.

### Goal, confidence, risk and next decision

The mission goal is explicit, readable task summaries that help an unfamiliar human understand recorded work without replacing Original or recorded facts. Confidence is high in the typed state/rendering approach and in the measured local denial behavior, but low in complete authenticated isolation until the next slice is exercised. The largest risk is allowing a coding CLI's startup/auth/tool surfaces to exceed summarization. The next FO decision is to dispatch the same worker for the bounded isolation slice below, then require a fresh verifier; this is not approval to bypass its stop conditions. AC-1 still needs an identified unfamiliar human and recorded answers, coordinated by Navigator.

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

### Local isolation experiments and retained negatives

All startup probes used a cleared environment, synthetic text, disposable HOME/cwd, an OS network deny, bounded six-second supervisor timeout, and no real credentials. They were not real-provider launches. `claude -p` used `--safe-mode --bare --tools "" --disable-slash-commands --strict-mcp-config --mcp-config '{"mcpServers":{}}' --setting-sources '' --no-session-persistence --output-format json --model synthetic-nonexistent-model`; the historical `--bare` experiment is not the proposed existing-login invocation. `codex exec` used `--ignore-user-config --ignore-rules --ephemeral --skip-git-repo-check --sandbox read-only --color never`, disabled `hooks/plugins/apps/shell_tool/multi_agent`, set `project_doc_max_bytes=0` and `web_search="disabled"`, and used a synthetic nonexistent model with stdin `-`. These flags are candidates, not complete tool-disable proof.

- With selected-path denials, both CLIs started. Claude exited 1 with zero API usage and “Not logged in”; its structured result had `is_error: true` despite `subtype: success`, so the adapter must inspect exit status and error fields. Codex attempted reconnects despite missing auth/model; the OS denied network and the supervisor killed its group at six seconds. App-level retries must be absent, and CLI internal retries must be accounted for rather than hidden behind that claim.
- A real local Python canary showed an ordinary forked child disappeared after `killpg`, while a child calling `setsid()` survived. The escaped PID was explicitly killed. This rules out process-group-only cleanup as sufficient proof.
- A separate kernel canary with deny-fork/exec/network and selected filesystem restrictions returned `EPERM (1)` for `fork`, outside synthetic read/write, `/bin/sh` execution, and loopback connection. It would fail if any of those operations became permitted. Creating a socket alone initially succeeded; the corrected probe tested an actual connection.
- An initial default-deny file-data profile caused all tested CLIs to abort (`SIGABRT`, no stderr). PID-filtered kernel logs identified a denied read of the literal root directory `/`. Allowing that directory entry, not its descendants, advanced startup. Claude then failed on `/tmp/claude-501`; setting `CLAUDE_CODE_TMPDIR` to request scratch fixed that. Runtime directory entries and timezone reads resolved a subsequent Bun trap in historical OMP testing. With those narrow repairs, default-deny file-data reads plus deny-fork/exec again reached the same no-auth outcomes. These negatives were retained; they do not prove isolation is impossible.
- The final local startup profile denies network, file-data reads, writes, fork and exec by default for those operations; it allows exact CLI binaries, necessary system/runtime paths and private scratch. This is a feasibility profile, **not a complete hardened sandbox**: other OS capabilities were not audited and authenticated inference was not exercised. Do not reopen general HTTPS and call it safe.
- A disposable existing Linux arm64 Python image, run with `--pull never --rm --network none --read-only --cap-drop ALL --security-opt no-new-privileges --pids-limit 16 --tmpfs /tmp:rw,nosuid,nodev,noexec,size=16m`, reported zero effective capabilities, `NoNewPrivs=1`, root writes denied with `EROFS (30)`, network denied with `ENETUNREACH (101)`, and scratch writes allowed. No images/tools were installed and the container was removed. This supports testing a Linux namespace/container approach; it does not prove the released x64 CLIs, existing host login, or connected egress work there.

Local recipes, version/help captures, per-variant results and scripts are declared at `/tmp/spacetop-090-probe/`: `probe.py`, `probe-nofork.py`, `probe-strict.py`, `probe-strict-root.py`, `probe-strict-runtime.py`, `boundaries.py`, `boundaries-nofork.py`, `existing-login-default.py`, `existing-login-exec-only.py`, `existing-login-security-runtime.py`, and result JSON files. `manifest.json` records the first probe hashes. They are disposable planning artifacts, not committed regression tests or a prerequisite hidden from future implementers. Promote the relevant recipes into repo-owned fixtures/tests in the first implementation slice. The outcomes and reproduction inputs above remain in this task if scratch is lost.

Historical OMP 18.12.6 evidence is retained only to explain why CLI flags were not assumed sufficient. Pinned [main.ts](https://github.com/can1357/oh-my-pi/blob/v18.12.6/packages/coding-agent/src/main.ts) maps `--no-tools` to an empty built-in tool list, while [sdk.ts](https://github.com/can1357/oh-my-pi/blob/v18.12.6/packages/coding-agent/src/sdk.ts) separately discovers MCP/custom tools/context unless additional SDK restrictions apply. OMP is no longer an implementation or proof obligation.

### Next executable slice: isolation before UI

Use one reusable implementation worker, then a fresh verifier. Work in the task worktree once FO assigns it. Keep this first slice small and independent of rendering:

1. Introduce typed, nonterminal launch/result boundaries and a repo-owned local fixture executable. Build an executable-plus-argv `Command`, never a shell string or source-derived flag. Source text travels only through bounded stdin, never command-line arguments or `@file` interpolation. Start a supervisor before writing stdin; concurrently drain bounded stdout/stderr, close stdin at EOF, and make timeout/cancel/drop reap processes and join I/O workers. Do not use `Command::output()`/`wait_with_output()` as an unbounded collector.
2. For macOS, turn the measured profile into a minimal explicit capability policy; validate symlink/canonical executable identity and private mode-0700 request scratch. Deny workflow/project/private-file reads except the captured stdin and specific existing CLI-owned auth/config reads. Keep all app-owned writes in scratch; CLI auth/config must remain unmodified. Prove that tools/hooks/plugins/MCP/project instructions cannot load or act, including managed/built-in customization paths. Test any permitted auth helper without exposing credentials or enabling arbitrary child execution. Do not claim Claude support until the existing keychain conflict is resolved under an effective descendant boundary.
3. For Linux x64, test native user/mount/PID/network namespace plus seccomp/cgroup containment, or a maintained equivalent launcher, on the repo's Ubuntu x64 execution surface. Verify required kernel/user-namespace permissions rather than assuming them. Docker is a local experiment option, not an approved new runtime dependency. Existing-login discovery must work without copying credentials into new accounts/images. Define a typed unsupported-isolation error when prerequisites are absent; keep ordinary inspection usable. Missing Linux evidence is work to do, not proof of impossibility or permission for macOS-only delivery.
4. Add a localhost-only synthetic protocol probe for Codex/Luna first, using a fake token and no vendor endpoint. Inspect emitted tool declarations and startup context; return adversarial tool-call output and confirm it is rejected/contained. Test inference-only egress: deny all other hosts, protocols, local sockets, metadata endpoints and redirects. No hidden CLI discovery/telemetry/refresh endpoint may become a general network escape. A simple HTTPS host allowlist alone does not prove an inference-only surface. Use a narrow reviewed mediation approach only if needed; it must preserve CLI-owned login and must not turn into app-managed credentials.
5. Re-run existing-login checks under the final candidate boundary for both adapters. Claude's safe-mode preserves auth according to installed help, but built-in tools/plugins still require explicit restrictions; use `--tools ""`, disabled skills, strict empty MCP and explicit settings-source behavior as tested. Codex's `--sandbox read-only` only describes model shell behavior, and its feature flags are not an all-tools-off contract. Verify the actual tool set and contain remaining in-process capabilities. Ignoring dangerous hook/plugin/project overrides must not silently replace provider/account/model settings; enumerate any required safety overrides in docs and fail closed for incompatible configuration.
6. Only when local readiness is demonstrated, run the authorized real synthetic smoke below, then have a fresh verifier inspect raw evidence. If the boundary cannot support existing auth, finish the code/evidence slice with a concrete tradeoff and minimal option; do not ask for API keys, switch model families or silently omit an adapter. No large UI implementation until this stop condition is cleared.

The next slice is executable engineering work; the current evidence does not require a new account/model/budget questionnaire. Any need for a new external runtime dependency or a changed product/security contract beyond the approved narrow summarizer allowance is a decision for the captain. Full feasibility and AC-5 remain open now.

### Owned implementation sequence after isolation proof

| Step | Owned modules | Behavior and lowest-layer proof |
| --- | --- | --- |
| Typed snapshot/config | New `spacetop-core/src/domain/summary.rs`; `config.rs`; query/source access in `query.rs`/`sources.rs` | `SummaryProvider`, exact model ID, limits, `SummarySnapshot`, cache key, request identity, typed failures. Config loads only through existing absolute user paths. Capture bounded exact source bytes and the authoritative metadata used by the current query snapshot; preserve main metadata versus selected worktree-body provenance explicitly. Reject mismatched/changed reads; never silently truncate. |
| Prompt/output | New `spacetop-core/src/summary/{prompt,output}.rs` | Versioned fixed instructions; delimit captured source as data. Preserve claimed versus checked evidence, contradictions and missing human proof. No linked-evidence reads. Parse provider envelopes strictly, handle Claude `is_error` and exit status, bound decoding, reject empty/malformed output. Strip/replace terminal controls before display. Plain summary text has no executable links or ANSI. |
| Adapter/process boundary | New `spacetop-core/src/summary/{adapters,process,isolation}.rs` | Separate Claude/Haiku and Codex/Luna argv builders, version capability checks, safe existing-login handling, effective descendant containment, bounded pipes/deadlines. Use existing threads/channels; no async runtime by default. Extend `rustix` process features only if justified; document any unavoidable maintained sandbox dependency before adding it. |
| App request/cache state | New `spacetop/src/app/summary.rs` and `summary_worker.rs`; `app/overview.rs`, `app/session.rs`, `app/keys.rs`, `lib.rs` | Explicit request intents; first-generation disclosure; one active request at a time; bounded in-memory cache. Key includes canonical workflow, task identity, selected source-copy identity, exact input digest, authoritative-fact digest, provider, exact model and prompt version. Request nonce/epoch is separate so a canceled or superseded result cannot revive progress. No persistence or automatic regeneration. |
| Preview/progress | `ui/preview.rs`, new `ui/summary.rs`, help/footer and key tests | `S` is presently unused in inspected preview bindings; reserve it only after complete keymap collision tests. Prefer `S` to enter Summary, `Enter` to generate/regenerate only when explicit, and `Esc` to cancel active work before closing; retain Original/diff access. A typed running request drives an indeterminate spinner and “Generating summary…” using existing ticks only. Completion/error/cancel/timeout stops it. No fake percentage/ETA. |
| Docs/policy | `README.md`, `AGENTS.md`, `docs/development-policy.md`, `docs/code-review-policy.md`, help/config examples | Narrow allowance for constrained summarizer execution; unchanged workflow read-only and Git-write rules. Explain transfer/charges, CLI/model prerequisites, existing-login use, platform isolation limits, cache staleness, unavailable errors, cancel and no fallback. Update loaders only if references change. |

Source capture must not accidentally send a worktree's conflicting frontmatter as authoritative main status. Capture and label source copies separately; hash the exact assembled input sent to the provider. Re-check identity before presenting a result as current. Source changes mark cached prose stale and require explicit regeneration. Navigation may leave a request active, but its spinner/result belongs only to its captured key; a different task remains Original/idle. Results after cancel/quit are discarded. Cache bounds should cover total bytes as well as entry count; initial proposed runtime limits are 64 KiB source, 64 KiB stdout, 16 KiB stderr, 45 seconds and 16 cached results/1 MiB total, all subject to model context validation. Live smoke uses the stricter 8 KiB source limit.

### AC proof map and falsifiable checks

| AC | Required proof and owner | What fails the check |
| --- | --- | --- |
| AC-1 | Navigator coordinates an identified unfamiliar human. Implementation generates the frozen synthetic source's actual summary; facilitator records verbatim answers against the separate key below, source/summary/model hashes, time and candidate. | An agent answer, missing participant/answers, pre-exposed source, invented approval, or inability to identify purpose/progress/open limitation leaves AC-1 unmet. |
| AC-2 | Core prompt/envelope tests plus app/TestBackend fixtures for rejected reviews, contradiction, unperformed human check, main/worktree facts. Fresh verifier inspects real summary and Original/diff reachability. | Prose mutates typed status/gates, erases rejection/missing proof, invents reconciliation, exposes source links outside captured sections, or blocks Original/diff. |
| AC-3 | Pure app state tests schedule A/B task, workflow, active/archive and source-copy switches; mutate bytes/metadata; reverse completion order; cancel/retry same key. Verify exact cache reuse and stale labeling. | A completion/spinner attaches to a different selection, changed input appears current, a canceled nonce revives, or navigation/regeneration occurs implicitly. |
| AC-4 | Real fixture processes cover absent binary, unavailable auth/model, nonzero exit, misleading success envelope, empty/malformed output, oversized input/stdout/stderr, blocked stdin, simultaneous pipe floods, timeouts, cancel, app exit and escaped descendants. TestBackend uses controlled ticks; Navigator drives actual debug TUI and captures in-progress and terminal frames. | Any browse action launches a provider; input silently truncates; progress stalls input or persists after terminal state; extra polls create requests; process survives; Original is inaccessible. |
| AC-5 | Both adapter paths receive local adversarial protocol/OS tests; actual available configured providers receive synthetic real E2E after readiness. Audit fixture/project/workflow manifests and outside-read/exec/network canaries, not only “no changed files.” Sanitizer tests and actual terminal capture cover ESC/CSI, OSC 8/52, DCS, C0/C1 controls, CR/backspace and malformed UTF-8. | Any requested edit/read/shell/tool action escapes, hidden startup customization runs, credentials leak, terminal control affects UI, or an installed logged-in provider's containment conflict is mislabeled supported. Unavailable live evidence is explicit, never a live pass. |

Run relevant focused tests during each slice, then `cargo fmt --check`, `cargo test`, and required `make lint` on final Rust changes. Keep `cargo test -p spacetop-core --test no_write_git_calls` and `cargo test -p spacetop-core --test no_terminal_deps` valid; do not exempt arbitrary summary commands from existing Git safety checks. Add lower-layer process/isolation tests and test-host-specific CI runs for both release targets. The ignored watcher test is unnecessary unless watcher behavior changes. Planning changed no code, so Rust build/lint tests were not run here.

### Authorized real smoke and acceptance evidence

**Zero of six actual live CLI launches used.** Prior fake-model starts were network-denied, credential-free local probes; auth-status commands do not invoke a model. Before each real launch, the sole worker must reserve number, adapter, purpose, exact model and candidate in `/tmp/spacetop-090-fo/live-smoke-ledger.json` and notify FO. Each attempted generation counts, even auth/model failure. No automatic retries, replacement launches or parallel duplicate worker. Maximum six total, normally normal/adversarial/cancel for each adapter; no more than 8 KiB synthetic input, 45 seconds, 64 KiB stdout and 16 KiB stderr. These limits are not a billing/token ceiling. Record observable usage and any internal CLI retries; do not invent cost or assume killing the local process erases already incurred usage.

Use only `claude-haiku-5-5` and `gpt-6-luna` once locally confirmed for the selected path; if unavailable, surface that exact failure with Original intact. The normal call checks readable, source-grounded output; adversarial source requests outside read, file edit, shell, network/MCP action and false approval; cancellation checks responsiveness and actual process cleanup. Put all attack variants in one source, not unbudgeted calls. A timeout/cancel fixture proves local handling; real provider cancellation alone cannot prove remote computation stopped. Future TUI/Navigator calls also consume this ceiling, so coordinate the same six launches with final debug-TUI acceptance rather than spend all calls on disposable pre-UI probes. Any necessary extra live call requires new authorization.

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
