# Spacetop Versioning And Deployment Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a versioning and GitHub Release deployment path for Spacetop, shipping macOS arm64 and Linux x64 binary archives while keeping `make install` as a local developer install.

**Architecture:** Use the root Cargo workspace version as the source of truth, expose it through Clap, and make GitHub Actions enforce tag/package/binary version agreement. CI remains read-only and release publication is isolated to a tag-triggered workflow with explicit `contents: write` permission.

**Tech Stack:** Rust 2021, Cargo workspace, Clap, Make, Bash, GitHub Actions, GitHub Releases, `gh` CLI in Actions, tar/gzip, SHA-256 checksums.

---

## File Map

- Modify `crates/spacetop/src/cli.rs`: expose `spacetop --version` and add CLI tests.
- Create `CHANGELOG.md`: establish the release-note source for the current unreleased work.
- Create `docs/release-policy.md`: document versioning, supported platforms, release commands, and failure policy.
- Modify `README.md`: make GitHub Release downloads the user install path and move `make install` under local development.
- Create `.github/workflows/ci.yml`: run format, tests, and lint on PRs and pushes to `main`.
- Create `.github/workflows/release.yml`: build macOS arm64 and Linux x64 assets, validate versions, generate checksums, and create a draft GitHub Release.

No implementation task should modify `docs/spacetop-dev/README.md`, `docs/spacetop-dev/_mods/pr-merge.md`, or the existing untracked v2 plan files.

## Task 1: Expose CLI Version

**Files:**
- Modify: `crates/spacetop/src/cli.rs`

- [ ] **Step 1: Add a failing test for the version flag**

Add this test inside the existing `#[cfg(test)] mod tests` in `crates/spacetop/src/cli.rs`:

```rust
    #[test]
    fn version_output_uses_workspace_package_version() {
        let version = Cli::command().render_version().to_string();

        assert!(
            version.contains(env!("CARGO_PKG_VERSION")),
            "version output `{version}` did not contain Cargo package version `{}`",
            env!("CARGO_PKG_VERSION")
        );
    }
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```bash
cargo test -p spacetop version_output_uses_workspace_package_version
```

Expected: the test fails because the Clap command has no configured version text.

- [ ] **Step 3: Enable Clap version output**

Modify the `#[command(...)]` attribute in `crates/spacetop/src/cli.rs` to include `version`:

```rust
#[derive(Debug, Clone, Parser)]
#[command(
    name = "spacetop",
    version,
    about = "Inspect Spacedock workflow state from the terminal.",
    long_about = "Spacetop is a read-only terminal UI for browsing Spacedock workflow state files."
)]
pub struct Cli {
    /// Path to a Spacedock workflow directory. When omitted, SpaceTop
    /// discovers workflows under the current git root.
    #[arg(short = 'w', long, value_name = "PATH")]
    pub workflow_dir: Option<PathBuf>,
}
```

- [ ] **Step 4: Run the focused test and verify it passes**

Run:

```bash
cargo test -p spacetop version_output_uses_workspace_package_version
```

Expected: the focused test passes.

- [ ] **Step 5: Run all CLI tests**

Run:

```bash
cargo test -p spacetop cli::tests
```

Expected: all CLI tests pass.

- [ ] **Step 6: Commit the CLI version support**

Run:

```bash
git add crates/spacetop/src/cli.rs
git commit -m "feat: expose spacetop version"
```

## Task 2: Add Release Notes And Release Policy Docs

**Files:**
- Create: `CHANGELOG.md`
- Create: `docs/release-policy.md`

- [ ] **Step 1: Create `CHANGELOG.md`**

Create `CHANGELOG.md` with this content:

```markdown
# Changelog

All notable changes to Spacetop are documented in this file.

Spacetop uses semantic versioning. While Spacetop remains below `1.0.0`, minor
versions may include breaking changes, and those changes are called out in the
release notes.

## Unreleased

### Added

- GitHub Release deployment policy for macOS arm64 and Linux x64 binary assets.

### Changed

- `make install` remains a local developer install path rather than the user
  deployment path.

### Fixed

### Removed

### Internal
```

