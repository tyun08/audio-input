// Wire protocol mirror of src-tauri/src/input/imk/protocol.rs.
// MUST stay in lockstep with the Rust side — the snake_case JSON tags are
// the contract. Add a roundtrip test on both sides whenever you change a
// variant or field.

import Foundation

public struct ImkRange: Codable, Equatable {
    public let location: UInt32
    public let length: UInt32
    public init(location: UInt32, length: UInt32) {
        self.location = location
        self.length = length
    }
}

public struct ImkRect: Codable, Equatable {
    public let x: Double
    public let y: Double
    public let w: Double
    public let h: Double
    public init(x: Double, y: Double, w: Double, h: Double) {
        self.x = x; self.y = y; self.w = w; self.h = h
    }
}

/// Requests the Tauri side sends to the helper. Decoded server-side.
public enum ImkRequest: Equatable {
    case insert(text: String, replacementRange: ImkRange?)
    case getContext
    case ping
}

extension ImkRequest: Codable {
    // Manual Codable because `enum with associated values` + `serde tag =
    // "type"` don't line up via the synthesized derivation.
    private enum CodingKeys: String, CodingKey {
        case type, text, replacement_range
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        switch type {
        case "insert":
            let text = try c.decode(String.self, forKey: .text)
            let range = try c.decodeIfPresent(ImkRange.self, forKey: .replacement_range)
            self = .insert(text: text, replacementRange: range)
        case "get_context":
            self = .getContext
        case "ping":
            self = .ping
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: c,
                debugDescription: "unknown request type '\(type)'"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .insert(text, range):
            try c.encode("insert", forKey: .type)
            try c.encode(text, forKey: .text)
            if let range { try c.encode(range, forKey: .replacement_range) }
        case .getContext:
            try c.encode("get_context", forKey: .type)
        case .ping:
            try c.encode("ping", forKey: .type)
        }
    }
}

/// Responses the helper sends back. Encoded server-side.
public enum ImkResponse: Equatable {
    case ok
    case context(
        beforeCursor: String,
        selectedText: String,
        cursorRect: ImkRect,
        appBundleId: String,
        appName: String
    )
    case error(message: String)
}

extension ImkResponse: Codable {
    private enum CodingKeys: String, CodingKey {
        case type, message
        case before_cursor, selected_text, cursor_rect, app_bundle_id, app_name
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        switch type {
        case "ok":
            self = .ok
        case "context":
            self = .context(
                beforeCursor: try c.decode(String.self, forKey: .before_cursor),
                selectedText: try c.decode(String.self, forKey: .selected_text),
                cursorRect: try c.decode(ImkRect.self, forKey: .cursor_rect),
                appBundleId: try c.decode(String.self, forKey: .app_bundle_id),
                appName: try c.decode(String.self, forKey: .app_name)
            )
        case "error":
            self = .error(message: try c.decode(String.self, forKey: .message))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: c,
                debugDescription: "unknown response type '\(type)'"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .ok:
            try c.encode("ok", forKey: .type)
        case let .context(beforeCursor, selectedText, cursorRect, appBundleId, appName):
            try c.encode("context", forKey: .type)
            try c.encode(beforeCursor, forKey: .before_cursor)
            try c.encode(selectedText, forKey: .selected_text)
            try c.encode(cursorRect, forKey: .cursor_rect)
            try c.encode(appBundleId, forKey: .app_bundle_id)
            try c.encode(appName, forKey: .app_name)
        case let .error(message):
            try c.encode("error", forKey: .type)
            try c.encode(message, forKey: .message)
        }
    }
}
