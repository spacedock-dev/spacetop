# Changelog

All notable changes to Spacetop are documented in this file.

Spacetop uses semantic versioning. While Spacetop remains below `1.0.0`, minor
versions may include breaking changes, and those changes are called out in the
release notes.

## Unreleased

### Added

### Changed

### Fixed

### Removed

### Internal

## v0.4.0 - 2026-10-05

This release covers changes since `v0.3.0`.

### Added

- Read-only durable Spacedock gate status and current-stage readiness inspection.
- A browser for verified gate Briefings, recorded decisions, and local evidence.
- An optional Herdr sidecar plugin that opens Spacetop beside the invoking agent,
  preserves its worktree context, and reuses the inspector within each workspace.
- Release archives and the installer now deliver both `spacetop` and
  `spacetop-herdr`; archives also include the Herdr manifest and setup docs.

### Changed

- Task lists and detail headers no longer display inferred runtime activity.
  This changes the visible task status indicators from v0.3.0.
- Detached and wrong-branch split-root checkouts remain readable, with explicit
  diagnostics and guarded sync eligibility.
- Both binaries report the matching v0.4.0 workspace version.

### Fixed

- Preserve runtime scan evidence across reloads to prevent activity flickering.
- Fail closed for escaped or unverifiable split-root sync targets.
- Preserve invalid gate metadata diagnostics and match upstream gate identity
  and inventory semantics.
- Package and install the Herdr helper with released binaries.

### Merged Pull Requests

- [#80](https://github.com/spacedock-dev/spacetop/pull/80) Surface detached split-root state checkouts.
- [#81](https://github.com/spacedock-dev/spacetop/pull/81) Investigate split-root state containment warning.
- [#82](https://github.com/spacedock-dev/spacetop/pull/82) Stop runtime activity indicator flickering between scans.
- [#83](https://github.com/spacedock-dev/spacetop/pull/83) Support durable Spacedock gate status.
- [#84](https://github.com/spacedock-dev/spacetop/pull/84) Browse gate briefings and decisions.
- [#85](https://github.com/spacedock-dev/spacetop/pull/85) Hide runtime activity from task list and detail header.
- [#86](https://github.com/spacedock-dev/spacetop/pull/86) Open Spacetop as a Herdr sidecar plugin.

Full changelog: https://github.com/spacedock-dev/spacetop/compare/v0.3.0...v0.4.0

## v0.2.0 - 2026-06-17

This release covers changes since `v0.1.0`.

### Added

- Active local agent session markers for tasks that match running Codex or Claude Code worktrees.
- GitHub Release deployment policy for macOS arm64 and Linux x64 binary assets.
- Published `install.sh` as a versioned GitHub Release asset.

### Changed

- `make install` remains a local developer install path rather than the user deployment path.
- Centralized entity identity handling in a core module.
- Unified agent code review policy documentation.
- Preview headers now separate source and worktree information across clearer lines.

### Fixed

- Archived tasks no longer remain in the active list until restart.
- Worktree copies of archived entities no longer reappear as active tasks.
- Workflow Definition pages support mouse wheel scrolling.
- Metrics, activity, and timeline views now surface clearer diagnostics when git history is unavailable.

### Removed

### Internal

- Documented an end-to-end GitHub Release runbook that can be executed from Claude Code or Codex.

### Merged Pull Requests

- [#60](https://github.com/spacedock-dev/spacetop/pull/60) Archived task remains in active list until restart.
- [#61](https://github.com/spacedock-dev/spacetop/pull/61) Workflow Definition page supports mouse scroll wheel.
- [#62](https://github.com/spacedock-dev/spacetop/pull/62) Preview header gives source and worktree separate lines.
- [#63](https://github.com/spacedock-dev/spacetop/pull/63) Diagnose history unavailable in metrics, activity, and timeline views.
- [#64](https://github.com/spacedock-dev/spacetop/pull/64) Publish installer as release asset.
- [#65](https://github.com/spacedock-dev/spacetop/pull/65) Unify agent code review policy.
- [#66](https://github.com/spacedock-dev/spacetop/pull/66) Worktree copies of archived entities reappear as active tasks.
- [#67](https://github.com/spacedock-dev/spacetop/pull/67) Centralize entity identity module.
- [#68](https://github.com/spacedock-dev/spacetop/pull/68) Agent session active marker.

Full changelog: https://github.com/spacedock-dev/spacetop/compare/v0.1.0...v0.2.0
