# Spacetop Versioning And Deployment Policy - Design

**Status:** Approved design
**Date:** 2026-06-11
**Author:** brainstorming session (captain: Kent)

## Summary

Spacetop needs a real release policy before it grows beyond local developer
installs. Today the project has a workspace version in `Cargo.toml`, but users
still install by building from source with `make install`. This design keeps
that local path for contributors while making GitHub Releases the supported
distribution path.

The first deployment target is intentionally narrow:

- macOS arm64: `aarch64-apple-darwin`
- Linux x64: `x86_64-unknown-linux-gnu`

Package-manager distribution, Homebrew taps, crates.io publishing, installers,
and auto-update support are deferred until the GitHub Release flow is stable.

## Goals

1. Define one version source of truth for the workspace and release artifacts.
2. Make `spacetop --version` report the same version that is tagged and shipped.
3. Add a GitHub Actions release flow that builds and publishes binary assets for
   macOS arm64 and Linux x64.
4. Keep `make install` as a local developer install, not the deployment story.
5. Prevent partial or mismatched releases.
6. Document a small, repeatable release process that a maintainer or agent can
   follow without guessing.

## Non-Goals

- No Homebrew tap in the first release pass.
- No crates.io publish in the first release pass.
- No Windows binary in the first release pass.
- No package signing or notarization in the first release pass.
- No auto-update mechanism.
- No semantic-version compatibility checker.
- No change to Spacetop's read-only product contract.

## Versioning Policy

The root `Cargo.toml` `[workspace.package] version` is the source of truth.
Both `spacetop` and `spacetop-core` inherit `version.workspace = true`.

Release tags use `vX.Y.Z` or a SemVer pre-release tag such as
`v0.2.0-rc.1`.

The release process must ensure these values match:

- root `Cargo.toml` workspace version
- `Cargo.lock` package entries for `spacetop` and `spacetop-core`
- git tag without the leading `v`
- `spacetop --version`
- GitHub Release title
- release asset filenames

### SemVer Meaning

Spacetop should use SemVer conservatively:

- `PATCH`: bug fixes, small UI polish, documentation updates, release workflow
  fixes, and behavior-preserving refactors.
- `MINOR`: new read-only features, compatible CLI flags, compatible parser
  support for additional Spacedock metadata, new views, and new export surfaces.
- `MAJOR`: breaking CLI behavior, removed flags, incompatible output changes,
  or a product-contract change that affects user expectations.

While the project remains under `1.0.0`, breaking changes may still happen in
minor releases, but the release notes must call them out explicitly.

### Pre-Releases

Use pre-releases for risky releases or release-candidate testing:

- `v0.2.0-alpha.1` for early preview artifacts.
- `v0.2.0-rc.1` for release candidates expected to become stable with minimal
  change.

Pre-release GitHub Releases should be marked as pre-release and should not be
called "latest" unless a maintainer explicitly decides otherwise.

## CLI Version Surface

The `spacetop` CLI should expose the Cargo package version via Clap:

```rust
#[derive(Debug, Clone, Parser)]
#[command(
    name = "spacetop",
    version,
    about = "Inspect Spacedock workflow state from the terminal.",
    long_about = "Spacetop is a read-only terminal UI for browsing Spacedock workflow state files."
)]
pub struct Cli {
    // ...
}
```

Tests should assert that `spacetop --version` is available through the Clap
definition. The release workflow should execute the built binary and compare
the reported version with the git tag.

## Deployment Policy

GitHub Releases are the supported deployment surface.

`make install` remains available, but only as a local developer convenience:

- It builds from the current checkout.
- It installs to `~/.cargo/bin/spacetop` by default.
- It is not a reproducible deployment path for users.
- It is not the basis for release notes or support.

The README should present GitHub Release downloads as the user install path and
move `make install` under contributor/development instructions.

## GitHub Actions Design

Create two workflows:

1. `.github/workflows/ci.yml`
2. `.github/workflows/release.yml`

GitHub documentation requires workflow files to live under
`.github/workflows`. GitHub's Rust Actions guide supports using the same Cargo
build and test commands used locally, and GitHub Releases can attach compiled
binary files.

### CI Workflow

CI runs on pull requests and pushes to `main`.

Required jobs:

- `cargo fmt --check`
- `cargo test`
- `make lint`

