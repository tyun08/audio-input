#!/usr/bin/env bash
# End-to-end smoke test: real Swift helper binary + raw netcat client.
# Run from repo root: ./imk-helper/Tests/e2e_smoke.sh
#
# Validates the full wire path without depending on the Rust client,
# so it's useful even on a fresh clone with only Swift installed.
# `swift test` (or `swift run ImkHelperTests`) covers the Swift side
# in isolation; this script proves the binary actually listens and
# answers correctly when driven externally.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT/imk-helper"

SOCK="/tmp/imk-e2e-$$.sock"
trap 'kill ${HELPER_PID:-} 2>/dev/null || true; rm -f "$SOCK"' EXIT

echo "==> swift build (debug)"
swift build >/dev/null

echo "==> launching helper on $SOCK"
IMK_HELPER_SOCKET="$SOCK" .build/debug/imk-helper >/dev/null 2>&1 &
HELPER_PID=$!

# Wait up to 2s for the socket to appear.
for _ in $(seq 1 20); do
  [ -S "$SOCK" ] && break
  sleep 0.1
done
[ -S "$SOCK" ] || { echo "FAIL: helper never created socket"; exit 1; }

# Each subtest: send a JSON line, expect the supplied JSON line back.
expect_reply() {
  local desc="$1" request="$2" expected="$3"
  local actual
  actual=$(echo "$request" | nc -U "$SOCK" | head -1)
  if [ "$actual" = "$expected" ]; then
    echo "  ✓ $desc"
  else
    echo "  ✗ $desc"
    echo "    sent:     $request"
    echo "    expected: $expected"
    echo "    got:      $actual"
    return 1
  fi
}

echo ""
echo "==> wire-format checks"
expect_reply "ping → ok" \
  '{"type":"ping"}' \
  '{"type":"ok"}'
expect_reply "insert → ok" \
  '{"type":"insert","text":"hello"}' \
  '{"type":"ok"}'
expect_reply "insert with replacement_range → ok" \
  '{"type":"insert","text":"x","replacement_range":{"location":4,"length":6}}' \
  '{"type":"ok"}'
# Malformed input must produce an error reply, but the exact message
# wording comes from Foundation and varies across macOS versions. Just
# check the type field.
actual=$(echo 'not json' | nc -U "$SOCK" | head -1)
echo "$actual" | python3 -c '
import json, sys
d = json.load(sys.stdin)
assert d["type"] == "error", d
assert "decode" in d["message"].lower() or "json" in d["message"].lower(), d
print("  ✓ malformed JSON → error response")
'
echo ""
echo "==> get_context returns valid context shape"
actual=$(echo '{"type":"get_context"}' | nc -U "$SOCK" | head -1)
echo "$actual" | python3 -c '
import json, sys
d = json.load(sys.stdin)
assert d["type"] == "context", d
assert "before_cursor" in d
assert "selected_text" in d
assert "cursor_rect" in d
assert "app_bundle_id" in d
assert "app_name" in d
print("  ✓ get_context shape ok")
'

echo ""
echo "ALL CHECKS PASSED"
