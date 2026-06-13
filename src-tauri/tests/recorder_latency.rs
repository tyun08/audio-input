//! Integration tests for the async (zero-wait) recording start path.
//!
//! These run without any audio hardware: the device probe / stream build is
//! offloaded to a background thread, so the synchronous `start_capture` and
//! `stop` calls return immediately regardless of whether a microphone exists.

use audio_input_lib::audio::Recorder;
use std::time::{Duration, Instant};

/// Hotkey press -> `start_capture` returning must be effectively instantaneous.
/// The blocking device probe now happens off-thread, so even a generous bound
/// (well under the issue's 50 ms acceptance criterion) holds on headless CI.
#[test]
fn start_capture_returns_without_blocking() {
    let mut recorder = Recorder::new();

    let t0 = Instant::now();
    recorder
        .start_capture(None, None)
        .expect("start_capture must not fail synchronously");
    let elapsed = t0.elapsed();

    let _ = recorder.stop();

    assert!(
        elapsed < Duration::from_millis(50),
        "start_capture blocked for {:?}, expected < 50ms (zero-wait start)",
        elapsed
    );
}

/// `stop` must also return promptly so the recording tail isn't clipped.
#[test]
fn stop_returns_without_blocking() {
    let mut recorder = Recorder::new();
    recorder.start_capture(None, None).expect("start_capture");

    let t0 = Instant::now();
    let _ = recorder.stop().expect("stop must succeed");
    let elapsed = t0.elapsed();

    assert!(
        elapsed < Duration::from_millis(50),
        "stop blocked for {:?}, expected < 50ms (zero-wait stop)",
        elapsed
    );
}

/// `start_requested_at` is recorded synchronously so latency tooling can anchor
/// measurements to the moment the hotkey fired.
#[test]
fn start_records_request_instant() {
    let mut recorder = Recorder::new();
    assert!(recorder.start_requested_at().is_none());

    recorder.start_capture(None, None).expect("start_capture");
    assert!(recorder.start_requested_at().is_some());

    let _ = recorder.stop();
}

/// Rapid start/stop cycling must remain safe (the epoch guard prevents a slow
/// background probe from resurrecting a stopped stream) and never panic.
#[test]
fn rapid_start_stop_cycles_are_safe() {
    let mut recorder = Recorder::new();
    for _ in 0..25 {
        recorder.start_capture(None, None).expect("start_capture");
        // Must always succeed even when start/stop interleave with the
        // background setup thread.
        let _ = recorder.stop().expect("stop");
    }
}

/// A fresh start clears any samples left in the shared buffer from a prior run.
/// Checked immediately after `start_capture` returns — the buffer is cleared
/// synchronously before the background device thread is spawned, so this holds
/// even on a machine with a real microphone (no sample can arrive within the
/// microseconds before the assertion).
#[test]
fn start_resets_buffer() {
    let mut recorder = Recorder::new();

    // Simulate residual samples from a previous capture.
    recorder
        .get_buffer_ref()
        .lock()
        .unwrap()
        .extend_from_slice(&[0.1, 0.2, 0.3]);

    recorder.start_capture(None, None).expect("start_capture");
    assert!(
        recorder.get_buffer_ref().lock().unwrap().is_empty(),
        "start_capture should clear the shared buffer synchronously"
    );
    let _ = recorder.stop();
}
