# Second bounded isolation pass — 2026-10-23

There is useful new protocol evidence, but no complete existing-login isolation
boundary. No production code, UI, account configuration, broker, dependency or
provider generation was added. Live usage remains 0/6. This does not establish
that every dependency-free approach is impossible.

## 1. Exact auth-helper capabilities: narrowed, still inconclusive

`auth_differential.py` exercises only `claude --safe-mode auth status --json`,
or the explicit metadata-only `security default-keychain -d user` command.
It never asks `security` for an item value or opens a credential file. All
variants deny network and filesystem writes outside scratch (plus the two
recorded runtime devices). Changes are exact literal reads, not HOME/Keychain
subtree grants. Profiles still inherit the original `allow default` Mach policy;
no missing Mach allowance can be inferred from these file-read failures, and no
minimal Mach policy is claimed.

Own-PID denial metadata named HOME directory access and
`~/.claude/settings.json`. Allowing either, and then both, did not restore login.
The observed helper PIDs also produced no new denial naming the missing item
capability. The default-keychain metadata query **did succeed** under the narrow
profile and returned the existing login-keychain path. Thus default keychain
discovery is not itself the failing operation. The remaining gap is between
that discovery and the CLI recognizing the existing credential; its exact
permission/configuration cause is not established.

An early diagnostic observer divided libproc's return value by sizeof(pid_t)
twice and missed short child lists. That affected only diagnostic coverage,
not the profile or auth outcomes. The corrected observer captured two actual
helper PIDs; a real one-child test now pins the API's count units. Sampling
children is explicitly **not** used as a containment or complete-enumeration
claim. Apple's [libproc implementation](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.c)
returns a PID count from `proc_listchildpids`, unlike `proc_listpids`.

Evidence: `advance-auth-{settings,settings-home,observed-helpers}.json`,
`advance-keychain-metadata.json`. Earlier `advance-auth-helper-denials.json`
retains the pre-correction observer limitation. No broader permission was added
to manufacture a successful auth result.

## 2. OS descendant lifecycle: concrete failures and host prerequisites

The actual Darwin SDK marks NOTE_TRACK/NOTE_CHILD unsupported since 10.5.
They cannot provide inherited descendant tracking here. The installed launchd
manual describes AbandonProcessGroup cleanup as killing the original process
group; that is not a solution to the already-demonstrated setsid escape.
See Apple's [event.h](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/event.h).

The installed `/usr/share/sandbox/cvmsServer.sb` establishes that
`syscall-unix` / `syscall-number` filters are available on this host. Actual
canaries distinguish three candidates:

| Changed policy | Direct setsid/setpgid | posix_spawn with SETSID/SETPGROUP |
| --- | --- | --- |
| deny process-info-setcontrol | Allowed | Allowed; new SID/PGID observed |
| deny SYS_setsid and SYS_setpgid | EPERM | Still allowed; new SID/PGID observed |
| also deny SYS_posix_spawn | EPERM | EPERM |

The second result is an actual bypass of the direct-syscall guard, not the
earlier killpg-only experiment. Apple [kern_exec.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exec.c)
applies the spawn attributes internally. Reproduce with
`python3 -B tests/fixtures/summary-isolation/session_policy.py CANDIDATE`, where
CANDIDATE is `setcontrol`, `direct-syscalls`, or `all-session-syscalls`.
`advance-session-comparison.json` records the first two observed results;
`advance-spawn-inner.json` records all four denied calls in the final variant.

The full syscall guard made the previously logged-in Claude reference report
logged-out (`advance-spawn-deny.json`). That diagnostic added restrictions to the
recorded broad-read positive control; it was not a proposed broad-read production
profile. The first Python invocation also failed because Homebrew's launcher
itself uses posix_spawn. The corrected canary launches the observed framework
binary directly and is retained separately. We did not claim successful
cancel/timeout/exit cleanup for an auth-compatible helper boundary: no tested
variant reached that prerequisite. Existing direct-child timeout/cancel tests
remain narrower evidence.

