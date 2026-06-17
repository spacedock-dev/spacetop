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

## v0.2.0 - 2026-06-17

This release covers changes since `v0.1.0`.

### Added

- Active local agent session markers for tasks that match running Codex or
  Claude Code worktrees.
- GitHub Release deployment policy for macOS arm64 and Linux x64 binary assets.
- Published `install.sh` as a versioned GitHub Release asset.

### Changed

- `make install` remains a local developer install path rather than the user
  deployment path.
- Centralized entity identity handling in a core module.
- Unified agent code review policy documentation.
- Preview headers now separate source and worktree information across clearer
  lines.

### Fixed

- Archived tasks no longer remain in the active list until restart.
- Worktree copies of archived entities no longer reappear as active tasks.
- Workflow Definition pages support mouse wheel scrolling.
- Metrics, activity, and timeline views now surface clearer diagnostics when
  git history is unavailable.

### Removed

### Internal

- Documented an end-to-end GitHub Release runbook that can be executed from
  Claude Code or Codex.

### Merged Pull Requests

- [#60](https://github.com/spacedock-dev/spacetop/pull/60) Archived task
  remains in active list until restart.
- [#61](https://github.com/spacedock-dev/spacetop/pull/61) Workflow Definition
  page supports mouse scroll wheel.
- [#62](https://github.com/spacedock-dev/spacetop/pull/62) Preview header gives
  source and worktree separate lines.
- [#63](https://github.com/spacedock-dev/spacetop/pull/63) Diagnose history
  unavailable in metrics, activity, and timeline views.
- [#64](https://github.com/spacedock-dev/spacetop/pull/64) Publish installer as
  release asset.
- [#65](https://github.com/spacedock-dev/spacetop/pull/65) Unify agent code
  review policy.
- [#66](https://github.com/spacedock-dev/spacetop/pull/66) Worktree copies of
  archived entities reappear as active tasks.
- [#67](https://github.com/spacedock-dev/spacetop/pull/67) Centralize entity
  identity module.
- [#68](https://github.com/spacedock-dev/spacetop/pull/68) Agent session active
  marker.

Full changelog:
https://github.com/spacedock-dev/spacetop/compare/v0.1.0...v0.2.0
