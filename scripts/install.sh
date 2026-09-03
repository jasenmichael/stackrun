#!/bin/sh
# Install stackrun from GitHub Releases. Served at:
#   https://jasenmichael.github.io/stackrun/install.sh
#
#   curl -fsSL https://jasenmichael.github.io/stackrun/install.sh | sh
#   curl -fsSL ... | STACKRUN_VERSION=1.0.0 sh
#   curl -fsSL ... | STACKRUN_INSTALL=/usr/local/bin sh
set -eu

REPO="${STACKRUN_REPO:-jasenmichael/stackrun}"
GITHUB="${GITHUB:-https://github.com}"
INSTALL_DIR="${STACKRUN_INSTALL:-${HOME}/.local/bin}"
BIN_NAME=stackrun
DRY_RUN=0

usage() {
  cat <<'EOF'
Install stackrun from GitHub Releases.

Usage: install.sh [--dry-run] [--help]

Environment:
  STACKRUN_VERSION   Version without leading v (default: latest release)
  STACKRUN_INSTALL   Install directory (default: ~/.local/bin)
  STACKRUN_TARGET    Rust target triple override
  STACKRUN_REPO      owner/name (default: jasenmichael/stackrun)
EOF
}

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

die() {
  echo "stackrun-install: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "need $1 on PATH"
}

detect_target() {
  if [ -n "${STACKRUN_TARGET:-}" ]; then
    echo "$STACKRUN_TARGET"
    return
  fi

  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Linux)
      libc=gnu
      if [ -f /etc/alpine-release ]; then
        libc=musl
      elif command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
        libc=musl
      fi
      case "$arch" in
        x86_64|amd64)
          echo "x86_64-unknown-linux-${libc}"
          ;;
        aarch64|arm64)
          if [ "$libc" = musl ]; then
            die "no published linux-arm64-musl binary; use linux-arm64-gnu or build from source"
          fi
          echo "aarch64-unknown-linux-gnu"
          ;;
        *)
          die "unsupported Linux arch: $arch"
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64) echo "x86_64-apple-darwin" ;;
        arm64) echo "aarch64-apple-darwin" ;;
        *) die "unsupported macOS arch: $arch" ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*)
      die "Windows: download the zip from ${GITHUB}/${REPO}/releases (x86_64-pc-windows-msvc)"
      ;;
    *)
      die "unsupported OS: $os. See ${GITHUB}/${REPO}/releases"
      ;;
  esac
}

resolve_version() {
  if [ -n "${STACKRUN_VERSION:-}" ]; then
    echo "${STACKRUN_VERSION#v}"
    return
  fi
  need_cmd curl
  url=$(curl -fsSL -o /dev/null -w '%{url_effective}' "${GITHUB}/${REPO}/releases/latest")
  tag=${url##*/}
  [ -n "$tag" ] && [ "$tag" != "latest" ] || die "could not resolve latest release from ${GITHUB}/${REPO}/releases/latest"
  echo "${tag#v}"
}

verify_checksum() {
  file=$1
  sums=$2
  line=$(grep -E "[ /]$(printf '%s' "$file" | sed 's/[.[*^$]/\\&/g')\$" "$sums" | tail -n 1) \
    || die "no SHA256SUMS entry for $file"
  hash=${line%% *}
  [ -n "$hash" ] || die "bad SHA256SUMS line for $file"
  if command -v sha256sum >/dev/null 2>&1; then
    echo "$hash  $file" | sha256sum -c -
  elif command -v shasum >/dev/null 2>&1; then
    echo "$hash  $file" | shasum -a 256 -c -
  else
    die "need sha256sum or shasum"
  fi
}

target=$(detect_target)
version=$(resolve_version)
archive="stackrun-v${version}-${target}.tar.gz"
asset_url="${GITHUB}/${REPO}/releases/download/v${version}/${archive}"
sums_url="${GITHUB}/${REPO}/releases/download/v${version}/SHA256SUMS"

echo "stackrun ${version} (${target})"
echo "from ${asset_url}"
echo "into ${INSTALL_DIR}/${BIN_NAME}"

if [ "$DRY_RUN" -eq 1 ]; then
  echo "dry-run: skip download"
  exit 0
fi

need_cmd curl
need_cmd tar

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl -fsSL -o "${tmp}/${archive}" "$asset_url" || die "download failed: $asset_url"
curl -fsSL -o "${tmp}/SHA256SUMS" "$sums_url" || die "download failed: $sums_url"

(
  cd "$tmp"
  verify_checksum "$archive" SHA256SUMS
  tar -xzf "$archive"
)

extracted=$(find "$tmp" -type f -name "$BIN_NAME" | head -n 1)
[ -n "$extracted" ] || die "archive had no ${BIN_NAME}"

mkdir -p "$INSTALL_DIR"
cp "$extracted" "${INSTALL_DIR}/${BIN_NAME}"
chmod +x "${INSTALL_DIR}/${BIN_NAME}"

echo "installed ${INSTALL_DIR}/${BIN_NAME}"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "add ${INSTALL_DIR} to PATH" ;;
esac
"${INSTALL_DIR}/${BIN_NAME}" --version
