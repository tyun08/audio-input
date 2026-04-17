use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    Recording,
    Processing,
    Error(String),
}

impl std::fmt::Display for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppState::Idle => write!(f, "idle"),
            AppState::Recording => write!(f, "recording"),
            AppState::Processing => write!(f, "processing"),
            AppState::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(Mutex::new(AppState::Idle))
}

/// Holds the screenshot captured at recording start for use in the polish step.
pub type ScreenshotState = Arc<Mutex<Option<String>>>;

pub fn new_screenshot_state() -> ScreenshotState {
    Arc::new(Mutex::new(None))
}

/// Tracks the wall-clock instant when the current recording session started.
pub type RecordingStartTime = Arc<Mutex<Option<std::time::Instant>>>;

pub fn new_recording_start_time() -> RecordingStartTime {
    Arc::new(Mutex::new(None))
}
