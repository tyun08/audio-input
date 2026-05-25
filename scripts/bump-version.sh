#!/usr/bin/env bash
# Bump the version in all three places that must stay in sync:
#   - package.json
#   - src-tauri/Cargo.toml
#   - src-tauri/tauri.conf.json
# Then refresh Cargo.lock so the workspace builds cleanly.
#
# Usage: scripts/bump-version.sh 0.4.12
#
# Does NOT commit, tag, or push — release flow stays in RELEASING.md.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <new-version>   e.g. $0 0.4.12" >&2
  exit 2
fi

NEW="$1"

if [[ ! "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: version must be MAJOR.MINOR.PATCH (got: $NEW)" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PKG="package.json"
CARGO="src-tauri/Cargo.toml"
TAURI="src-tauri/tauri.conf.json"

for f in "$PKG" "$CARGO" "$TAURI"; do
  [[ -f "$f" ]] || { echo "error: $f not found" >&2; exit 1; }
done

# Extract current versions (same patterns release.yml uses for verification).
CUR_PKG=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$PKG" | head -1)
CUR_CARGO=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$CARGO" | head -1)
CUR_TAURI=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TAURI" | head -1)

echo "current: package.json=$CUR_PKG  Cargo.toml=$CUR_CARGO  tauri.conf.json=$CUR_TAURI"
echo "target:  $NEW"

# Use perl for portable in-place edits (BSD sed on macOS vs GNU sed differ).
# `unless $done++` limits the substitution to the first match only — both JSON
# files have many "version" keys (dependencies etc); only the top-level one
# should change.
perl -i -pe 's/"version"\s*:\s*"[^"]+"/"version": "'"$NEW"'"/ unless $done++' "$PKG"
perl -i -pe 's/"version"\s*:\s*"[^"]+"/"version": "'"$NEW"'"/ unless $done++' "$TAURI"
perl -i -pe 's/^version\s*=\s*"[^"]+"/version = "'"$NEW"'"/ unless $done++' "$CARGO"

# Refresh Cargo.lock — workspace package version is recorded there too.
( cd src-tauri && cargo build --quiet )

echo
echo "bumped to $NEW. files changed:"
git --no-pager diff --stat "$PKG" "$CARGO" "src-tauri/Cargo.lock" "$TAURI"
