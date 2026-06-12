---
id: "058"
title: Add curl-based release installer
status: implement
source: captain request 2026-06-12
kind: feature
risk: medium
milestone: v1-maintenance
proof: shell/install tests plus README and release-policy alignment
started: 2026-06-12T09:55:09Z
completed:
verdict:
score: 0.82
worktree: .worktrees/codex-058-curl-install-script
issue:
pr:
---

New users should be able to copy one install command from the GitHub repository
main page, paste it into a terminal, and get the latest released `spacetop`
binary installed without manually choosing a release asset.

The deliverable is a top-level shell installer script and matching README
install instructions. The script should detect supported operating systems and
architectures, download the corresponding archive from the latest GitHub
Release, verify the archive with the release checksum file, unpack `spacetop`,
install it into a user-writable bin directory by default, and fail clearly for
unsupported platforms.

## Scope

- Kind: feature
- Risk: medium
- Milestone: v1-maintenance
- Touches: docs / release / install tooling
- Non-goals: changing release asset names, adding package-manager formulas,
  mutating Spacedock workflow markdown, or supporting platforms not currently
  produced by the release workflow.

## Acceptance criteria

**AC-1 -- One-command GitHub README install path.**
The README shows a copy-paste command that fetches and runs the top-level
installer from the repository's main branch, with clear notes about the default
install directory and override variable.

Verified by: README diff review and an installer dry run that exercises command
construction without modifying a real system path.

**AC-2 -- Release asset selection matches current release policy.**
The installer maps supported platforms to the existing release assets:
`aarch64-apple-darwin` for macOS Apple Silicon and
`x86_64-unknown-linux-gnu` for Linux x64. Unsupported OS or CPU combinations
exit with a clear non-zero error.

Verified by: shell tests or scripted dry runs covering supported and
unsupported `uname` combinations.

**AC-3 -- Downloaded archive integrity is checked before install.**
The installer retrieves the latest GitHub Release archive and `SHA256SUMS`,
verifies the selected archive checksum before unpacking, and stops before
installing if verification fails or the checksum tool is unavailable.

Verified by: shell tests using mocked `curl`, archive, and checksum inputs, or
an equivalent deterministic local harness.

**AC-4 -- Install behavior is user-safe and auditable.**
The installer defaults to a user-writable bin directory, supports an explicit
install-dir override, avoids broad filesystem writes, cleans temporary files,
and verifies the installed binary with `spacetop --version`.

Verified by: local install test against a temporary directory plus code review
of write paths.

## Proof plan

- Lowest test layer: shell-level installer tests with mocked network/release
  assets, plus documentation assertions where practical.
- Required command: focused installer tests, `cargo test`, and `make lint` if
  Rust-side release tests or docs assertions are changed.
- Manual check, if any: run installer against a temporary install directory on
  the local platform when network access is available.
- Docs/policy update needed: README install section; update
  `docs/release-policy.md` only if the supported install path or asset contract
  changes.

## Implementation plan

Recommended approach: add a small POSIX `sh` installer at the repository root
and make the README point to that exact script through the raw GitHub URL. Keep
release asset names unchanged and treat tests as the main safety net for
platform selection, checksum handling, and filesystem writes.

### Files to change

- `install.sh`: new top-level installer. It should use `set -eu`, detect
  platform with `uname -s` and `uname -m`, map supported targets to the current
  release assets, download the latest release archive and `SHA256SUMS` with
  `curl`, verify checksum before extraction, install `spacetop` to
  `${SPACETOP_INSTALL_DIR:-$HOME/.cargo/bin}`, clean its temp directory on
  exit, and run the installed binary with `--version`.
- `README.md`: replace the manual "Install Released Binary" flow with a
  copy-paste command such as
  `curl -fsSL https://raw.githubusercontent.com/spacedock-dev/spacetop/main/install.sh | sh`,
  plus concise notes for `SPACETOP_INSTALL_DIR`, supported platforms, and local
  build installation.
- `docs/release-policy.md`: keep the supported asset list unchanged. Add one
  short note that the curl installer is the documented user install path and
  depends on the two archives plus `SHA256SUMS`.
- `.github/workflows/release.yml`: no release packaging change is expected.
  Touch only if implementation discovers that the latest-release installer
  requires an additional published asset or stricter upload invariant.
- `crates/spacetop/tests/install_script.rs`: new integration tests for the
  installer script using temporary directories and mocked command binaries.
- `crates/spacetop/tests/release_workflow.rs`: extend only for lightweight
  README/release-policy assertions if those docs need pinning alongside the
  existing release workflow checks.

### Installer behavior

1. Resolve the GitHub repository as
   `${SPACETOP_REPO:-spacedock-dev/spacetop}` and latest release base URL as
   `https://github.com/${SPACETOP_REPO}/releases/latest/download`.
