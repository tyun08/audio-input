//! Async client for the IMK helper socket.
//!
//! One short-lived connection per request — simpler than connection
//! pooling, and the helper is local so connect cost is negligible (~µs).
//! Each connection writes one newline-delimited JSON request and reads
//! one newline-delimited JSON response, then closes.
//!
//! Per-request timeout via `tokio::time::timeout`: if the helper is wedged
//! or the socket doesn't exist (helper not installed / not running), we
//! return `Err(ImkError::Unavailable)` quickly rather than hanging the
//! caller. Callers (e.g. `inject_text`) treat that as "fall back to the
//! clipboard path".

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::timeout;

use super::protocol::{Range, Request, Response};

/// How long to wait on connect + send + recv. Anything beyond ~200 ms
/// means the helper is unresponsive — better to fall back than block UI.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug, Error)]
pub enum ImkError {
    /// Socket file is missing or connection refused — helper isn't
    /// running. Caller should fall back to the clipboard path.
    #[error("IMK helper is not available at {path}: {source}")]
    Unavailable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Helper responded with an explicit error.
    #[error("IMK helper error: {0}")]
    Server(String),

    /// We got a response, but not the one we expected for the request.
    /// Indicates a protocol-level bug, not a transient failure.
    #[error("IMK helper returned unexpected response: {0:?}")]
    UnexpectedResponse(Response),

    /// Socket I/O failed mid-request.
    #[error("IMK helper IO error: {0}")]
    Io(#[from] io::Error),

    /// Couldn't serialize the request or deserialize the response.
    #[error("IMK helper JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// `tokio::time::timeout` fired.
    #[error("IMK helper timed out after {0:?}")]
    Timeout(Duration),
}

/// Tauri-side client for the IMK helper.
pub struct ImkClient {
    socket_path: PathBuf,
    timeout: Duration,
}

impl ImkClient {
    /// New client pointing at the default `/tmp/audio-input-imk.sock`.
    pub fn new() -> Self {
        Self::with_path(PathBuf::from(super::SOCKET_PATH))
    }

    /// New client pointing at a custom socket path. Mostly for tests.
    pub fn with_path(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Override the per-request timeout (default 200 ms). Useful for
    /// integration tests with slower mock servers.
    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// Insert text at the current insertion point. If `replacement_range`
    /// is `Some`, that range is replaced (used for selected-text transforms).
    pub async fn insert(
        &self,
        text: impl Into<String>,
        replacement_range: Option<Range>,
    ) -> Result<(), ImkError> {
        let req = Request::Insert {
            text: text.into(),
            replacement_range,
        };
        match self.send(req).await? {
            Response::Ok => Ok(()),
            Response::Error { message } => Err(ImkError::Server(message)),
            other => Err(ImkError::UnexpectedResponse(other)),
        }
    }

    /// Liveness probe — succeeds iff the helper is up and responsive.
    /// Useful for an "Is IMK ready?" startup check.
    pub async fn ping(&self) -> Result<(), ImkError> {
        match self.send(Request::Ping).await? {
            Response::Ok => Ok(()),
            Response::Error { message } => Err(ImkError::Server(message)),
            other => Err(ImkError::UnexpectedResponse(other)),
        }
    }

    /// Fetch the active cursor context for context-aware polishing.
    pub async fn get_context(&self) -> Result<Response, ImkError> {
        match self.send(Request::GetContext).await? {
            ctx @ Response::Context { .. } => Ok(ctx),
            Response::Error { message } => Err(ImkError::Server(message)),
            other => Err(ImkError::UnexpectedResponse(other)),
        }
    }

    /// Low-level: one round trip with the configured timeout.
    async fn send(&self, req: Request) -> Result<Response, ImkError> {
        let path = self.socket_path.clone();
        let to = self.timeout;
        let fut = async move {
            let stream = UnixStream::connect(&path)
                .await
                .map_err(|e| ImkError::Unavailable { path, source: e })?;
            let (read_half, mut write_half) = stream.into_split();

            let mut payload = serde_json::to_string(&req)?;
            payload.push('\n');
            write_half.write_all(payload.as_bytes()).await?;
            // Best-effort shutdown of the write side so the server sees
            // EOF on its read side and replies promptly. Ignore the
            // result — closing twice is harmless.
            let _ = write_half.shutdown().await;

            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line.is_empty() {
                return Err(ImkError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "IMK helper closed the socket without responding",
                )));
            }
            let resp: Response = serde_json::from_str(line.trim_end())?;
            Ok::<Response, ImkError>(resp)
        };

        match timeout(to, fut).await {
            Ok(res) => res,
            Err(_) => Err(ImkError::Timeout(to)),
        }
    }
}

