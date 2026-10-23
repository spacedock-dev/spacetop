# Summary implementation evidence and handoff

The product implements the explicitly approved trusted-CLI contract. This is
not a full-isolation pass. Previous strict-isolation findings in
`tests/fixtures/summary-isolation/` remain historical negative evidence.

## Actual normal launches

Both normal launches used committed production-boundary candidate
`87cad772207b5ed806d59555b403eb48c023e2ec`, through the thin
`spacetop-core/examples/summary_smoke.rs` caller. That caller uses production
snapshot capture, prompt, argv, bounded nonblocking pipes and tracked cleanup.
It did not use an unconnected prototype. These are native macOS arm64 results,
not Linux x64 or final-TUI acceptance.

| Launch | CLI/model | Result | Private raw evidence |
|---|---|---|---|
| 1 | Claude 2.1.309 / claude-haiku-5-5 | Exit success; is_error=false; 3227-byte stdout, empty stderr; one reported turn, 5215 ms; 221-word parsed summary | `/tmp/spacetop-090-live/launch-1/` |
| 2 | Codex 0.164.0 / gpt-6-luna | Exit success; one completed agent message and completed turn; 1503-byte stdout, empty stderr; 178-word parsed summary | `/tmp/spacetop-090-live/launch-2/` |

Source SHA-256: `698210b90b684ed69400fcd0ea0f294581b8c13e3354ea817f9a487427e99c8b`.
Captured input SHA-256: `cbc3f6772683dc80d3a1fd1f224ebaec79567aea022c8979617e41c1aa115d28` (2118 bytes).
Prompt SHA-256: `bb3bbd217189d3fd79317cccb2f6ae5c136f1fd0253e96c1f3f8b8a7666b2cea` (3021 bytes).
Claude stdout SHA-256: `6c56bde56d6e67fbb330ca215c8084dbd765a72e91cb0a4088d9a38076084a35`.
Codex stdout SHA-256: `088792b21c8bea77b539b7780ff397376e8909fb49f65bee311213641ab94afc`.

Claude reported list-basis cost USD 0.00120029, 2 input tokens, 3098 cache-created,
649 cache-read and 1148 output tokens (721 thinking). Codex reported 18345 input,
8064 cached input and 215 output tokens; no cost was supplied. These are observed
CLI fields, not a billing guarantee. No app retries occurred. No Codex retry
events were emitted; CLI-internal retry counts were not exposed. Exact argv,
binary hashes, preparation and result JSON are in the shared ledger and private
folders. Raw outputs are not copied into the repository to avoid exposing the
facilitator material before the human walkthrough.

The final UI candidate preserves prompt/adapter/process source. Source capture
adds nonblocking/no-follow file opening (so a replaced FIFO cannot block the UI)
and distinguishes worktree-only metadata. A no-launch `prepare` replay confirmed
the frozen normal input and prompt hashes still match. These outputs remain
attributed to the actual immutable launch candidate. Changed prompt/input bytes
must not inherit these output bindings. Reproduction requires an explicit new live
allocation; do not rerun the example as an ordinary test.

## Local component checks

`cargo test -p spacetop-core --test summary` covers literal stdin, native argv,
source/fact mismatch rejection, provider/source-copy/archive identity, empty or
malformed envelopes, terminal controls, output caps, blocked stdin deadlines,
simultaneous floods, cancellation and direct-child reaping. App tests cover zero
browse requests, disclosure, nonce rejection, cache bounds/reuse, stale input,
wrong-selection results, exact-request spinner and failure terminal states.
Worker tests cover shutdown joining and no launch after quit. Ratatui TestBackend
checks disclosure, running/completion, recorded headers, Original and narrow
rendering. These are component renders, not captured native-terminal acceptance.

No new package was added. The existing rustix dependency enables its `process`
feature for nonblocking supervision, waitid with WNOWAIT and group termination.
The direct child stays unreaped until group cleanup to avoid PID reuse. Process
pipes run nonblocking in one worker; no blocked pipe thread survives cancellation.
Escaped descendants remain outside the cleanup guarantee.

## Final local check record

The full workspace run passed 785 tests, with six existing ignored tests.
Subsequent source-capture guards have their focused core suite and final lint
recorded with the native stage report. `cargo fmt --check`, `make lint` and debug
build passed. Full test/build/lint logs are at
`/tmp/spacetop-090-product-{tests,build,lint}.log`. The unchanged Git-write and
terminal-free guardrails passed in the workspace suite. The unrelated ignored
watcher test was not needed. A full-suite-discovered unloaded-tab refresh panic
was fixed; its summary-active workflow-switch regression passes.

## Navigator: four reserved real TUI launches

The shared ledger is `/tmp/spacetop-090-fo/live-smoke-ledger.json`: **2/6 used**;
worker allocation exhausted. Four calls remain for Navigator adversarial/cancel
on both adapters. Reserve each unique number/owner/model/purpose/candidate and
source/prompt hashes before generation, notify FO, and count errors/cancellations.
No automatic replacement attempts. Do not run `summary_smoke` again under these
four TUI allocations. Keep total input <=8 KiB, deadline <=45 s and pipe caps
64 KiB / 16 KiB. Real calls may incur usage before cancellation.

1. Build the final committed debug candidate with `cargo build -p spacetop`.
   Copy `target/debug/spacetop` to a private immutable path; record candidate,
   binary hash, terminal identity and dimensions. Do not use a release build.
2. Create disposable inputs with
   `python3 tests/fixtures/summary-ui/setup.py /tmp/spacetop-summary-nav-UNIQUE`.
   Record hashes of that tree before launch. The fixture contains only synthetic
   markers and explicit attack DATA, and does not read any credentials.
3. Launch the copied binary with `--workflow-dir /tmp/spacetop-summary-nav-UNIQUE/workflow`.
   For Codex, use an isolated absolute XDG_CONFIG_HOME containing only
   `spacetop/config.yaml` with `summary: {provider: codex}`. Preserve HOME and
   CLI-specific config variables/login. Use a private XDG_STATE_HOME for TUI state.
4. `Enter` opens preview, `S` shows Summary/disclosure. Browsing and these keys
   must consume no request. Bind the exact production snapshot/prompt before
   pressing generation `Enter` (the example's `prepare` mode only computes
   hashes and performs **no CLI launch**; it can prepare the same fixture file).
   The first generation `Enter` is the explicit consent/action boundary.
5. Capture the real in-progress spinner, navigate to the second task and verify
   no foreign spinner/result, return, and capture the terminal state. Inspect
   observed tool/action events if available and audit all synthetic files. Do
   not call the attack a pass merely because no tool declarations were listed.
   Test `O`/`S` Original, retained status/gates, and worktree diff separately.
6. Use the second allocated call per adapter to cancel promptly with `Esc` while
   the spinner is active. Capture stopped progress, Original responsiveness,
   tracked cleanup and no late-result revival. An already finished request is
   not a cancellation pass. No unbudgeted redo if it completes too quickly.
7. Source edits must show retained prose as STALE and require explicit `R` or
   generation `Enter`; navigation never regenerates. Use fake/local component
   tests for further race variations without spending real calls.

Human AC-1 remains **unmet** until Navigator identifies an unfamiliar reader and
records actual answers. Keep the facilitator source/answer key private; use only
an actually generated, hash-bound summary and the separate reader questions.
Do not coach, simulate answers, infer approval or regenerate toward a desired
human result. Fresh independent verification, illustrated Navigator acceptance
and native Linux x64 runtime evidence remain distinct open evidence boundaries.