The CI workflow should use restricted permissions:

```yaml
permissions:
  contents: read
```

The workflow may cache Cargo registry, git, and `target/` state using
`actions/cache`, keyed by OS and `Cargo.lock`.

### Release Workflow

The release workflow runs on tags matching `v*`.

It must:

1. Check out the tagged commit.
2. Extract the tag version by stripping the leading `v`.
3. Read the workspace version from `Cargo.toml`.
4. Fail if tag version and workspace version differ.
5. Build release binaries for:
   - `aarch64-apple-darwin`
   - `x86_64-unknown-linux-gnu`
6. Run the built binary with `--version`.
7. Fail if binary version and tag version differ.
8. Package each binary into a `.tar.gz`.
9. Generate `SHA256SUMS`.
10. Create a draft GitHub Release or publish directly, depending on the chosen
    release maturity.
11. Upload all assets.

Recommended initial behavior: create a draft release first. This allows assets
and notes to be checked before publishing. Once the process has been exercised,
the team can switch to direct publish.

The release workflow needs explicit write permission only where release assets
are created:

```yaml
permissions:
  contents: write
```

All non-release jobs should keep read-only token permissions.

## Artifact Policy

Release assets should use stable names:

```text
spacetop-vX.Y.Z-aarch64-apple-darwin.tar.gz
spacetop-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
SHA256SUMS
```

Each archive contains:

```text
spacetop
README.md
LICENSE or license text, once present
```

If a `LICENSE` file does not yet exist, the first implementation should either
add the project license file or omit it explicitly and record that follow-up.

## Release Notes Policy

Add `CHANGELOG.md` before the first release.

The changelog should use this structure:

```markdown
## vX.Y.Z - YYYY-MM-DD

### Added
### Changed
### Fixed
### Removed
### Internal
```

The GitHub Release notes should be derived from the changelog section for that
version. Release notes should also include:

- supported platforms
- installation command examples
- checksum verification hint
- any known limitations

## Sentry Policy

Release builds already support compile-time `SENTRY_DSN` injection. The release
workflow should not require `SENTRY_DSN`.

Rules:

- If `SENTRY_DSN` is present in GitHub Secrets, release binaries include it.
- If `SENTRY_DSN` is absent, release binaries still build and Sentry stays
  disabled.
- The release notes do not need to mention whether Sentry was compiled in.

## Failure Policy

The release workflow must fail before publishing if:

- the tag does not start with `v`
- the tag version is not valid SemVer
- the tag version differs from the workspace version
- `cargo test` or `make lint` fails
- any target binary fails to build
- any built binary reports the wrong version
- checksum generation fails

The release workflow must not publish a partial release. If one platform fails,
the whole release fails.

If a GitHub Release or asset already exists for the tag, the workflow should
fail rather than overwrite it.

## Release Process

The human release flow is:

```text
1. Merge feature PRs to main.
2. Choose the next version.
3. Update root Cargo.toml workspace version.
4. Update Cargo.lock.
5. Update CHANGELOG.md.
6. Commit: release: vX.Y.Z
7. Tag: vX.Y.Z
8. Push main and the tag.
9. Let GitHub Actions build release assets.
10. Review and publish the draft GitHub Release.
```

The workflow should document exact commands after implementation, but the design
does not mandate one release script. A small helper script is acceptable if it
keeps the GitHub Actions workflow readable.

## Implementation Units

This design should be implemented in one focused task:

1. Add CLI version support and tests.
2. Add `CHANGELOG.md`.
3. Add release policy documentation.
4. Add CI workflow.
5. Add release workflow.
6. Update README install instructions.
7. Verify local commands and static workflow structure.

No Rust product behavior should change except exposing `--version`.

## Open Questions

1. Whether the first release workflow should publish immediately or always create
   a draft. Recommendation: draft first.
2. Whether Linux x64 should use native `x86_64-unknown-linux-gnu` on Ubuntu or a
   musl static target. Recommendation: start with GNU for lower setup risk.
3. Whether to add artifact attestation in the first pass. Recommendation: defer
   until binary release basics are stable.

## References

- GitHub Actions workflow syntax:
  https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
- GitHub Actions Rust build/test guide:
  https://docs.github.com/en/actions/tutorials/build-and-test-code/rust
- GitHub Releases:
  https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository
