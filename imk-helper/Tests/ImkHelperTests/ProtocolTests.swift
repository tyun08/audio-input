// Codec tests — must agree with src-tauri/src/input/imk/protocol.rs.
// If you tweak a field name or variant on one side, update both files
// AND both test suites.

import Foundation
import ImkHelperCore

private func encode<T: Encodable>(_ v: T) throws -> String {
    let data = try JSONEncoder().encode(v)
    return String(data: data, encoding: .utf8)!
}
private func decode<T: Decodable>(_ s: String, as _: T.Type) throws -> T {
    try JSONDecoder().decode(T.self, from: Data(s.utf8))
}

let protocolTests: [TestCase] = [
    TestCase(name: "insert without range omits field") {
        let json = try encode(ImkRequest.insert(text: "hello", replacementRange: nil))
        try assertFalse(json.contains("replacement_range"), "got: \(json)")
        try assertTrue(json.contains("\"type\":\"insert\""))
        try assertTrue(json.contains("\"text\":\"hello\""))
    },
    TestCase(name: "insert with range includes field") {
        let req = ImkRequest.insert(
            text: "rewrite",
            replacementRange: ImkRange(location: 10, length: 5)
        )
        let json = try encode(req)
        try assertTrue(json.contains("\"location\":10"), "got: \(json)")
        try assertTrue(json.contains("\"length\":5"), "got: \(json)")
    },
    TestCase(name: "get_context serializes with just type") {
        let json = try encode(ImkRequest.getContext)
        try assertEq(json, #"{"type":"get_context"}"#)
    },
    TestCase(name: "ping serializes") {
        let json = try encode(ImkRequest.ping)
        try assertEq(json, #"{"type":"ping"}"#)
    },
    TestCase(name: "ok response roundtrips") {
        let r = ImkResponse.ok
        let back = try decode(encode(r), as: ImkResponse.self)
        try assertEq(r, back)
    },
    TestCase(name: "context response roundtrips") {
        let r = ImkResponse.context(
            beforeCursor: "Hi John, regarding our meeting...",
            selectedText: "",
            cursorRect: ImkRect(x: 320.0, y: 480.0, w: 2.0, h: 18.0),
            appBundleId: "com.apple.mail",
            appName: "Mail"
        )
        try assertEq(try decode(encode(r), as: ImkResponse.self), r)
    },
    TestCase(name: "error response carries message") {
        let r = ImkResponse.error(message: "no active client")
        let json = try encode(r)
        try assertEq(json, #"{"type":"error","message":"no active client"}"#)
        try assertEq(try decode(json, as: ImkResponse.self), r)
    },
    TestCase(name: "all request variants roundtrip") {
        let reqs: [ImkRequest] = [
            .insert(text: "abc", replacementRange: nil),
            .insert(text: "xyz", replacementRange: ImkRange(location: 0, length: 3)),
            .getContext,
            .ping,
        ]
        for r in reqs {
            try assertEq(try decode(encode(r), as: ImkRequest.self), r)
        }
    },
    TestCase(name: "unknown request type fails cleanly") {
        try assertThrows(try decode(#"{"type":"future_thing"}"#, as: ImkRequest.self))
    },
    TestCase(name: "unknown response type fails cleanly") {
        try assertThrows(try decode(#"{"type":"future_thing"}"#, as: ImkResponse.self))
    },
    TestCase(name: "wire compatibility with rust client payloads") {
        // Exact JSON the Rust client (verified by its own roundtrip tests)
        // sends. If either side breaks the format, this test catches it
        // before integration testing does.
        let r1 = try decode(#"{"type":"insert","text":"hello"}"#, as: ImkRequest.self)
        try assertEq(r1, .insert(text: "hello", replacementRange: nil))

        let r2 = try decode(
            #"{"type":"insert","text":"x","replacement_range":{"location":4,"length":6}}"#,
            as: ImkRequest.self
        )
        try assertEq(r2, .insert(text: "x", replacementRange: ImkRange(location: 4, length: 6)))

        let r3 = try decode(#"{"type":"ok"}"#, as: ImkResponse.self)
        try assertEq(r3, .ok)
    },
]
