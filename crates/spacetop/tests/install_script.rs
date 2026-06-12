use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

const README: &str = "README.md";
const RELEASE_POLICY: &str = "docs/release-policy.md";

#[test]
fn darwin_arm64_selects_macos_archive_and_installs_to_temp_dir() {
    let harness = InstallerHarness::new();
    harness.fake_uname("Darwin", "arm64");
    harness.fake_curl();
    harness.fake_sha256sum_success();
    harness.fake_tar();
    harness.fake_install();

    let output = harness.run();

    assert_success(&output);
    assert_log_contains(
        &harness,
        "curl.log",
        "https://github.com/spacedock-dev/spacetop/releases/latest/download/SHA256SUMS\n",
    );
    assert_log_contains(
        &harness,
        "curl.log",
        "https://github.com/spacedock-dev/spacetop/releases/latest/download/spacetop-v0.1.0-aarch64-apple-darwin.tar.gz\n",
    );
    assert!(
        harness.install_dir.join("spacetop").is_file(),
        "installer should place spacetop in SPACETOP_INSTALL_DIR"
    );
    assert_log_contains(&harness, "version.log", "--version\n");
    assert_temp_cleaned(&harness);
}

#[test]
fn linux_x86_64_selects_linux_archive() {
    let harness = InstallerHarness::new();
    harness.fake_uname("Linux", "x86_64");
    harness.fake_curl();
    harness.fake_sha256sum_success();
    harness.fake_tar();
    harness.fake_install();

    let output = harness.run();

    assert_success(&output);
    assert_log_contains(
        &harness,
        "curl.log",
        "https://github.com/spacedock-dev/spacetop/releases/latest/download/spacetop-v0.1.0-x86_64-unknown-linux-gnu.tar.gz\n",
    );
}

#[test]
fn unsupported_platform_exits_with_detected_pair() {
    let harness = InstallerHarness::new();
    harness.fake_uname("Darwin", "x86_64");
    harness.fake_curl();
    harness.fake_sha256sum_success();
    harness.fake_tar();
    harness.fake_install();

    let output = harness.run();

    assert_failure(&output);
    assert_stderr_contains(&output, "unsupported platform: Darwin x86_64");
    assert_log_absent(&harness, "curl.log");
    assert_log_absent(&harness, "install.log");
}

#[test]
fn checksum_mismatch_exits_before_extract_or_install_and_cleans_temp_dir() {
    let harness = InstallerHarness::new();
    harness.fake_uname("Linux", "x86_64");
    harness.fake_curl();
    harness.fake_sha256sum_failure();
    harness.fake_tar();
    harness.fake_install();

    let output = harness.run();

    assert_failure(&output);
    assert_stderr_contains(&output, "checksum verification failed");
    assert_log_absent(&harness, "tar.log");
    assert_log_absent(&harness, "install.log");
    assert_temp_cleaned(&harness);
}

#[test]
fn missing_sha256sum_falls_back_to_shasum() {
    let harness = InstallerHarness::new();
    harness.fake_uname("Darwin", "arm64");
    harness.fake_curl();
    harness.fake_shasum_success();
    harness.fake_tar();
    harness.fake_install();

    let output = harness.run();

    assert_success(&output);
    assert_log_contains(&harness, "shasum.log", "-a 256 -c selected.SHA256SUMS\n");
}

#[test]
fn missing_checksum_tool_exits_before_extracting() {
    let harness = InstallerHarness::new();
    harness.fake_uname("Linux", "x86_64");
    harness.fake_curl();
    harness.fake_tar();
    harness.fake_install();

    let output = harness.run();

    assert_failure(&output);
    assert_stderr_contains(&output, "missing checksum tool");
    assert_log_absent(&harness, "tar.log");
    assert_log_absent(&harness, "install.log");
}

#[test]
fn readme_documents_one_command_installer_and_install_dir_override() {
    let readme = read_text(README);

    assert!(
        readme.contains(
            "curl -fsSL https://raw.githubusercontent.com/spacedock-dev/spacetop/main/install.sh | sh"
        ),
        "README should provide one copy-paste curl install command"
    );
    assert!(
        readme.contains("SPACETOP_INSTALL_DIR"),
        "README should document install directory override"
    );
    assert!(
        readme.contains("macOS Apple Silicon") && readme.contains("Linux x64"),
        "README should name supported binary installer platforms"
    );
}

#[test]
fn release_policy_documents_installer_asset_contract() {
    let policy = read_text(RELEASE_POLICY);

    assert!(
        policy.contains("curl installer"),
        "release policy should mention the curl installer"
    );
    assert!(
        policy.contains("spacetop-vX.Y.Z-aarch64-apple-darwin.tar.gz")
            && policy.contains("spacetop-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz")
            && policy.contains("SHA256SUMS"),
        "release policy should keep the installer asset contract explicit"
    );
}

struct InstallerHarness {
    _tmp: TempDir,
    bin_dir: PathBuf,
    logs_dir: PathBuf,
    install_dir: PathBuf,
    fake_temp_dir: PathBuf,
}

impl InstallerHarness {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bin_dir = tmp.path().join("bin");
        let logs_dir = tmp.path().join("logs");
        let install_dir = tmp.path().join("install");
        let fake_temp_dir = tmp.path().join("installer-tmp");
        fs::create_dir_all(&bin_dir).expect("create fake bin");
        fs::create_dir_all(&logs_dir).expect("create logs dir");
        fs::create_dir_all(&install_dir).expect("create install dir");

