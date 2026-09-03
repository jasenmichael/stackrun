#!/usr/bin/env bash
# Build the GitHub Pages site (docs branch) from README.md + install.sh + demo assets.
# Usage: scripts/generate-pages.sh [version] [outdir]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="${1:-}"
if [[ -z "$version" ]]; then
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
fi
version="${version#v}"
outdir="${2:-$root/site}"

mkdir -p "$outdir"
python3 "$root/scripts/pages/render_readme.py" \
  "$root/README.md" \
  "$outdir/index.html" \
  "$version"
cp "$root/scripts/install.sh" "$outdir/install.sh"
chmod +x "$outdir/install.sh"
cp "$root/README.md" "$outdir/README.md"
if [[ -f "$root/scripts/pages/demo.cast" ]]; then
  cp "$root/scripts/pages/demo.cast" "$outdir/demo.cast"
fi
if [[ -f "$root/scripts/pages/demo.svg" ]]; then
  cp "$root/scripts/pages/demo.svg" "$outdir/demo.svg"
fi
touch "$outdir/.nojekyll"

echo "pages: v${version} -> ${outdir}"
