# Summary isolation feasibility fixtures

Task 090 is stopped before product/UI implementation. These scripts reproduce
bounded local evidence; they are **not a production summary runner or a supported
sandbox**. There is no Spacetop launch path, new user configuration, or runtime
dependency. Existing inspection behavior is unchanged.

The second bounded comparison is recorded in [ADVANCE.md](ADVANCE.md). The
single subsequent exact-item diagnostic and stopping decision are recorded in
[KEYCHAIN.md](KEYCHAIN.md). Existing-login and lifecycle requirements remain
unresolved; synthetic protocol fixtures are diagnostic evidence, not product
progress.

## Reproduce

Use Python 3.10+ on Unix from the repository root. Tests need no coding CLI,
credentials, package install, or network:

```sh
python3 -B -m unittest discover -s tests/fixtures/summary-isolation -v
```

The tests exercise literal stdin, rejection before spawn, missing executable,
nonzero/error envelopes, both output caps, blocked-stdin timeout, and direct-child
cancellation/reaping. The process-group negative test starts a trusted `setsid`
child and observes its heartbeat **after** killing the original process group.
Closing a supervisor-owned pipe ends that child; a five-second child deadline is
an additional backstop. The test does not kill arbitrary PIDs from files.
It must fail if the negative result disappears. A passing test here confirms
that process groups are **insufficient**, not that production cleanup is safe.

macOS-only OS canary (no CLI/auth):

```sh
python3 -B tests/fixtures/summary-isolation/probe.py
```

This checks actual fork, outside-file read/write, exec and loopback connection
denials (`EPERM`), plus an allowed scratch write. Scratch is mode 0700. The
profile denies those operations by default and permits specific runtime paths;
its other OS capabilities remain unaudited. The Python runtime allowance is
only for the canary. It must not be reused for a provider adapter.

Optional, explicit **auth-status only**, using installed Claude/Codex and their
selected local configuration:

```sh
python3 -B tests/fixtures/summary-isolation/probe.py --auth-status
```

Every status variant denies network and user-file writes. The runner never opens
credential files; only the CLI gets named read allowances. Output is limited to
booleans, exit/outcome, executable path and executable SHA256. Auth status is not
inference entitlement. `USER` must remain present when present in the original
environment: omitting it alone changed Claude from logged-in to logged-out on
the tested host. No CLI config/home override is invented. Profiles that allow
helpers or broad reads are explicitly diagnostic, not production candidates.

Local fake Codex/Luna protocol (macOS, installed Codex; **no real model call**):

```sh
python3 -B tests/fixtures/summary-isolation/codex_protocol.py
python3 -B tests/fixtures/summary-isolation/codex_protocol.py --adversarial
python3 -B tests/fixtures/summary-isolation/codex_protocol.py --adversarial --auth-exception
python3 -B tests/fixtures/summary-isolation/codex_protocol.py --adversarial --auth-exception --disable-image-tool
```

This uses a disposable CLI home, a fake token and a loopback fake Responses
endpoint. The OS denies egress except the one chosen localhost port. The fixed
model argument is `gpt-6-luna`; `--ignore-user-config` is intentional for this
synthetic experiment and does **not** establish preservation of real provider
configuration. The server returns canned output, not model-generated prose.
The normal exchange makes one local request. The adversarial exchange makes two
(tool response, then final text), with transport retries disabled. Its reported
token counts are fake protocol data, not billing evidence.

## Observed results and limits

The committed JSON under `evidence/` records macOS arm64 results on 2026-10-23.
Executable hashes pin Claude 2.1.309 and Codex 0.164.0. Raw protocol output
contains only synthetic text and temporary paths. The scripts clean up their
own scratch. No real generation launch was made; the shared live budget remains
0 of 6.