impl Default for ImkClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, target_family = "unix"))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::UnixListener;
    use tokio::sync::Notify;

    /// Spin up a one-shot mock IMK helper on a unique socket path. The
    /// `handler` decides what to send back for each received line. Caller
    /// gets back the path + a Notify it can wait on for server shutdown.
    async fn spawn_mock<F>(handler: F) -> (PathBuf, Arc<Notify>)
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("imk-test.sock");
        let listener = UnixListener::bind(&path).expect("bind");
        let stop = Arc::new(Notify::new());
        let stop2 = stop.clone();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            // Keep `dir` alive for the lifetime of the server so the
            // socket file isn't deleted out from under us mid-test.
            let _dir_guard = dir;
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { continue };
                        let h = handler.clone();
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = stream.into_split();
                            let mut reader = BufReader::new(read_half);
                            let mut line = String::new();
                            let _ = reader.read_line(&mut line).await;
                            let mut reply = h(line.trim_end());
                            reply.push('\n');
                            let _ = write_half.write_all(reply.as_bytes()).await;
                            let _ = write_half.shutdown().await;
                        });
                    }
                    _ = stop2.notified() => break,
                }
            }
        });

        // Give the listener a moment to be ready before the test connects.
        tokio::time::sleep(Duration::from_millis(20)).await;
        (path, stop)
    }

    #[tokio::test]
    async fn insert_happy_path() {
        let (path, stop) = spawn_mock(|line| {
            // Sanity-check what the client sent us, then reply OK.
            assert!(line.contains(r#""type":"insert""#), "got: {line}");
            assert!(line.contains(r#""text":"hello world""#), "got: {line}");
            r#"{"type":"ok"}"#.to_string()
        })
        .await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_secs(2));
        client.insert("hello world", None).await.expect("insert ok");
        stop.notify_one();
    }

    #[tokio::test]
    async fn insert_with_replacement_range() {
        let (path, stop) = spawn_mock(|line| {
            assert!(
                line.contains(r#""replacement_range":{"location":4,"length":6}"#),
                "got: {line}"
            );
            r#"{"type":"ok"}"#.to_string()
        })
        .await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_secs(2));
        client
            .insert(
                "replacement",
                Some(Range {
                    location: 4,
                    length: 6,
                }),
            )
            .await
            .expect("insert ok");
        stop.notify_one();
    }

    #[tokio::test]
    async fn server_error_surfaces_as_server_variant() {
        let (path, stop) = spawn_mock(|_| {
            r#"{"type":"error","message":"no active client"}"#.to_string()
        })
        .await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_secs(2));
        let err = client.insert("x", None).await.unwrap_err();
        match err {
            ImkError::Server(msg) => assert_eq!(msg, "no active client"),
            other => panic!("expected Server error, got {other:?}"),
        }
        stop.notify_one();
    }

    #[tokio::test]
    async fn missing_socket_is_unavailable() {
        // Path that doesn't exist — should fail fast with Unavailable.
        let client = ImkClient::with_path("/tmp/audio-input-imk-doesnotexist.sock".into())
            .with_timeout(Duration::from_millis(500));
        let err = client.insert("x", None).await.unwrap_err();
        assert!(
            matches!(err, ImkError::Unavailable { .. }),
            "expected Unavailable, got {err:?}"
        );
    }

    #[tokio::test]
    async fn get_context_returns_full_payload() {
        let (path, stop) = spawn_mock(|_| {
            r#"{"type":"context","before_cursor":"Hi John","selected_text":"","cursor_rect":{"x":1.0,"y":2.0,"w":3.0,"h":4.0},"app_bundle_id":"com.apple.mail","app_name":"Mail"}"#.to_string()
        })
        .await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_secs(2));
        let resp = client.get_context().await.expect("context");
        match resp {
            Response::Context {
                before_cursor,
                app_name,
                ..
            } => {
                assert_eq!(before_cursor, "Hi John");
                assert_eq!(app_name, "Mail");
            }
            other => panic!("expected Context, got {other:?}"),
        }
        stop.notify_one();
    }

    #[tokio::test]
    async fn unexpected_response_for_insert_is_caught() {
        let (path, stop) = spawn_mock(|_| {
            // Server replies with a Context to our Insert — protocol bug,
            // should not be silently accepted.
            r#"{"type":"context","before_cursor":"","selected_text":"","cursor_rect":{"x":0,"y":0,"w":0,"h":0},"app_bundle_id":"x","app_name":"x"}"#.to_string()
        })
        .await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_secs(2));
        let err = client.insert("x", None).await.unwrap_err();
        assert!(
            matches!(err, ImkError::UnexpectedResponse(_)),
            "got: {err:?}"
        );
        stop.notify_one();
    }

    #[tokio::test]
    async fn slow_server_triggers_timeout() {
        // Server accepts but never replies — must time out without
        // blocking the test runner indefinitely.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slow.sock");
        let listener = UnixListener::bind(&path).unwrap();
        tokio::spawn(async move {
            let _dir_guard = dir;
            if let Ok((stream, _)) = listener.accept().await {
                // Hold the stream open and sleep beyond the client timeout.
                tokio::time::sleep(Duration::from_secs(5)).await;
                drop(stream);
            }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_millis(80));
        let err = client.ping().await.unwrap_err();
        assert!(matches!(err, ImkError::Timeout(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn ping_succeeds_against_well_behaved_server() {
        let (path, stop) = spawn_mock(|line| {
            assert!(line.contains(r#""type":"ping""#));
            r#"{"type":"ok"}"#.to_string()
        })
        .await;
        let client = ImkClient::with_path(path).with_timeout(Duration::from_secs(2));
        client.ping().await.expect("ping ok");
        stop.notify_one();
    }
}
