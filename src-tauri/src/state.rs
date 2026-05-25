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

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionMode {
    #[default]
    Dictate,
    SmartCompose,
}

impl std::fmt::Display for TranscriptionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranscriptionMode::Dictate => write!(f, "dictate"),
            TranscriptionMode::SmartCompose => write!(f, "smart_compose"),
        }
    }
}

impl TranscriptionMode {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Dictate => Self::SmartCompose,
            Self::SmartCompose => Self::Dictate,
        }
    }
}

pub type SharedMode = Arc<Mutex<TranscriptionMode>>;

pub fn new_shared_mode(mode: TranscriptionMode) -> SharedMode {
    Arc::new(Mutex::new(mode))
}
