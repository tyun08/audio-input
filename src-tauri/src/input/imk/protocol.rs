//! Wire protocol for talking to the IMK helper.
//!
//! Newline-delimited JSON over Unix socket. Both sides MUST keep the
//! Swift `Protocol.swift` in `imk-helper/` in lockstep with this file —
//! the snake_case tags are normative.

use serde::{Deserialize, Serialize};

/// NSRange-equivalent: location + length, both in UTF-16 code units (what
/// macOS text APIs work in). 0-based location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub location: u32,
    pub length: u32,
}

/// Screen-coordinate rectangle for the cursor. Origin is bottom-left
/// (Cocoa convention) — front-end converts to top-left as needed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Messages the Tauri side sends to the IMK helper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Insert `text` at the current insertion point. If
    /// `replacement_range` is `Some`, replace that range first (used for
    /// the selected-text-transform workflow).
    Insert {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replacement_range: Option<Range>,
    },

    /// Read the current cursor context: text before cursor, selected
    /// text, cursor rect, active app. Used to feed context into the
    /// polish prompt.
    GetContext,

    /// Liveness probe — server replies `Ok`. Used by reconnect logic.
    Ping,
}

/// Messages the IMK helper sends back to the Tauri side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// Operation completed (no payload).
    Ok,

    /// Reply to `GetContext`.
    Context {
        before_cursor: String,
        selected_text: String,
        cursor_rect: Rect,
        app_bundle_id: String,
        app_name: String,
    },

    /// Operation failed. `message` is human-readable, not for parsing.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(v: T) {
        let s = serde_json::to_string(&v).expect("serialize");
        let back: T = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(v, back, "roundtrip mismatch via {s}");
    }

    #[test]
    fn insert_without_range_omits_field() {
        let req = Request::Insert {
            text: "hello".into(),
            replacement_range: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        // Critical: when no range, the field must be ABSENT (not null) so
        // Swift's Codable doesn't choke and the wire is compact.
        assert!(!s.contains("replacement_range"), "got: {s}");
        assert!(s.contains(r#""type":"insert""#));
        assert!(s.contains(r#""text":"hello""#));
    }

    #[test]
    fn insert_with_range_includes_field() {
        let req = Request::Insert {
            text: "rewrite".into(),
            replacement_range: Some(Range { location: 10, length: 5 }),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains(r#""replacement_range":{"location":10,"length":5}"#), "got: {s}");
    }

    #[test]
    fn get_context_serializes_with_just_type() {
        let s = serde_json::to_string(&Request::GetContext).unwrap();
        assert_eq!(s, r#"{"type":"get_context"}"#);
    }

    #[test]
    fn ping_serializes() {
        let s = serde_json::to_string(&Request::Ping).unwrap();
        assert_eq!(s, r#"{"type":"ping"}"#);
    }

    #[test]
    fn ok_response_roundtrips() {
        roundtrip(Response::Ok);
    }

    #[test]
    fn error_response_carries_message() {
        let r = Response::Error { message: "no active client".into() };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"type":"error","message":"no active client"}"#);
        roundtrip(r);
    }

    #[test]
    fn context_response_roundtrips() {
        let r = Response::Context {
            before_cursor: "Hi John, regarding our meeting...".into(),
            selected_text: "".into(),
            cursor_rect: Rect { x: 320.0, y: 480.0, w: 2.0, h: 18.0 },
            app_bundle_id: "com.apple.mail".into(),
            app_name: "Mail".into(),
        };
        roundtrip(r);
    }

    #[test]
    fn all_request_variants_roundtrip() {
        roundtrip(Request::Insert {
            text: "abc".into(),
            replacement_range: None,
        });
        roundtrip(Request::Insert {
            text: "xyz".into(),
            replacement_range: Some(Range { location: 0, length: 3 }),
        });
        roundtrip(Request::GetContext);
        roundtrip(Request::Ping);
    }

    #[test]
    fn unknown_response_type_fails_with_useful_error() {
        // Defensive: if the Swift side sends a future variant we don't
        // know about, we want a clear deserialize failure, not a panic
        // or a silent miss.
        let r: Result<Response, _> = serde_json::from_str(r#"{"type":"future_thing"}"#);
        assert!(r.is_err(), "expected error, got {r:?}");
    }
}
