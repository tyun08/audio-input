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

/// Holds the screenshot captured at recording start for use in the polish /
/// smart compose step.
#[derive(Debug, Default)]
pub struct ScreenshotContext {
    image_data_url: Option<String>,
    capturing: bool,
    generation: u64,
}

impl ScreenshotContext {
    pub fn begin_capture(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.image_data_url = None;
        self.capturing = true;
        self.generation
    }

    pub fn finish_capture(&mut self, generation: u64, image_data_url: Option<String>) -> bool {
        if self.generation != generation || !self.capturing {
            return false;
        }
        self.image_data_url = image_data_url;
        self.capturing = false;
        true
    }

    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.image_data_url = None;
        self.capturing = false;
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing
    }

    pub fn take_ready(&mut self) -> Option<String> {
        if self.capturing {
            None
        } else {
            self.image_data_url.take()
        }
    }

    pub fn cancel_capture(&mut self) -> Option<String> {
        self.generation = self.generation.wrapping_add(1);
        self.capturing = false;
        self.image_data_url.take()
    }
}

pub type ScreenshotState = Arc<Mutex<ScreenshotContext>>;

pub fn new_screenshot_state() -> ScreenshotState {
    Arc::new(Mutex::new(ScreenshotContext::default()))
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

#[cfg(test)]
mod tests {
    use super::ScreenshotContext;

    #[test]
    fn screenshot_context_ignores_stale_capture() {
        let mut ctx = ScreenshotContext::default();
        let first = ctx.begin_capture();
        let _second = ctx.begin_capture();

        assert!(!ctx.finish_capture(first, Some("stale".to_string())));
        assert!(ctx.is_capturing());
    }

    #[test]
    fn screenshot_context_takes_only_ready_capture() {
        let mut ctx = ScreenshotContext::default();
        let generation = ctx.begin_capture();

        assert_eq!(ctx.take_ready(), None);
        assert!(ctx.finish_capture(generation, Some("current".to_string())));
        assert_eq!(ctx.take_ready(), Some("current".to_string()));
        assert_eq!(ctx.take_ready(), None);
    }
}
