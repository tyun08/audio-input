// Request handler used by the socket server.
//
// Phase 1 (this file): pure protocol echo — every request gets a static
// response so the wire path is testable without an IMKInputController
// being active. The real implementation lives in Phase 2:
//
//   .insert     → IMKInputController.client().insertText(...) on main queue
//   .getContext → read selectedRange + attributedSubstring + cursorRect
//                 from the active IMKTextInput client; map to ImkResponse.context
//   .ping       → unchanged, always .ok
//
// Tests drive this directly; the executable wires it into SocketServer.

import Foundation

public enum Handler {
    /// Phase 1 dispatcher. Returns a deterministic response purely from
    /// the request shape — does NOT touch any AppKit/IMKit APIs.
    /// Tests rely on this so they can run without a UI session.
    public static func handlePhase1(_ req: ImkRequest) -> ImkResponse {
        switch req {
        case .insert:
            // Real Phase 2 will dispatch to IMKInputController.
            // Phase 1 acknowledges so the Rust client + socket round-trip
            // can be exercised end-to-end via swift test / cargo test.
            return .ok
        case .getContext:
            // Phase 1 returns an empty-but-valid context. Lets the Rust
            // side validate response parsing without an active editor.
            return .context(
                beforeCursor: "",
                selectedText: "",
                cursorRect: ImkRect(x: 0, y: 0, w: 0, h: 0),
                appBundleId: "",
                appName: ""
            )
        case .ping:
            return .ok
        }
    }
}
