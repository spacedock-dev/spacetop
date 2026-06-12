# Release Policy

Spacetop releases are published through GitHub Releases. Local `make install`
builds remain available for contributors, but release assets are the supported
user install path.

## Release Trigger And Version Agreement

The root `Cargo.toml` `[workspace.package] version` is the source of truth.
Both `spacetop` and `spacetop-core` inherit this version.

The release workflow is triggered by publishing a GitHub Release. The workflow
loads the release tag from the GitHub Release event, not from a pushed tag event.

Release tags use the form `vX.Y.Z`. Pre-release tags use SemVer pre-release
syntax such as `v0.2.0-rc.1`.

Before release assets are uploaded, these values must match:

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
- `install.sh`

The macOS archive targets Apple Silicon. The Linux archive targets x64 GNU
Linux.

The README curl installer is the documented user install path for released
binaries. The README downloads `install.sh` from the latest GitHub Release, not
from a moving source branch. The installer depends on those two archive assets
and `SHA256SUMS` retaining the filenames above. The installer resolves the
latest release tag first, then downloads assets from the canonical
`/releases/download/<tag>/` URLs.

## Human Release Flow

1. Merge feature PRs to `main`.
2. Choose the next version.
3. Update the root `Cargo.toml` workspace version.
4. Run `cargo check` to update `Cargo.lock`.
5. Move relevant `CHANGELOG.md` entries from `Unreleased` to `vX.Y.Z`.
6. Commit with `release: vX.Y.Z`.
7. Record the exact release commit SHA: `release_commit="$(git rev-parse HEAD)"`.
8. Push `main`.
9. Create and publish a GitHub Release for tag `vX.Y.Z`, targeting
   `${release_commit}`.
10. Let GitHub Actions build release assets and upload them to that existing
   GitHub Release.
11. Verify the Release page contains both platform archives, `SHA256SUMS`, and
   `install.sh`.

CLI example:

```bash
gh release create vX.Y.Z \
  --target "${release_commit}" \
  --title vX.Y.Z \
  --notes-file RELEASE_NOTES.md
```

Creating a draft GitHub Release is allowed, but it does not build assets until
the draft is published. When creating a Release in the web UI, use a tag that
already points at the exact release commit or set the Release target to that
specific commit.

## Failure Policy

The release workflow fails before uploading assets when:

- the tag does not start with `v`
- the tag version is not valid SemVer
- the tag version differs from the workspace version
- `cargo fmt --check`, `cargo test`, or `make lint` fails
- any target binary fails to build
- the GitHub Release title differs from the tag
- any built binary reports the wrong version
- checksum generation fails
- the GitHub Release already has an asset with a name the workflow would upload

The workflow must not upload a partial platform set. If one platform fails, the
upload job does not run.

## Sentry

Release builds support compile-time `SENTRY_DSN` injection. The release workflow
does not require `SENTRY_DSN`.

If the secret exists, release binaries include it. If it is absent, release
binaries still build and Sentry stays disabled.
