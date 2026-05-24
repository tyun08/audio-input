// End-to-end Socket + Handler tests. Starts a real Unix socket server
// on a unique temp path, connects with a plain socket client, exchanges
// a newline-delimited JSON request, verifies the response.

import Foundation
import Darwin
import ImkHelperCore

private func uniqueSocketPath() -> String {
    let dir = NSTemporaryDirectory()
    let id = UUID().uuidString.prefix(8)
    return "\(dir)imk-\(id).sock"
}

/// Connect → write request + \n → half-close write → read until \n → decode.
/// Synchronous, used from a single test thread.
private func sendRequest(to path: String, _ payload: String) throws -> ImkResponse {
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    if fd < 0 { throw TestError.failed("socket() failed: errno=\(errno)") }
    defer { close(fd) }

    var addr = sockaddr_un()
    addr.sun_family = sa_family_t(AF_UNIX)
    let pathBytes = path.utf8CString
    // Capture the field's byte count into a local so Swift's exclusivity
    // checker doesn't see overlapping accesses (reading the size while
    // also writing the bytes).
    let pathCap = MemoryLayout.size(ofValue: addr.sun_path)
    withUnsafeMutablePointer(to: &addr.sun_path) { p in
        p.withMemoryRebound(to: CChar.self, capacity: pathCap) { dst in
            pathBytes.withUnsafeBufferPointer { src in
                _ = memcpy(dst, src.baseAddress!, pathBytes.count)
            }
        }
    }
    let addrSize = socklen_t(MemoryLayout<sockaddr_un>.size)
    let r = withUnsafePointer(to: &addr) {
        $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
            Darwin.connect(fd, sa, addrSize)
        }
    }
    if r != 0 { throw TestError.failed("connect() failed: errno=\(errno)") }

    var toSend = payload
    if !toSend.hasSuffix("\n") { toSend.append("\n") }
    let bytes = Array(toSend.utf8)
    _ = bytes.withUnsafeBufferPointer { write(fd, $0.baseAddress, $0.count) }
    shutdown(fd, SHUT_WR)

    var buf = [UInt8](repeating: 0, count: 4096)
    var collected = [UInt8]()
    let deadline = Date().addingTimeInterval(2.0)
    while Date() < deadline {
        let n = read(fd, &buf, buf.count)
        if n > 0 {
            collected.append(contentsOf: buf[0..<n])
            if collected.contains(UInt8(ascii: "\n")) { break }
        } else if n == 0 {
            break
        } else if errno != EAGAIN && errno != EWOULDBLOCK {
            throw TestError.failed("read() failed: errno=\(errno)")
        }
    }
    let line: ArraySlice<UInt8>
    if let idx = collected.firstIndex(of: UInt8(ascii: "\n")) {
        line = collected[0..<idx]
    } else {
        line = collected[...]
    }
    let s = String(bytes: line, encoding: .utf8) ?? ""
    return try JSONDecoder().decode(ImkResponse.self, from: Data(s.utf8))
}

let socketTests: [TestCase] = [
    TestCase(name: "phase-1 handler echoes ok for insert") {
        let path = uniqueSocketPath()
        let server = SocketServer(path: path, handler: Handler.handlePhase1)
        try server.start()
        defer { server.stop() }
        let resp = try sendRequest(to: path, #"{"type":"insert","text":"hello"}"#)
        try assertEq(resp, .ok)
    },
    TestCase(name: "phase-1 handler echoes ok for ping") {
        let path = uniqueSocketPath()
        let server = SocketServer(path: path, handler: Handler.handlePhase1)
        try server.start()
        defer { server.stop() }
        try assertEq(try sendRequest(to: path, #"{"type":"ping"}"#), .ok)
    },
    TestCase(name: "phase-1 handler returns empty context for get_context") {
        let path = uniqueSocketPath()
        let server = SocketServer(path: path, handler: Handler.handlePhase1)
        try server.start()
        defer { server.stop() }
        let resp = try sendRequest(to: path, #"{"type":"get_context"}"#)
        if case let .context(beforeCursor, _, _, _, appName) = resp {
            try assertEq(beforeCursor, "")
            try assertEq(appName, "")
        } else {
            throw TestError.failed("expected .context, got \(resp)")
        }
    },
    TestCase(name: "malformed request gets error response") {
        let path = uniqueSocketPath()
        let server = SocketServer(path: path, handler: Handler.handlePhase1)
        try server.start()
        defer { server.stop() }
        let resp = try sendRequest(to: path, "this is not json")
        if case let .error(message) = resp {
            try assertTrue(message.contains("decode failed"), "got: \(message)")
        } else {
            throw TestError.failed("expected .error, got \(resp)")
        }
    },
    TestCase(name: "custom handler receives parsed request") {
        let path = uniqueSocketPath()
        // NSMutableArray for thread-safe capture across handler thread.
        let captured = NSMutableArray()
        let server = SocketServer(path: path) { req in
            captured.add("\(req)")
            return .ok
        }
        try server.start()
        defer { server.stop() }
        _ = try sendRequest(to: path, #"{"type":"insert","text":"captured"}"#)
        Thread.sleep(forTimeInterval: 0.05)
        try assertEq(captured.count, 1)
        let saw = captured[0] as! String
        try assertTrue(saw.contains("captured"), "handler did not parse text: \(saw)")
    },
    TestCase(name: "restart on same path unlinks stale socket") {
        // Simulates recovery from a previous instance that crashed
        // without cleaning up its socket file.
        let path = uniqueSocketPath()
        let first = SocketServer(path: path, handler: Handler.handlePhase1)
        try first.start()
        first.stop()
        // First server is gone; file might or might not be cleaned up.
        // Touch the path to simulate a stale leftover.
        FileManager.default.createFile(atPath: path, contents: Data(), attributes: nil)
        let second = SocketServer(path: path, handler: Handler.handlePhase1)
        try second.start()
        second.stop()
    },
    TestCase(name: "stop is idempotent") {
        let path = uniqueSocketPath()
        let server = SocketServer(path: path, handler: Handler.handlePhase1)
        try server.start()
        server.stop()
        server.stop()
    },
]
