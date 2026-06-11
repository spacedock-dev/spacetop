# Release Policy

Spacetop releases are published through GitHub Releases. Local `make install`
builds remain available for contributors, but release assets are the supported
user install path.

## Version Source Of Truth

The root `Cargo.toml` `[workspace.package] version` is the source of truth.
Both `spacetop` and `spacetop-core` inherit this version.

Release tags use the form `vX.Y.Z`. Pre-release tags use SemVer pre-release
syntax such as `v0.2.0-rc.1`.

Before publishing, these values must match:

- root `Cargo.toml` workspace version
- `Cargo.lock` package entries for `spacetop` and `spacetop-core`
- git tag without the leading `v`
- `spacetop --version`
- GitHub Release title
- release asset filenames

## Version Meaning

- `PATCH`: bug fixes, UI polish, documentation updates, release workflow fixes,
  and behavior-preserving refactors.
- `MINOR`: new read-only features, compatible CLI flags, compatible parser
  support for additional Spacedock metadata, new views, and new export surfaces.
- `MAJOR`: breaking CLI behavior, removed flags, incompatible output changes, or
  a product-contract change that affects user expectations.

While Spacetop remains below `1.0.0`, breaking changes may happen in minor
releases. Release notes must call them out explicitly.

## Supported Release Assets

The first supported binary assets are:

- `spacetop-vX.Y.Z-aarch64-apple-darwin.tar.gz`
- `spacetop-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
- `SHA256SUMS`

The macOS archive targets Apple Silicon. The Linux archive targets x64 GNU
Linux.

## Human Release Flow

1. Merge feature PRs to `main`.
2. Choose the next version.
3. Update the root `Cargo.toml` workspace version.
4. Run `cargo check` to update `Cargo.lock`.
5. Move relevant `CHANGELOG.md` entries from `Unreleased` to `vX.Y.Z`.
6. Commit with `release: vX.Y.Z`.
7. Tag with `vX.Y.Z`.
8. Push `main` and the tag.
9. Let GitHub Actions build release assets.
10. Review and publish the draft GitHub Release.

## Failure Policy

The release workflow fails before publishing when:

- the tag does not start with `v`
- the tag version is not valid SemVer
- the tag version differs from the workspace version
- `cargo fmt --check`, `cargo test`, or `make lint` fails
- any target binary fails to build
- any built binary reports the wrong version
- checksum generation fails
- a GitHub Release or asset already exists for the tag

The workflow must not publish a partial release. If one platform fails, the
whole release fails.

## Sentry

Release builds support compile-time `SENTRY_DSN` injection. The release workflow
does not require `SENTRY_DSN`.

If the secret exists, release binaries include it. If it is absent, release
binaries still build and Sentry stays disabled.