        let harness = Self {
            _tmp: tmp,
            bin_dir,
            logs_dir,
            install_dir,
            fake_temp_dir,
        };
        harness.fake_coreutils();
        harness.fake_mktemp();
        harness
    }

    fn run(&self) -> Output {
        Command::new("/bin/sh")
            .arg(workspace_root().join("install.sh"))
            .env_clear()
            .env("PATH", &self.bin_dir)
            .env("HOME", self._tmp.path().join("home"))
            .env("SPACETOP_INSTALL_DIR", &self.install_dir)
            .env("FAKE_LOGS_DIR", &self.logs_dir)
            .env("FAKE_TEMP_DIR", &self.fake_temp_dir)
            .output()
            .expect("run installer")
    }

    fn fake_uname(&self, os: &str, arch: &str) {
        self.write_executable(
            "uname",
            &format!(
                r#"#!/bin/sh
case "$1" in
  -s) printf '%s\n' '{os}' ;;
  -m) printf '%s\n' '{arch}' ;;
  *) exit 2 ;;
esac
"#
            ),
        );
    }

    fn fake_curl(&self) {
        self.write_executable(
            "curl",
            r#"#!/bin/sh
url=
out=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done
printf '%s\n' "$url" >> "$FAKE_LOGS_DIR/curl.log"
case "$url" in
  */SHA256SUMS)
    {
      printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  spacetop-v0.1.0-aarch64-apple-darwin.tar.gz\n'
      printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  spacetop-v0.1.0-x86_64-unknown-linux-gnu.tar.gz\n'
    } > "$out"
    ;;
  *.tar.gz)
    printf 'archive\n' > "$out"
    ;;
  *)
    exit 22
    ;;
esac
"#,
        );
    }

    fn fake_sha256sum_success(&self) {
        self.write_executable(
            "sha256sum",
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOGS_DIR/sha256sum.log"
exit 0
"#,
        );
    }

    fn fake_sha256sum_failure(&self) {
        self.write_executable(
            "sha256sum",
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOGS_DIR/sha256sum.log"
exit 1
"#,
        );
    }

    fn fake_shasum_success(&self) {
        self.write_executable(
            "shasum",
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOGS_DIR/shasum.log"
exit 0
"#,
        );
    }

    fn fake_tar(&self) {
        self.write_executable(
            "tar",
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOGS_DIR/tar.log"
archive=
dest=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -xzf) archive="$2"; shift 2 ;;
    -C) dest="$2"; shift 2 ;;
    *) shift ;;
  esac
done
name="${archive##*/}"
package="${name%.tar.gz}"
/bin/mkdir -p "$dest/$package"
cat > "$dest/$package/spacetop" <<'SCRIPT'
#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOGS_DIR/version.log"
printf 'spacetop 0.1.0\n'
SCRIPT
/bin/chmod 755 "$dest/$package/spacetop"
"#,
        );
    }

    fn fake_install(&self) {
        self.write_executable(
            "install",
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOGS_DIR/install.log"
if [ "$1" = "-m" ]; then
  mode="$2"
  src="$3"
  dest="$4"
else
  mode="755"
  src="$1"
  dest="$2"
fi
/bin/cp "$src" "$dest"
/bin/chmod "$mode" "$dest"
"#,
        );
    }

    fn fake_mktemp(&self) {
        self.write_executable(
            "mktemp",
            r#"#!/bin/sh
/bin/rm -rf "$FAKE_TEMP_DIR"
/bin/mkdir -p "$FAKE_TEMP_DIR"
printf '%s\n' "$FAKE_TEMP_DIR"
"#,
        );
    }

    fn fake_coreutils(&self) {
        for command in ["mkdir", "rm", "cp", "chmod", "cat"] {
            self.write_executable(
                command,
                &format!(
                    r#"#!/bin/sh
exec /bin/{command} "$@"
"#
                ),
            );
        }
    }

    fn write_executable(&self, name: &str, content: &str) {
        let path = self.bin_dir.join(name);
        fs::write(&path, content).unwrap_or_else(|error| {
            panic!("failed to write fake command {}: {error}", path.display());
        });
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("set fake command executable");
    }

    fn log_path(&self, name: &str) -> PathBuf {
        self.logs_dir.join(name)
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "expected failure\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_stderr_contains(output: &Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "stderr should contain {expected:?}, got:\n{stderr}"
    );
}

fn assert_log_contains(harness: &InstallerHarness, name: &str, expected: &str) {
    let path = harness.log_path(name);
    let log = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("failed to read log {}: {error}", path.display());
    });
    assert!(
        log.contains(expected),
        "log {} should contain {expected:?}, got:\n{log}",
        path.display()
    );
}

fn assert_log_absent(harness: &InstallerHarness, name: &str) {
    let path = harness.log_path(name);
    assert!(!path.exists(), "log should not exist: {}", path.display());
}

fn assert_temp_cleaned(harness: &InstallerHarness) {
    assert!(
        !harness.fake_temp_dir.exists(),
        "installer temp dir should be removed: {}",
        harness.fake_temp_dir.display()
    );
}

fn read_text(path: &str) -> String {
    fs::read_to_string(workspace_root().join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate should live two levels below workspace root")
        .to_path_buf()
}
