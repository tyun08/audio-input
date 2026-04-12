use tracing::debug;

/// RMS energy threshold below which audio is considered silent.
/// 0.01 ≈ -40 dBFS — comfortably above the typical microphone floor
/// (~0.001–0.003 RMS) and well below the quietest real speech
/// (~0.02–0.05 RMS), so it reliably skips silence without
/// ever discarding genuine utterances.
const SILENCE_RMS_THRESHOLD: f32 = 0.01;

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
    let rms =
        (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    debug!("Audio RMS energy: {:.6}", rms);
    rms < SILENCE_RMS_THRESHOLD
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // --- silence cases -------------------------------------------------------

    #[test]
    fn empty_samples_are_silent() {
        assert!(is_silent(&[]));
    }

    #[test]
    fn all_zeros_are_silent() {
        let samples = vec![0.0f32; 16_000]; // 1 s @ 16 kHz
        assert!(is_silent(&samples));
    }

    #[test]
    fn microphone_floor_noise_is_silent() {
        // Typical microphone background noise: amplitude ~0.001
        // RMS of a sine with amplitude A = A / sqrt(2) ≈ 0.000707
        let samples: Vec<f32> = (0..16_000)
            .map(|i| 0.001 * (2.0 * PI * 100.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!(is_silent(&samples));
    }

    #[test]
    fn rms_just_below_threshold_is_silent() {
        // Sine with amplitude chosen so RMS = 0.9 * threshold
        // RMS = A / sqrt(2)  →  A = RMS * sqrt(2)
        let target_rms = SILENCE_RMS_THRESHOLD * 0.9;
        let a = target_rms * 2.0_f32.sqrt();
        let samples: Vec<f32> = (0..16_000)
            .map(|i| a * (2.0 * PI * 440.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!(is_silent(&samples));
    }

    // --- speech cases --------------------------------------------------------

    #[test]
    fn normal_speech_level_is_not_silent() {
        // Typical speech amplitude: ~0.1–0.2; RMS of a sine ≈ 0.106
        let samples: Vec<f32> = (0..16_000)
            .map(|i| 0.15 * (2.0 * PI * 440.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!(!is_silent(&samples));
    }

    #[test]
    fn quiet_whisper_is_not_silent() {
        // Quiet whisper: amplitude ~0.03; RMS ≈ 0.021
        let samples: Vec<f32> = (0..16_000)
            .map(|i| 0.03 * (2.0 * PI * 300.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!(!is_silent(&samples));
    }

    #[test]
    fn rms_just_above_threshold_is_not_silent() {
        // Sine with amplitude chosen so RMS = 1.1 * threshold
        let target_rms = SILENCE_RMS_THRESHOLD * 1.1;
        let a = target_rms * 2.0_f32.sqrt();
        let samples: Vec<f32> = (0..16_000)
            .map(|i| a * (2.0 * PI * 440.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!(!is_silent(&samples));
    }

    #[test]
    fn brief_loud_burst_is_not_silent() {
        // 100 ms of speech (1 600 samples @ 16 kHz) followed by silence.
        // Even with only ~10 % of the clip having energy, total RMS
        // should be above threshold.
        let mut samples = vec![0.0f32; 16_000];
        for i in 0..1_600 {
            samples[i] = 0.3 * (2.0 * PI * 440.0 * i as f32 / 16_000.0).sin();
        }
        // RMS ≈ sqrt(1600 * (0.3/sqrt(2))^2 / 16000) = 0.3/sqrt(2) * sqrt(0.1) ≈ 0.03
        assert!(!is_silent(&samples));
    }

    #[test]
    fn single_sample_spike_is_silent() {
        // A single-sample impulse in an otherwise silent buffer is not speech;
        // RMS = peak / sqrt(n) which is negligibly small.
        let mut samples = vec![0.0f32; 16_000];
        samples[8_000] = 1.0; // full-scale spike
        // RMS = 1.0 / sqrt(16000) ≈ 0.0079 — below threshold
        assert!(is_silent(&samples));
    }

    // --- edge cases ----------------------------------------------------------

    #[test]
    fn single_sample_above_threshold_rms() {
        // Verify the formula: a single sample whose squared value gives
        // RMS exactly at threshold is handled consistently.
        let val = SILENCE_RMS_THRESHOLD; // RMS of [val] = val itself
        let samples = vec![val];
        // RMS == threshold → not silent (we use strict `<`)
        assert!(!is_silent(&samples));
    }
}