- [ ] **Step 2: Create `docs/release-policy.md`**

Create `docs/release-policy.md` with this content:

```markdown
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
- `cargo test` or `make lint` fails
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
```

- [ ] **Step 3: Check docs for formatting issues**

Run:

```bash
git diff --check -- CHANGELOG.md docs/release-policy.md
```

Expected: no output and exit code 0.

- [ ] **Step 4: Commit release docs**

Run:

```bash
git add CHANGELOG.md docs/release-policy.md
git commit -m "docs: add release policy"
```

## Task 3: Update README Installation Guidance

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace the local-install section with release-first install guidance**

In `README.md`, replace the current `### Install Local Build` section with:

```markdown
### Install Released Binary

Download the archive for your platform from the GitHub Releases page:

- macOS Apple Silicon: `spacetop-vX.Y.Z-aarch64-apple-darwin.tar.gz`
- Linux x64: `spacetop-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`

Verify the archive against `SHA256SUMS`, then unpack it and move `spacetop` into
a directory on your `PATH`.

Example:

```bash
tar -xzf spacetop-vX.Y.Z-aarch64-apple-darwin.tar.gz
install -m 755 spacetop ~/.cargo/bin/spacetop
spacetop --version
```

### Install Local Build

Contributors can still build and install from the current checkout:

```bash
make build
make install
```

By default, install places the binary at `~/.cargo/bin/spacetop`.

To install to a different location, override `PREFIX`:

```bash
make install PREFIX=/usr/local/bin
```

To remove the installed binary:

```bash
make uninstall
```
```

- [ ] **Step 2: Add release policy to the development command list**

In the "Development" command block in `README.md`, keep the existing commands and add no new command. After the workspace-layout list, add:

```markdown
Release and versioning policy lives in `docs/release-policy.md`.
```

- [ ] **Step 3: Check README formatting**

Run:

```bash
git diff --check -- README.md
```

Expected: no output and exit code 0.

- [ ] **Step 4: Commit README install documentation**

Run:

```bash
git add README.md
git commit -m "docs: document release install path"
```

## Task 4: Add Pull Request CI Workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the workflow directory**

Run:

```bash
mkdir -p .github/workflows
```

Expected: `.github/workflows` exists.

- [ ] **Step 2: Create `.github/workflows/ci.yml`**

Create `.github/workflows/ci.yml` with this content:

```yaml
name: CI

on:
  pull_request:
  push:
    branches:
      - main

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Format, test, and lint
    runs-on: ubuntu-24.04

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache Cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ci-${{ runner.os }}-${{ hashFiles('Cargo.lock') }}
          restore-keys: |
            ci-${{ runner.os }}-

      - name: Check formatting
        run: cargo fmt --check

      - name: Run tests
        run: cargo test

      - name: Run lint
        run: make lint
```

- [ ] **Step 3: Run YAML syntax sanity check if Ruby is available**

Run:

```bash
ruby -e 'require "yaml"; YAML.load_file(".github/workflows/ci.yml"); puts "ci yaml ok"'
```

Expected: `ci yaml ok`. If Ruby is not available, run this instead:

```bash
sed -n '1,160p' .github/workflows/ci.yml
```

Expected: the workflow content matches Step 2.

- [ ] **Step 4: Commit CI workflow**

