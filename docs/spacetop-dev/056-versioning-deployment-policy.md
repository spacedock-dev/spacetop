---
id: "056"
title: "Implement versioning and GitHub Release deployment policy"
status: implement
source: "captain - approved versioning/deployment policy design and plan"
kind: feature
risk: medium
milestone: v1-maintenance
proof: "cargo fmt --check; cargo test; make lint; SENTRY_DSN= cargo build --release -p spacetop; target/release/spacetop --version"
started: 2026-06-11
completed:
verdict:
score: 0.99
worktree: .worktrees/spacedock-ensign-056-versioning-deployment-policy
issue:
pr: "#50"
mod-block: merge:pr-merge
---

Implement the approved versioning and deployment policy for Spacetop. The work
adds a real version surface, release documentation, CI, and a GitHub Release
workflow that publishes macOS arm64 and Linux x64 binary archives.

Design: `docs/superpowers/specs/2026-06-11-spacetop-versioning-deployment-policy-design.md`

Plan: `docs/superpowers/plans/2026-06-11-spacetop-versioning-deployment-policy.md`

## Scope

- Kind: feature
- Risk: medium
- Milestone: v1-maintenance
- Touches: CLI, README, release docs, changelog, GitHub Actions CI, GitHub Actions release workflow
- Non-goals: Homebrew tap, crates.io publishing, Windows binaries, package signing, notarization, auto-update support, product behavior changes beyond `spacetop --version`

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Spacetop exposes a reliable version surface.**
`spacetop --version` reports the Cargo workspace package version inherited by the `spacetop` binary.
Verified by: focused Clap test, `cargo test`, and `target/release/spacetop --version`.

**AC-2 -- GitHub Actions CI protects mainline quality.**
Pull requests and pushes to `main` run formatting, tests, and lint with read-only token permissions.
Verified by: `.github/workflows/ci.yml` review and local command parity.

**AC-3 -- GitHub Release workflow publishes the supported binary set.**
Tag-triggered releases validate tag/workspace/binary version agreement, build `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`, package `.tar.gz` assets, generate `SHA256SUMS`, and create a draft GitHub Release without partial publication.
Verified by: `.github/workflows/release.yml` review, YAML parse checks, and release workflow static checks in the implementation plan.

**AC-4 -- User and maintainer documentation matches the new release policy.**
README install guidance points users at GitHub Release assets, while `make install` remains documented as a local developer path; release policy and changelog documents exist.
Verified by: README, `docs/release-policy.md`, and `CHANGELOG.md`.

## Proof plan

- Lowest test layer: CLI unit test for version output; static workflow checks for GitHub Actions; docs diff checks for release guidance.
- Required commands: `cargo fmt --check`; `cargo test`; `make lint`; `SENTRY_DSN= cargo build --release -p spacetop`; `target/release/spacetop --version`.
- Manual check, if any: no live tag release required in the implementation stage.
- Docs/policy update needed: README install guidance, `docs/release-policy.md`, and `CHANGELOG.md`.