| Probe | Observation | What this rules out |
| --- | --- | --- |
| Direct OS canary | Outside read/write, exec, fork and connection denied; scratch write allowed | Empty cwd or CLI flags as the only filesystem/exec boundary |
| Kill original process group | Detached child keeps producing heartbeats; fixture lease then stops it | `killpg` as a complete descendant cleanup strategy |
| Claude reference vs reference plus deny-fork | Logged-in vs logged-out, with all other permissions unchanged | Deny-fork as an existing-login-compatible solution for this installed Claude |
| Claude exec allowlist (`security`, `sh`, CLI), broad reads | Logged-in | Missing login/account as the cause of the strict profile's failure |
| Claude strict reads plus helper exec | Logged-out | The tested exact read allowlist as sufficient for auth; exact missing read still unresolved |
| Codex strict nofork auth status | Logged in using ChatGPT | A local auth-recognition failure under this profile; inference/refresh remain untested |
| Codex fake endpoint | Advertises `tools: []`; rejects `exec_command`/`read_file`; rejects patch by read-only policy | Those tested arbitrary shell/edit calls under this exact local candidate |
| Unadvertised `view_image` call | Still routed; outside read denied by OS with `EPERM` | Empty tool declarations as proof that all tools are disabled |
| Same tool, explicit synthetic auth-file read allowance | Reaches image decoding and returns invalid image data instead of `EPERM` | Per-process read allowances as separation between authentication and model-directed file reads; no real credential was inspected |
| Explicit `--disable view_image` | Both outside and allowed synthetic auth-file calls become unsupported | The tested `view_image` route under this additional switch; other internal tool paths still need coverage |

Two errors in the older `/tmp` experiments were corrected: a helper profile's
allow statement was outside its loop and retained only the last auth path, and
its cleared environment omitted `USER`. Neither historical result proves that
authenticated isolation is impossible. The corrected nofork control now isolates
the actual incompatibility. Tests pin the corrected allowlist and environment.

The strict profile still permits auth/config reads in the CLI process. The
synthetic auth-file case demonstrates that this permission also reaches an
in-process tool. It establishes a model-directed read attempt reaching the image
decoder, not exfiltration of a real credential. Installed `codex features list`
exposes the stable `view_image` feature; disabling it closes this tested route.
The earlier negative is preserved beside that narrower positive. Other
in-process tools and auth/config exceptions still need review. This suite does not
prove CLI hooks/plugins/MCP/managed configuration suppression, auth refresh,
vendor-only inference egress or redirect handling. A localhost protocol result
does not establish any of those connected-provider properties.

The released Linux x64 target remains required. A bounded Docker
`--platform linux/amd64` compatibility attempt downloaded the official
`python:3.12-slim` amd64 image and passed all ten synthetic supervisor tests under
emulation, with no network, read-only root, all capabilities dropped,
no-new-privileges and a 16-process limit. Raw output is in
`evidence/linux-amd64-attempt.txt`; selected image ID is
`sha256:e8a38829209aea82774882e5157e1f7c3e8c8393ff53a697b3580896e57f345c`.
Only this fixture directory was mounted, read-only; no host credentials were
mounted. The temporary container was removed. No CLI or global tool was installed.
This proves the synthetic tests run as amd64, **not** native Linux CLI/auth or a
production launcher. Those require a native x64 host or compatible CLI artifacts
and an existing-login test context. Container use is not an approved new product
dependency. macOS-only probe commands fail explicitly on unsupported platforms.

## Next decision

The concrete fork is how to preserve Claude's required helper process while
guaranteeing descendant cleanup. The tested nofork solution cannot do both.
Keep summary UI and real calls stopped. Recommend one bounded containment slice:
evaluate an explicit helper-aware OS launcher/lifecycle boundary on both release
hosts, and require synthetic escape tests plus existing-login status before
connecting provider egress. If that needs a new runtime dependency or an auth
broker, present that concrete design for captain approval first. Do not replace
existing login, narrow to macOS/Codex, or treat missing acceptance as a pass.

AC-1 through AC-4 remain unimplemented. AC-5 has local component evidence only;
both actual adapters, real synthetic E2E, final TUI frames and the unfamiliar
human walkthrough remain open. No completed-product or verification-ready claim
is made by these fixtures.