Run:

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add pull request checks"
```

## Task 5: Add Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create `.github/workflows/release.yml`**

Create `.github/workflows/release.yml` with this content:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always

jobs:
  validate:
    name: Validate release version
    runs-on: ubuntu-24.04
    outputs:
      version: ${{ steps.version.outputs.version }}
      tag: ${{ steps.version.outputs.tag }}

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Validate tag and workspace version
        id: version
        shell: bash
        run: |
          set -euo pipefail

          tag="${GITHUB_REF_NAME}"
          if [[ ! "${tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
            echo "release tag must look like vX.Y.Z or vX.Y.Z-prerelease: ${tag}" >&2
            exit 1
          fi

          version="${tag#v}"
          cargo_version="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; data=json.load(sys.stdin); print(data["packages"][0]["version"])')"

          if [[ "${version}" != "${cargo_version}" ]]; then
            echo "tag version ${version} does not match Cargo version ${cargo_version}" >&2
            exit 1
          fi

          if gh release view "${tag}" >/dev/null 2>&1; then
            echo "GitHub Release ${tag} already exists" >&2
            exit 1
          fi

          echo "version=${version}" >> "${GITHUB_OUTPUT}"
          echo "tag=${tag}" >> "${GITHUB_OUTPUT}"
        env:
          GH_TOKEN: ${{ github.token }}

      - name: Run tests
        run: cargo test

      - name: Run lint
        run: make lint

  build:
    name: Build ${{ matrix.target }}
    needs: validate
    strategy:
      fail-fast: true
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-14
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-24.04
    runs-on: ${{ matrix.os }}

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache Cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: release-${{ matrix.target }}-${{ hashFiles('Cargo.lock') }}
          restore-keys: |
            release-${{ matrix.target }}-

      - name: Build release binary
        run: cargo build --release --target "${{ matrix.target }}" -p spacetop
        env:
          SENTRY_DSN: ${{ secrets.SENTRY_DSN }}

      - name: Validate binary version
        shell: bash
        run: |
          set -euo pipefail
          binary="target/${{ matrix.target }}/release/spacetop"
          actual="$("${binary}" --version)"
          expected="spacetop ${{ needs.validate.outputs.version }}"
          if [[ "${actual}" != "${expected}" ]]; then
            echo "expected '${expected}', got '${actual}'" >&2
            exit 1
          fi

      - name: Package archive
        shell: bash
        run: |
          set -euo pipefail
          version="${{ needs.validate.outputs.version }}"
          target="${{ matrix.target }}"
          package_dir="dist/spacetop-v${version}-${target}"
          mkdir -p "${package_dir}"
          cp "target/${target}/release/spacetop" "${package_dir}/"
          cp README.md "${package_dir}/"
          cp CHANGELOG.md "${package_dir}/"
          tar -C dist -czf "dist/spacetop-v${version}-${target}.tar.gz" "spacetop-v${version}-${target}"

      - name: Upload archive artifact
        uses: actions/upload-artifact@v4
        with:
          name: spacetop-${{ matrix.target }}
          path: dist/spacetop-v${{ needs.validate.outputs.version }}-${{ matrix.target }}.tar.gz
          if-no-files-found: error

  publish:
    name: Publish draft GitHub Release
    needs:
      - validate
      - build
    runs-on: ubuntu-24.04

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Download release archives
        uses: actions/download-artifact@v4
        with:
          path: dist
          merge-multiple: true

      - name: Generate checksums
        shell: bash
        run: |
          set -euo pipefail
          cd dist
          shasum -a 256 *.tar.gz > SHA256SUMS

      - name: Extract release notes
        shell: bash
        run: |
          set -euo pipefail
          version="${{ needs.validate.outputs.version }}"
          awk -v version="v${version}" '
            $0 == "## " version { capture=1; next }
            capture && /^## / { exit }
            capture { print }
          ' CHANGELOG.md > RELEASE_NOTES.md
          if [[ ! -s RELEASE_NOTES.md ]]; then
            cat > RELEASE_NOTES.md <<EOF
          Spacetop v${version}

          See CHANGELOG.md for release details.
          EOF
          fi

      - name: Create draft release
        shell: bash
        run: |
          set -euo pipefail
          tag="${{ needs.validate.outputs.tag }}"
          gh release create "${tag}" dist/*.tar.gz dist/SHA256SUMS \
            --draft \
            --title "${tag}" \
            --notes-file RELEASE_NOTES.md
        env:
          GH_TOKEN: ${{ github.token }}
```

- [ ] **Step 2: Fix the release-notes extraction checkout gap**

In `.github/workflows/release.yml`, the publish job's `Extract release notes` step reads `CHANGELOG.md` from the repository checkout, not from `dist`. Confirm the `Checkout` step stays before `Download release archives`.

