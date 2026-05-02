//! Recording history store.
//!
//! Persists every transcription attempt — audio + metadata — so the user can
//! retry after a provider failure without re-speaking, and so we retain a
//! local corpus for future fine-tuning / prompt iteration.
//!
//! Layout inside `app_data_dir/history/`:
//!   - `history.json` — ordered index of entries (oldest first)
//!   - `<id>.wav`     — the encoded 16 kHz mono WAV for the entry
//!
//! The store is capped at `max_entries` (configurable). On insert past the
//! cap, the oldest entries are evicted and their WAV files deleted.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

pub const DEFAULT_MAX_HISTORY: usize = 100;

/// Status of a recorded session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    /// Transcription has not finished yet (initial or during retry).
    Pending,
    /// Transcription succeeded.
    Completed,
    /// Transcription failed — retry is available.
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at_ms: u64,
    pub duration_s: f32,
    pub provider: String,
    #[serde(default)]
    pub raw_text: String,
    #[serde(default)]
    pub polished_text: String,
    pub status: HistoryStatus,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub polish_failed: bool,
}

pub struct HistoryStore {
    dir: PathBuf,
    entries: Vec<HistoryEntry>,
    max_entries: usize,
}

pub type HistoryState = Arc<Mutex<HistoryStore>>;

impl HistoryStore {
    pub fn load(dir: PathBuf, max_entries: usize) -> Self {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            warn!("Failed to create history dir {:?}: {}", dir, e);
        }
        let index = dir.join("history.json");
        let entries = std::fs::read_to_string(&index)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<HistoryEntry>>(&s).ok())
            .unwrap_or_default();
        info!(
            "History store loaded: {} entries from {:?}",
            entries.len(),
            index
        );
        let mut store = HistoryStore {
            dir,
            entries,
            max_entries: max_entries.max(1),
        };
        // In case previous runs left orphans or overflow, normalize.
        let _ = store.prune();
        store
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn set_max_entries(&mut self, max: usize) -> Result<()> {
        self.max_entries = max.max(1);
        self.prune()?;
        self.save_index()
    }

    #[allow(dead_code)]
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn get(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn wav_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.wav", id))
    }

    /// Insert a new pending entry and persist the WAV file.
    pub fn insert_pending(
        &mut self,
        provider: String,
        duration_s: f32,
        wav: &[u8],
    ) -> Result<String> {
        let id = generate_id(&self.entries);
        let created_at_ms = now_ms();

        std::fs::write(self.wav_path(&id), wav)
            .with_context(|| format!("Failed to persist audio for history entry {}", id))?;

        self.entries.push(HistoryEntry {
            id: id.clone(),
            created_at_ms,
            duration_s,
            provider,
            raw_text: String::new(),
            polished_text: String::new(),
            status: HistoryStatus::Pending,
            error: None,
            polish_failed: false,
        });

        self.prune()?;
        self.save_index()?;
        Ok(id)
    }

    /// Mark entry completed with the resulting text.
    pub fn mark_completed(
        &mut self,
        id: &str,
        raw_text: String,
        polished_text: String,
        polish_failed: bool,
    ) -> Result<()> {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.raw_text = raw_text;
            entry.polished_text = polished_text;
            entry.polish_failed = polish_failed;
            entry.status = HistoryStatus::Completed;
            entry.error = None;
        }
        self.save_index()
    }

    /// Mark entry failed with an error message.
    pub fn mark_failed(&mut self, id: &str, error: String) -> Result<()> {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.status = HistoryStatus::Failed;
            entry.error = Some(error);
        }
        self.save_index()
    }

    /// Flip status back to Pending (for a retry in progress).
    pub fn mark_pending(&mut self, id: &str) -> Result<()> {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.status = HistoryStatus::Pending;
            entry.error = None;
        }
        self.save_index()
    }

    pub fn load_wav(&self, id: &str) -> Result<Vec<u8>> {
        let path = self.wav_path(id);
        std::fs::read(&path).with_context(|| format!("Failed to read recording at {:?}", path))
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        let path = self.wav_path(id);
        let _ = std::fs::remove_file(&path);
        self.entries.retain(|e| e.id != id);
        self.save_index()
    }

    fn save_index(&self) -> Result<()> {
        let path = self.dir.join("history.json");
        let data = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&path, data)
            .with_context(|| format!("Failed to write history index {:?}", path))
    }

    fn prune(&mut self) -> Result<()> {
        while self.entries.len() > self.max_entries {
            let dropped = self.entries.remove(0);
            let _ = std::fs::remove_file(self.wav_path(&dropped.id));
        }
        // Drop orphan WAV files not referenced by the index (best-effort).
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            let known: std::collections::HashSet<String> = self
                .entries
                .iter()
                .map(|e| format!("{}.wav", e.id))
                .collect();
            for entry in rd.flatten() {
                let p: PathBuf = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("wav") {
                    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                        if !known.contains(name) {
                            let _ = std::fs::remove_file(&p);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn new_history_state(dir: PathBuf, max_entries: usize) -> HistoryState {
    Arc::new(Mutex::new(HistoryStore::load(dir, max_entries)))
}

pub fn history_dir(app_data: &Path) -> PathBuf {
    app_data.join("history")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Millisecond timestamp as an id, with a small counter suffix if the last
/// entry was created within the same millisecond (rare but possible during
/// rapid retries).
fn generate_id(entries: &[HistoryEntry]) -> String {
    let ts = now_ms();
    let mut id = format!("rec_{}", ts);
    let mut counter = 1u32;
    while entries.iter().any(|e| e.id == id) {
        id = format!("rec_{}_{}", ts, counter);
        counter += 1;
    }
    id
}
