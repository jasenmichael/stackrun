#!/usr/bin/env bash
# Copy a built native binary into an npm platform package directory.
# Usage: scripts/package-npm.sh <target> <binary-path>
set -euo pipefail

target="${1:?target triple}"
binary="${2:?path to stackrun binary}"
root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"

case "$target" in
  x86_64-unknown-linux-gnu) pkg=linux-x64-gnu ;;
  aarch64-unknown-linux-gnu) pkg=linux-arm64-gnu ;;
  x86_64-unknown-linux-musl) pkg=linux-x64-musl ;;
  x86_64-apple-darwin) pkg=darwin-x64 ;;
  aarch64-apple-darwin) pkg=darwin-arm64 ;;
  x86_64-pc-windows-msvc) pkg=win32-x64-msvc ;;
  *)
    echo "unknown target: $target" >&2
    exit 1
    ;;
esac

dest="$root/npm/platforms/$pkg"
mkdir -p "$dest"
name="stackrun"
if [[ "$pkg" == win32-* ]]; then
  name="stackrun.exe"
fi
cp "$binary" "$dest/$name"
chmod +x "$dest/$name"

# Keep package.json version in sync with Cargo.toml
if [[ -f "$dest/package.json" ]]; then
  node -e "
    const fs = require('fs');
    const p = '$dest/package.json';
    const j = JSON.parse(fs.readFileSync(p, 'utf8'));
    j.version = '$version';
    fs.writeFileSync(p, JSON.stringify(j, null, 2) + '\n');
  "
fi
