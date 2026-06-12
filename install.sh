#!/bin/sh
set -eu

repo="${SPACETOP_REPO:-spacedock-dev/spacetop}"
release_base="https://github.com/${repo}/releases/latest/download"
temp_dir=""

die() {
  printf 'spacetop install: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  if [ -n "${temp_dir}" ] && [ -d "${temp_dir}" ]; then
    rm -rf "${temp_dir}"
  fi
}

trap cleanup EXIT INT HUP TERM

need_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    die "missing required command: $1"
  fi
}

resolve_install_dir() {
  if [ "${SPACETOP_INSTALL_DIR+x}" ]; then
    install_dir="${SPACETOP_INSTALL_DIR}"
  else
    if [ -z "${HOME:-}" ]; then
      die "SPACETOP_INSTALL_DIR is unset and HOME is empty"
    fi
    install_dir="${HOME}/.cargo/bin"
  fi

  case "${install_dir}" in
    /*) ;;
    *) die "install directory must be absolute: ${install_dir}" ;;
  esac

  printf '%s\n' "${install_dir}"
}

resolve_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}:${arch}" in
    Darwin:arm64 | Darwin:aarch64)
      printf '%s\n' "aarch64-apple-darwin"
      ;;
    Linux:x86_64 | Linux:amd64)
      printf '%s\n' "x86_64-unknown-linux-gnu"
      ;;
    *)
      die "unsupported platform: ${os} ${arch}"
      ;;
  esac
}

resolve_checksum_tool() {
  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' "sha256sum"
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    printf '%s\n' "shasum"
    return
  fi

  die "missing checksum tool: install sha256sum or shasum"
}

select_archive_from_checksums() {
  target="$1"
  checksums="$2"
  archive_name=""
  selected_line=""

  while read -r checksum filename _rest; do
    if [ -z "${checksum:-}" ] || [ -z "${filename:-}" ]; then
      continue
    fi
    filename="${filename#\*}"
    case "${filename}" in
      spacetop-v*-"${target}".tar.gz)
        archive_name="${filename}"
        selected_line="${checksum}  ${filename}"
        break
        ;;
    esac
  done < "${checksums}"

  if [ -z "${archive_name}" ]; then
    die "SHA256SUMS has no archive for target ${target}"
  fi

  printf '%s\n' "${selected_line}" > "${temp_dir}/selected.SHA256SUMS"
  printf '%s\n' "${archive_name}"
}

verify_checksum() {
  checksum_tool="$1"
  archive_name="$2"

  if (
    cd "${temp_dir}"
    case "${checksum_tool}" in
      sha256sum) sha256sum -c selected.SHA256SUMS ;;
      shasum) shasum -a 256 -c selected.SHA256SUMS ;;
      *) exit 1 ;;
    esac
  ); then
    return
  fi

  die "checksum verification failed for ${archive_name}"
}

install_binary() {
  source_binary="$1"
  target_binary="$2"
  install_dir="$3"

  mkdir -p "${install_dir}"

  if command -v install >/dev/null 2>&1; then
    install -m 755 "${source_binary}" "${target_binary}"
  else
    cp "${source_binary}" "${target_binary}"
    chmod 755 "${target_binary}"
  fi
}

need_command curl
need_command tar
need_command mktemp

install_dir="$(resolve_install_dir)"
target="$(resolve_target)"
checksum_tool="$(resolve_checksum_tool)"

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/spacetop-install.XXXXXX")"
checksums="${temp_dir}/SHA256SUMS"

curl -fsSL "${release_base}/SHA256SUMS" -o "${checksums}"
archive_name="$(select_archive_from_checksums "${target}" "${checksums}")"
archive_path="${temp_dir}/${archive_name}"
curl -fsSL "${release_base}/${archive_name}" -o "${archive_path}"

verify_checksum "${checksum_tool}" "${archive_name}"

tar -xzf "${archive_path}" -C "${temp_dir}"
package_dir="${temp_dir}/${archive_name%.tar.gz}"
source_binary="${package_dir}/spacetop"
if [ ! -f "${source_binary}" ]; then
  die "archive did not contain expected binary: ${archive_name}"
fi

target_binary="${install_dir}/spacetop"
install_binary "${source_binary}" "${target_binary}" "${install_dir}"
"${target_binary}" --version >/dev/null

printf 'spacetop installed to %s\n' "${target_binary}"
