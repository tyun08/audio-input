use tracing::debug;

/// RMS energy threshold below which audio is considered silent.
/// 0.005 ≈ -46 dBFS — above the typical microphone floor
/// (~0.001–0.003 RMS) while catching quieter speech that the
/// previous 0.01 threshold was incorrectly rejecting.
pub const SILENCE_RMS_THRESHOLD: f32 = 0.005;

/// Returns `true` when `samples` contain only silence or near-silence
/// (microphone floor noise, room tone) and no meaningful speech energy.
///
/// The decision is based on the Root-Mean-Square (RMS) energy of the
/// entire clip:
/// * RMS < [`SILENCE_RMS_THRESHOLD`] → silent → **skip** the API call.
/// * RMS ≥ [`SILENCE_RMS_THRESHOLD`] → speech present → proceed normally.
///
/// An empty slice is always treated as silent.
pub fn is_silent(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    debug!("Audio RMS energy: {:.6}", rms);
    rms < SILENCE_RMS_THRESHOLD
}

// ---------------------------------------------------------------------------
// Tests — see src-tauri/tests/silence_detection.rs
// ---------------------------------------------------------------------------