2. Resolve install directory as `${SPACETOP_INSTALL_DIR:-$HOME/.cargo/bin}`.
   Fail clearly when neither `SPACETOP_INSTALL_DIR` nor `HOME` gives a usable
   absolute directory. Create only that directory and the script-owned temp
   directory.
3. Map platform values exactly:
   `Darwin` plus `arm64` or `aarch64` to `aarch64-apple-darwin`, and `Linux`
   plus `x86_64` or `amd64` to `x86_64-unknown-linux-gnu`. Every other
   OS/arch pair exits non-zero with a message that names the detected pair.
4. Download `SHA256SUMS` and the selected
   `spacetop-v<version>-<target>.tar.gz` from the latest-release URL. Prefer
   deriving the archive name from the checksum file entry for the selected
   target so the script does not need to call the GitHub API.
5. Verify checksums before extraction. Use `sha256sum -c` when available, then
   `shasum -a 256 -c` as the macOS fallback. If neither tool exists, fail before
   unpacking.
6. Extract with `tar -xzf` into the temp directory, install with
   `install -m 755` when available, and fall back to `mkdir -p` plus `cp` plus
   `chmod 755` only for portability. Do not use `sudo` automatically.
7. Verify the installed binary by running
   `${SPACETOP_INSTALL_DIR}/spacetop --version`; surface the binary path in the
   success message.

### Lowest-layer test plan

- Add `crates/spacetop/tests/install_script.rs` because the installer is a
  repo-root shell script and Cargo integration tests already run in CI through
  `cargo test`.
- In the test harness, create a temp `PATH` containing fake `uname`, `curl`,
  checksum tools, `tar`, and `install` commands. Each fake command should record
  its arguments in temp files so assertions can inspect behavior without network
  access or real system writes.
- OS/arch selection tests:
  - `Darwin` plus `arm64` selects `aarch64-apple-darwin`.
  - `Linux` plus `x86_64` selects `x86_64-unknown-linux-gnu`.
  - an unsupported pair such as `Darwin` plus `x86_64` exits non-zero and names
    the unsupported platform.
- Checksum tests:
  - successful checksum allows extraction and install.
  - checksum mismatch exits before `tar` or install runs.
  - missing `sha256sum` falls back to `shasum -a 256 -c`.
  - missing both checksum tools exits before extraction.
- Temp-dir and install behavior tests:
  - `SPACETOP_INSTALL_DIR` pointing at a temp bin directory receives only the
    `spacetop` binary and no writes occur under the repository or workflow
    directories.
  - the temp directory is removed after both success and failure.
  - `spacetop --version` is invoked from the installed binary path.
- Documentation assertion:
  - extend `release_workflow.rs` or add a small adjacent integration test to
    pin that `README.md` advertises the raw GitHub `install.sh` command and
    `SPACETOP_INSTALL_DIR`.

### Verification commands

Run, in order:

```bash
cargo fmt
cargo test -p spacetop --test install_script
cargo test
make lint
```

If network access is available, optionally run a manual dry install against a
temporary directory after the deterministic tests pass:

```bash
SPACETOP_INSTALL_DIR="$(mktemp -d)" sh ./install.sh
```

### Release-policy alignment and assumptions

- No release asset rename is planned. The installer must consume the existing
  `spacetop-vX.Y.Z-aarch64-apple-darwin.tar.gz`,
  `spacetop-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`, and `SHA256SUMS` assets.
- The plan assumes GitHub's `releases/latest/download/<asset>` URL resolves to
  the latest non-draft release assets for `spacedock-dev/spacetop`.
- The plan assumes each `SHA256SUMS` row uses the basename produced by the
  current release workflow, which runs `sha256sum *.tar.gz > SHA256SUMS` inside
  `dist`.
- Intel macOS, Linux ARM, musl Linux, and Windows remain unsupported until the
  release workflow publishes matching assets.

## Stage Report: plan

- DONE: Names exact files/modules to change for a top-level curl installer and README install path.
  See `install.sh`, `README.md`, `docs/release-policy.md`, `.github/workflows/release.yml`, and `crates/spacetop/tests/install_script.rs` in the implementation plan above.
- DONE: Specifies a lowest-layer test plan for OS/arch selection, checksum verification, and temp-dir install behavior.
  The test plan uses Cargo integration tests around the shell script with mocked `uname`, `curl`, checksum tools, `tar`, and `install`.
- DONE: Calls out release-policy alignment and any assumptions about GitHub latest-release asset names before implementation.
  The release-policy alignment section preserves current asset names and states the `releases/latest/download` and `SHA256SUMS` assumptions.

### Summary

Added a concrete plan for a repo-root POSIX shell installer, README install
surface, release-policy note, and deterministic shell-script test coverage. The
plan keeps the current release asset contract intact and defers any workflow
change unless implementation proves another asset invariant is required.
