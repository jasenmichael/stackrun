#!/usr/bin/env bash
# Build the GitHub Pages site (docs branch) from templates + install.sh.
# Usage: scripts/generate-pages.sh [version] [outdir]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="${1:-}"
if [[ -z "$version" ]]; then
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
fi
version="${version#v}"
outdir="${2:-$root/site}"
repo="jasenmichael/stackrun"

mkdir -p "$outdir"
sed -e "s|{{VERSION}}|${version}|g" -e "s|{{REPO}}|${repo}|g" \
  "$root/scripts/pages/index.html" > "$outdir/index.html"
cp "$root/scripts/install.sh" "$outdir/install.sh"
chmod +x "$outdir/install.sh"
cp "$root/README.md" "$outdir/README.md"
touch "$outdir/.nojekyll"

echo "pages: v${version} -> ${outdir}"
