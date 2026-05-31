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

# JSON files: slurp the whole file (`-0777`) and replace the FIRST `"version":
# "..."` occurrence. Slurp mode is needed because the previous line-by-line
# approach using `unless $done++` was buggy — `$done` was incremented on every
# line including non-matches, so the substitution was skipped before it could
# ever fire. Slurp + bare s/// (no /g) reliably hits the first match.
#
# Positional caveat: this assumes the top-level `version` is the file's first
# `"version"` occurrence. True today for package.json and tauri.conf.json. A
# nested `"version"` introduced ABOVE the top-level field (uncommon but
# possible in tauri.conf.json plugin blocks) would be hit instead. JSON-aware
# parsing would be sturdier — tracked as a fast-follow.
perl -i -0777 -pe 's/"version"\s*:\s*"[^"]+"/"version": "'"$NEW"'"/' "$PKG"
perl -i -0777 -pe 's/"version"\s*:\s*"[^"]+"/"version": "'"$NEW"'"/' "$TAURI"

# Cargo.toml: anchor on the [package] section so we never hit a dependency's
# version pin. Match the `version = "..."` line that lives between the
# `[package]` header and the next `[section]` header.
perl -i -0777 -pe '
  s/(\[package\][^\[]*?\nversion\s*=\s*")[^"]+(")/${1}'"$NEW"'${2}/s
' "$CARGO"

# Refresh Cargo.lock — the workspace package version is recorded there too.
# We edit it directly (same perl approach as the other files) rather than
# running `cargo update --offline`, which still requires the crates.io registry
# index and fails on a cold CI cache.
perl -i -0777 -pe '
  s/(name = "audio-input"\nversion = ")[^"]+(")/\1'"$NEW"'\2/
' src-tauri/Cargo.lock

# Sanity-check: confirm every file now reports the target version. Cheap
# protection against future regressions in the edit logic above (the previous
# version of this script silently produced no edits at all).
POST_PKG=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$PKG" | head -1)
POST_CARGO=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$CARGO" | head -1)
POST_TAURI=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TAURI" | head -1)
POST_LOCK=$(awk '/name = "audio-input"/ { getline; print; exit }' src-tauri/Cargo.lock \
  | sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p')

fail=0
[ "$POST_PKG"   = "$NEW" ] || { echo "::error::package.json post-bump = $POST_PKG (expected $NEW)" >&2; fail=1; }
[ "$POST_CARGO" = "$NEW" ] || { echo "::error::Cargo.toml post-bump = $POST_CARGO (expected $NEW)" >&2; fail=1; }
[ "$POST_TAURI" = "$NEW" ] || { echo "::error::tauri.conf.json post-bump = $POST_TAURI (expected $NEW)" >&2; fail=1; }
[ "$POST_LOCK"  = "$NEW" ] || { echo "::error::Cargo.lock audio-input post-bump = $POST_LOCK (expected $NEW)" >&2; fail=1; }
if [ "$fail" -eq 1 ]; then
  echo "::error::version bump did not land in all files — see diff above and check the edit logic" >&2
  exit 1
fi

echo
echo "bumped to $NEW. files changed:"
git --no-pager diff --stat "$PKG" "$CARGO" "src-tauri/Cargo.lock" "$TAURI"
