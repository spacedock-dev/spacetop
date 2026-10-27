# Exact-item discriminator: bounded pass closed

> Historical strict-isolation evidence. Kent's later trusted-CLI approval
> supersedes the implementation stop, not these observed failures. Current
> product contract and acceptance handoff: `docs/summary-verification.md`.

The one noninteractive metadata query returned `-25300` (`errSecItemNotFound`)
with no opaque reference. It identified no additional auth capability to grant.
Auth remains unresolved; helper lifecycle containment remains separately unmet.
This is diagnostic evidence, not product progress or readiness.

## Identity and query provenance

The installed Claude 2.1.309 arm64 binary has SHA-256
`5eadc0009cdcdbc0d06e58110d7b2baf88ced6088b1c33a60a21217fe410619c`.
Its embedded code, retained as bounded nonsecret snippets and byte offsets in
`evidence/keychain-mapping.json`, establishes this exact mapping:

- `E()` returns `prod`; production `OAUTH_FILE_SUFFIX` is empty.
- `vj(e)` concatenates `Claude Code`, the OAuth suffix, `e`, and an optional
  config-path hash. Auth lookup supplies `e = "-credentials"`.
- `CLAUDE_SECURESTORAGE_CONFIG_DIR` takes precedence over `CLAUDE_CONFIG_DIR`.
  Both are absent here, as is `CLAUDE_CODE_CUSTOM_OAUTH_URL`; the hash is absent.
  Therefore the service is exactly `Claude Code-credentials`.
- Account selection uses `USER` or userInfo and validates an ASCII allowlist.
  The current inherited USER satisfies it. No account attributes were queried.

The fixture rejects other builds, missing/invalid USER, and any presence of the
three config overrides (even empty), before compiling or querying. It does not
guess identities, enumerate entries or claim to support uninspected configs.
Static provenance includes a CLI value-reading command string; that command was
**not executed**. No credential file was opened by the diagnostic runner.

The installed Xcode macOS SDK's `Security.framework/Headers/SecKeychain.h`
documents `SecKeychainSetUserInteractionAllowed(false)` and its getter. The
native probe calls both and fails closed unless both succeed and the getter
reports false. This setting is process-local, not a global Keychain preference.
`SecItem.h` documents `kSecUseAuthenticationUIFail` returning an error when UI
would be needed. The query uses that additional guard. These legacy APIs are
deprecated but present in this SDK; compilation suppresses only their known
deprecation warnings, with other warnings treated as errors.

Exactly one `SecItemCopyMatching` call requested generic-password class, exact
service/account, `kSecMatchLimitOne`, and `kSecReturnRef=true`. ReturnData,
ReturnAttributes and ReturnPersistentRef were explicitly false. The opaque
reference is only tested for NULL and released. No content or attributes are
serialized. No value request, prompt, CLI auth call or provider call occurred.

## Observed result and limits

`evidence/keychain-presence.json` preserves the exact policy and statuses:
guard set/get both `0`, interaction allowed `false`, query `-25300`, reference
absent, exit `0`, stderr length `0`. The existing narrow policy denies network,
fork, outside file reads/writes and unlisted exec, and retains the previous
literal auth/config allowances. No HOME/Keychain subtree grant was added.
The native executable and private scratch are the only probe-specific paths.
The policy still defaults to allowing unclassified operations, including Mach;
this experiment is not a complete capability-policy proof.

This establishes that the exact item was not returned to this native caller
under this profile. It does **not** establish that the credential is absent,
that its value is inaccessible to the CLI's trusted helper, or that a broker
would work. No broader-read control was added. The native caller differs from
the installed `security` helper, and sandbox/search-list/caller filtering is
not distinguished by this status. No new narrow permission follows from it,
so the conditional one-delta auth/canary rerun was not triggered.

Per Navigator's stopping condition, this branch is now closed: no additional
same-kind auth metadata exploration, generic helper whitelist reruns, UI or
live calls. The live ledger remains **0/6**.

## Independent lifecycle conclusion

