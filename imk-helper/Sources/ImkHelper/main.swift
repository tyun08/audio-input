// Phase 1 entry point. Boots the socket server with the phase-1 stub
// handler so it can be exercised by `cargo test` (Rust client) or
// driven manually with `nc -U /tmp/audio-input-imk.sock`.
//
// Phase 2 swaps Handler.handlePhase1 for an IMKInputController-backed
// dispatcher and registers the IMKServer with IMKit. Until then this
// binary is purely a socket echo + protocol validator — useful for
// proving the wire path, useless for actually inserting text.

import Foundation
import ImkHelperCore

let socketPath = ProcessInfo.processInfo.environment["IMK_HELPER_SOCKET"]
    ?? "/tmp/audio-input-imk.sock"

let server = SocketServer(path: socketPath, handler: Handler.handlePhase1)

do {
    try server.start()
    FileHandle.standardError.write(Data(
        "imk-helper listening on \(socketPath) (phase-1 stub handler)\n".utf8
    ))
} catch {
    FileHandle.standardError.write(Data(
        "imk-helper failed to start: \(error)\n".utf8
    ))
    exit(1)
}

// Keep the process alive. RunLoop.main works under both a UI session
// (Phase 2, when IMKServer needs the run loop) and a plain CLI.
RunLoop.main.run()
