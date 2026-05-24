# imk-helper

InputMethodKit helper for Audio Input. Phase 1 of the integration plan in [`docs/IMK_PLAN.md`](../docs/IMK_PLAN.md).

## Status

**Phase 1 (this commit): socket plumbing only.** The helper binds a Unix domain socket, parses the wire protocol, and replies with stub responses. It does NOT yet:

- Register as a macOS input source (no `Info.plist` + `.app` wrapper yet)
- Call `IMKInputController.client().insertText(...)` (no AppKit/IMKit code yet)
- Read cursor context from the active text input client

Those are Phase 2 of `docs/IMK_PLAN.md`. The Phase 1 surface exists so the Rust client (`src-tauri/src/input/imk/`) has a stable contract to integrate against.

## Layout

```
imk-helper/
├── Package.swift
├── Sources/
│   ├── ImkHelperCore/         ← library: protocol + socket server + handler
│   │   ├── Protocol.swift     ← Codable mirror of Rust's protocol.rs
│   │   ├── SocketServer.swift ← Unix domain socket accept/read/write loop
│   │   └── Handler.swift      ← request → response dispatcher (Phase 1: stub)
│   └── ImkHelper/             ← executable: boots SocketServer with Phase 1 handler
│       └── main.swift
└── Tests/
    ├── ImkHelperTests/        ← executable test runner (no XCTest dep)
    │   ├── TestRunner.swift   ← assert helpers + runAll()
    │   ├── ProtocolTests.swift
    │   ├── SocketServerTests.swift
    │   └── main.swift
    └── e2e_smoke.sh           ← live helper + nc client wire-format check
```

Tests don't use `XCTest` because Apple's Command Line Tools (no Xcode) don't ship it. Each test file populates a `[TestCase]` array; `main.swift` collects them all and calls `runAll()`, which exits non-zero on any failure.

## Run

```bash
# Build everything (helper binary + tests)
cd imk-helper
swift build

# Unit + integration tests (Swift side only)
swift run ImkHelperTests

# Launch the helper for manual poking
.build/debug/imk-helper            # binds /tmp/audio-input-imk.sock
IMK_HELPER_SOCKET=/tmp/my.sock .build/debug/imk-helper

# Then in another terminal:
echo '{"type":"ping"}' | nc -U /tmp/audio-input-imk.sock
# → {"type":"ok"}

# Full wire-format smoke test (builds + launches + exercises + tears down)
./Tests/e2e_smoke.sh
```

## Protocol

Newline-delimited JSON over the Unix socket. One request per connection, one response, then the helper closes the socket. See `Sources/ImkHelperCore/Protocol.swift` + the matching Rust types in `src-tauri/src/input/imk/protocol.rs`.

Both sides have roundtrip tests for every variant. If you change a field name or add a variant, update BOTH and the cross-compatibility test in each suite.

## Phase 2 sketch (not yet implemented)

1. Add `Info.plist` + `build-app.sh` so the binary becomes a real `Audio Input.app` registered under `~/Library/Input Methods/`
2. Add `AppInputController: IMKInputController` subclass; on `.insert` dispatch `client().insertText(...)` on the main queue
3. On `.getContext`, read `selectedRange`, `attributedSubstringFromRange:`, `firstRectForCharacterRange:` from the active `IMKTextInput` client
4. Swap `Handler.handlePhase1` for a Phase 2 dispatcher in `main.swift`
5. Wire `src-tauri/src/input/injector.rs`'s `inject_text` to try `ImkClient` first, fall back to the clipboard path on `ImkError::Unavailable`