On the available Linux VM, the amd64-emulated process reported PID 1,
`unshare(CLONE_NEWUSER|CLONE_NEWPID) = EPERM`, read-only cgroup root, an existing
`cgroup.kill`, and successful PR_SET_CHILD_SUBREAPER. These facts describe the
current constrained container, not all Linux hosts. A subreaper alone is not a
race-free kill boundary. Linux documents [PID-namespace init termination](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html)
and [cgroup.kill](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
as relevant primitives, but their delegated use by Spacetop was not established.

The inspected installed CLI artifacts are arm64 Mach-O; installed Codex release
directories are all aarch64-apple-darwin. Native Linux x64 CLI/auth proof needs
compatible CLI artifacts, an existing-login context, and permitted user/PID
namespaces or a delegated cgroup. None was available through the inspected
surfaces. No CLI was downloaded or installed. Docker remains a test environment,
not an adopted runtime dependency. Evidence: `advance-linux-capabilities.json`.

## 3. CLI restrictions plus OS boundary: a partial working route

`claude_protocol.py` used a synthetic token, disposable HOME and local fake
Messages endpoint, with strict nofork and network restricted to that one port.
The exact model argument was `claude-haiku-5-5`. Two fake protocol requests
completed. Forged Bash and Read tool_use blocks returned explicit disabled-tool
errors; the seeded CLAUDE.md marker was absent from sent context. Outside files
were unchanged. This demonstrates those runtime restrictions, not successful
existing-login authentication or model comprehension. The CLI's usage/cost
fields are computed from canned fake usage and are **not actual billable usage**.
Evidence: `advance-claude-protocol.json`.

`advance-fake-oauth-path.json` separately confirms that a synthetic OAuth
environment variable is recognized under nofork without Keychain helpers.
That is only a parser/selection path. It does not validate a real token, prove
that an existing Keychain token can be reused through that variable, or authorize
a credential broker. Official [authentication documentation](https://code.claude.com/docs/en/authentication)
describes the variable for setup-token credentials, and distinguishes the
macOS Keychain from Linux's credential file. No such real token was obtained.

The Codex comparison preserved a custom provider from synthetic user config
without `--ignore-user-config`, kept exact `gpt-6-luna`, and did not send seeded
AGENTS.md text. Both its MCP startup control and `mcp_servers={}` variant failed
to activate the startup canary. Consequently this comparison establishes
provider-selection preservation but **does not prove MCP clearing**. Hook,
plugin and managed-configuration suppression still require discriminating
controls. The prior unadvertised view_image failure and explicit disable fix
remain intact. Evidence: `advance-codex-config-{control,clear-mcp}.json`.

## Next bounded discriminator

**Follow-up completed:** [KEYCHAIN.md](KEYCHAIN.md) records the one exact-item
query, its inconclusive `errSecItemNotFound` result and the explicit stop on
further same-kind auth metadata exploration. The original proposal below is
retained as provenance, not an instruction to rerun it.

Do not adopt a broker or new runtime on this evidence. One no-new-dependency
auth branch remains inconclusive. The smallest next local probe is a native
Security.framework query for the **statically verified exact CLI service and
selected config identity**, requesting only an opaque item reference/status
(`kSecReturnData=false`, match one), under the same narrow profile. Record only
OSStatus/presence, never attributes or credential values. It can distinguish
failure to find/authorize that item from the already-successful default-keychain
lookup. A successful metadata query still would not prove secret-value access;
an authorization error would identify a concrete per-item boundary to assess.
Do not enumerate Keychain entries or grant the whole database to make it pass.

That probe requires verifying the shipped CLI's service-name mapping first; the
mapping has not been guessed or queried here. If it names a capability that the
existing narrow profile can safely express, test that single change against
existing CLI auth status and the escape canaries. If it instead needs credential
access by a new broker, return its exact design for a separate decision then.
No complete dependency-free path or mandatory new dependency is established yet.

All five product ACs, native supported-host adapters, connected inference/refresh
egress, full customization suppression, reliable request cleanup and the real
unfamiliar-human walkthrough remain open. No UI/live work is ready to start.
