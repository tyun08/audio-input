use audio_input_lib::audio::{is_silent, SILENCE_RMS_THRESHOLD};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Silence cases
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Speech cases
// ---------------------------------------------------------------------------

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
    for (i, sample) in samples.iter_mut().take(1_600).enumerate() {
        *sample = 0.3 * (2.0 * PI * 440.0 * i as f32 / 16_000.0).sin();
    }
    // RMS ≈ sqrt(1600 * (0.3/sqrt(2))^2 / 16000) = 0.3/sqrt(2) * sqrt(0.1) ≈ 0.03
    assert!(!is_silent(&samples));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn single_sample_spike_is_silent() {
    // A single-sample impulse in an otherwise silent buffer is not speech;
    // RMS = peak / sqrt(n) which is negligibly small.
    let mut samples = vec![0.0f32; 16_000];
    samples[8_000] = 0.5;
    // RMS = 0.5 / sqrt(16000) ≈ 0.004 — below threshold
    assert!(is_silent(&samples));
}

#[test]
fn single_sample_at_threshold_rms_is_not_silent() {
    // A single sample whose value equals the threshold has RMS == threshold.
    // The check is strict (<), so it must NOT be treated as silent.
    let val = SILENCE_RMS_THRESHOLD; // RMS of [val] = val itself
    let samples = vec![val];
    assert!(!is_silent(&samples));
}