Auth success would not repair lifecycle containment. Historical evidence in
`ADVANCE.md` remains unchanged: direct setsid/setpgid calls can be denied, but
posix_spawn session/group attributes still escape; denying posix_spawn also
breaks the recorded positive Claude auth reference. The actual CLI auth path
uses `/bin/sh` and `/usr/bin/security`; these are the helpers whose creation a
strict nofork policy prevents. A trusted-helper exception would require an
explicit security-contract change. killpg and sampled PID trees are not
race-free cleanup and cannot be accepted as equivalent through typical-use
arguments. Default Mach access also remains outside the proved boundary.

Native Linux x64 still needs compatible CLI artifacts, an existing-login
context, and permission for a suitable lifecycle primitive (for example a
delegated cgroup or user/PID namespaces). The prior emulated container denied
unshare and exposed a read-only cgroup mount; that does not rule out other
Linux hosts or cut Linux support. Codex still needs discriminating suppression
controls, including the previously inconclusive MCP control. The exact
unadvertised view_image/auth-exception discovery is retained without weakening.

## Decision tabs: minimum next choices

### Option A - Recommended: retain the contract and keep summaries unavailable

| Pros | Cons | Choose this when |
|------|------|------------------|
| Preserves both providers/platforms and required cleanup; adds no credential owner or permission | Feature remains incomplete; a separately scoped architecture proposal is needed | Existing authorization and evidence are unchanged |

There is no remaining authorized local probe identified here that can settle
both auth and lifecycle readiness. Do not commission another generic metadata
round. This is not a proof that every dependency-free design is impossible.

### Option B - Authorize a credential-isolated nofork design investigation

| Pros | Cons | Choose this when |
|------|------|------------------|
| A concrete way to remove CLI auth-helper creation from the model process | Adds a trusted credential-handling component and needs explicit new permission; compatibility is unproved | The Captain permits Spacetop to become a credential owner for this narrow purpose |

The minimum proposed component is a native, non-model auth bridge inside the
request supervisor. On macOS it would query one exact Keychain value with UI
disabled; on Linux it would read the selected CLI credential file. It would
hand only required auth material to a fresh nofork CLI process in memory, never
logs or workflow files, and keep real auth/config paths unavailable to that
process. It must preserve configured provider/account semantics and fail
closed without asking for a new login. No external package is inherently
specified by this proposal, but it is a new security-sensitive product
component and credential access permission, not an already-authorized fix.
Exact token-shape/refresh compatibility, transport secrecy, in-process tool
access, complete OS capabilities and native Linux behavior are still required
proof. The prior fake OAuth recognition/protocol result proves neither real
existing-login token compatibility nor permission to transfer it. Current
evidence does not demonstrate this component is necessary or
sufficient. A first newly authorized investigation would inspect the shipped
token-selection/refresh dataflow and test a synthetic bridge protocol; no real
credential transfer or live call should follow from merely choosing to review
the design.

### Option C - Explicitly narrow the cleanup security contract

| Pros | Cons | Choose this when |
|------|------|------------------|
| Could permit existing CLI/helper login without introducing a credential bridge | Accepts trusted CLI/helper escape or orphan risk; still does not solve current strict-profile auth or prove readiness | The Captain intentionally accepts and records this residual risk |

This would name the pinned CLI, `/bin/sh` and `/usr/bin/security` as trusted
components whose descendants are outside the absolute cleanup guarantee.
Process-group cancellation would remain best effort and must be presented as
such: a forked descendant can change session/process group through posix_spawn
and remain alive after timeout, cancellation or parent exit. The known extra
permissions are process creation and exec of those exact helpers; the narrow
file-read policy still failed auth, so any further auth permission is unknown
and must not be presumed acceptable. It is a material contract change, not an engineering workaround or a
passing result. Do not implement it under the current assignment.

Recommendation: Option A under current authorization. If the Captain wants a
new scope, Option B is a concrete design investigation to review before any
credential handling; it is not an assertion that a broker is mandatory.
All five ACs, request-bound spinner, connected egress/refresh proof, real
unfamiliar-reader walkthrough and independent Navigator acceptance remain open.
