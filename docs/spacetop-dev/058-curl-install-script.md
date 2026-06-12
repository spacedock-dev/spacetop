---
id: "058"
title: Add curl-based release installer
status: shape
source: captain request 2026-06-12
kind: feature
risk: medium
milestone: v1-maintenance
proof: shell/install tests plus README and release-policy alignment
started:
completed:
verdict:
score: 0.82
worktree:
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
