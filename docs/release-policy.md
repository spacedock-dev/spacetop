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

## End-To-End Agent Runbook

This process is designed to run entirely from Claude Code or Codex using local
shell commands and the `gh` CLI. Do not use the GitHub web UI unless the CLI is
unavailable.

Prerequisites:

- The local checkout is on `main` and includes every change intended for the
  release.
- `gh auth status` succeeds for the release repository.
- The release actor can push to `main` and create GitHub Releases.
- For the next release after the current `v0.1.0`, choose either a patch
  version such as `v0.1.1` or a minor version such as `v0.2.0` according to the
  version meaning above.

Preparation:

```bash
git switch main
git pull --ff-only
cargo fmt
cargo test
make lint
```

Set the release version once and reuse it for every command:

```bash
version=0.1.1
tag="v${version}"
```

Update the root `Cargo.toml` `[workspace.package] version` to `${version}`
using the agent's file editor, then refresh Cargo metadata:

```bash
cargo check
```

The workspace crates inherit this version. `cargo check` updates `Cargo.lock`
so its `spacetop` and `spacetop-core` package entries match the release tag.

Update `CHANGELOG.md` by moving release-ready entries from `Unreleased` under a
new heading:

```markdown
## v0.1.1 - YYYY-MM-DD
```

Use the actual release date in UTC. Keep a fresh `Unreleased` section above the
new version heading.

Run the completion gate again after the version and changelog edits:

```bash
cargo fmt
cargo test
make lint
```

Commit and push the exact release commit:

```bash
git status --short
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: ${tag}"
release_commit="$(git rev-parse HEAD)"
git push origin main
```

Create release notes without leaving the terminal. For a small release, write a
short `RELEASE_NOTES.md` from the changelog section:

```bash
awk -v tag="${tag}" '
  index($0, "## " tag) == 1 { in_section = 1; print; next }
  in_section && /^## / { exit }
  in_section { print }
' CHANGELOG.md > RELEASE_NOTES.md
```

Inspect and edit `RELEASE_NOTES.md` if needed. Then create and publish the
GitHub Release. Publishing the Release is the action that starts the release
workflow:

```bash
gh release create "${tag}" \
  --target "${release_commit}" \
  --title "${tag}" \
  --notes-file RELEASE_NOTES.md
```

Watch the release workflow from the terminal:

```bash
gh run list --workflow Release --limit 1
gh run watch
```

If `gh run watch` does not select the release run automatically, copy the run id
from `gh run list` and run:

```bash
gh run watch RUN_ID
```

After the workflow succeeds, verify the published assets:

```bash
gh release view "${tag}" --json tagName,name,isDraft,isPrerelease,targetCommitish,assets
gh release download "${tag}" --pattern SHA256SUMS --pattern install.sh --dir /tmp/spacetop-release-check
gh release download "${tag}" --pattern "spacetop-${tag}-*.tar.gz" --dir /tmp/spacetop-release-check
cd /tmp/spacetop-release-check
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c SHA256SUMS
else
  shasum -a 256 -c SHA256SUMS
fi
```

The release is complete only when the GitHub Release contains:

- `spacetop-${tag}-aarch64-apple-darwin.tar.gz`
- `spacetop-${tag}-x86_64-unknown-linux-gnu.tar.gz`
- `SHA256SUMS`
- `install.sh`

Clean local scratch files after verification:

```bash
rm -f RELEASE_NOTES.md
rm -rf /tmp/spacetop-release-check
```

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