Run:

```bash
rg -n "name: Checkout|name: Download release archives|awk -v version" .github/workflows/release.yml
```

Expected: `Checkout` appears before `Download release archives`, and the `awk` line appears in the publish job.

- [ ] **Step 3: Run YAML syntax sanity check if Ruby is available**

Run:

```bash
ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml"); puts "release yaml ok"'
```

Expected: `release yaml ok`. If Ruby is not available, run this instead:

```bash
sed -n '1,260p' .github/workflows/release.yml
```

Expected: the workflow content matches Step 1.

- [ ] **Step 4: Commit release workflow**

Run:

```bash
git add .github/workflows/release.yml
git commit -m "ci: add GitHub release workflow"
```

## Task 6: Local Verification Pass

**Files:**
- Verify only. Do not modify files unless a command fails and the failure points to an implementation defect from Tasks 1-5.

- [ ] **Step 1: Check formatting**

Run:

```bash
cargo fmt --check
```

Expected: exit code 0.

- [ ] **Step 2: Run tests**

Run:

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 3: Run lint**

Run:

```bash
make lint
```

Expected: clippy completes with no warnings.

- [ ] **Step 4: Build release binary locally**

Run:

```bash
SENTRY_DSN= cargo build --release -p spacetop
```

Expected: release build succeeds.

- [ ] **Step 5: Validate local version output**

Run:

```bash
target/release/spacetop --version
```

Expected: output is `spacetop 0.1.0` until the next release version bump.

- [ ] **Step 6: Check release docs and workflows are included in git**

Run:

```bash
git status --short
```

Expected: no uncommitted changes from the implementation tasks. Existing unrelated files that predated this plan may still appear if they were intentionally left untouched; do not include them in implementation commits.

## Task 7: Final Review And Handoff

**Files:**
- Verify: `docs/superpowers/specs/2026-06-11-spacetop-versioning-deployment-policy-design.md`
- Verify: `CHANGELOG.md`
- Verify: `docs/release-policy.md`
- Verify: `README.md`
- Verify: `.github/workflows/ci.yml`
- Verify: `.github/workflows/release.yml`
- Verify: `crates/spacetop/src/cli.rs`

- [ ] **Step 1: Confirm spec coverage**

Run:

```bash
rg -n "macOS arm64|Linux x64|workspace version|GitHub Releases|make install|SENTRY_DSN|draft" \
  docs/superpowers/specs/2026-06-11-spacetop-versioning-deployment-policy-design.md \
  docs/release-policy.md \
  README.md \
  .github/workflows/release.yml
```

Expected: every design concept appears in at least one implementation file.

- [ ] **Step 2: Confirm changed files**

Run:

```bash
git log --oneline --max-count=8
```

Expected: recent commits include:

```text
feat: expose spacetop version
docs: add release policy
docs: document release install path
ci: add pull request checks
ci: add GitHub release workflow
```

- [ ] **Step 3: Prepare final implementation summary**

Use this format:

```markdown
Implemented versioning and deployment policy.

Changed:
- Added `spacetop --version`.
- Added CI for format, tests, and lint.
- Added tag-triggered GitHub Release workflow for macOS arm64 and Linux x64.
- Added changelog and release policy docs.
- Updated README install guidance.

Verification:
- `cargo fmt --check`
- `cargo test`
- `make lint`
- `SENTRY_DSN= cargo build --release -p spacetop`
- `target/release/spacetop --version`

Notes:
- GitHub Release publishing is draft-first.
- `make install` remains local-only.
- Homebrew/crates.io/Windows are intentionally deferred.
```

## Self-Review Notes

- Spec coverage: CLI version, GitHub Release deployment, macOS arm64 and Linux x64 assets, local-only `make install`, CI, Sentry behavior, failure policy, release notes, and README update are all mapped to tasks.
- Placeholder scan: no incomplete sections or deferred implementation blanks are intentionally left for the worker.
- Type consistency: the plan uses existing `Cli`, Clap `CommandFactory`, root workspace version, and package name `spacetop` consistently.
