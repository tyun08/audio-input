//! Latency benchmark suite for the recording pipeline.
//!
//! Run with `cargo bench` (the crate declares `harness = false` for this
//! target, so this `main` runs directly) or execute the produced binary in CI.
//!
//! It measures the metrics called out in the "async mic start" issue:
//!
//! * **Start latency** — wall-clock time the hotkey handler is blocked by
//!   `Recorder::start_capture` before it returns. This is the regression the
//!   async refactor targets: device probing / stream setup used to run
//!   synchronously here (often 1–2 s); it now runs on a background thread, so
//!   this should be sub-millisecond.
//! * **Stop latency** — wall-clock time `Recorder::stop` blocks the caller, so
//!   the tail of the recording isn't clipped by a slow stop path.
//! * **First-sample latency** — time from start request to the first audio
//!   sample landing in the shared buffer. Requires a real input device; when
//!   none is available (e.g. headless CI) it is reported as "n/a" rather than
//!   failing the run.
//!
//! The process exits non-zero if the synchronous start/stop latencies regress
//! past their thresholds, so it doubles as a guard in automated runs.

use audio_input_lib::audio::Recorder;
use std::time::{Duration, Instant};

/// Number of start/stop cycles to average the synchronous latencies over.
const ITERATIONS: usize = 20;

/// The hotkey handler must regain control near-instantly. The issue's
/// acceptance criterion is < 50 ms; we hold a much tighter bound here because
/// the work is now just buffer reset + thread spawn.
const START_LATENCY_BUDGET: Duration = Duration::from_millis(50);

/// Stopping must not block (and therefore clip the tail) on a slow path.
const STOP_LATENCY_BUDGET: Duration = Duration::from_millis(50);

/// How long to wait for the first sample when a device is present.
const FIRST_SAMPLE_TIMEOUT: Duration = Duration::from_secs(2);

fn measure_sync_latencies() -> (Duration, Duration) {
    let mut recorder = Recorder::new();
    let mut worst_start = Duration::ZERO;
    let mut worst_stop = Duration::ZERO;

    for _ in 0..ITERATIONS {
        let t0 = Instant::now();
        // `None` for the error callback: a setup failure (e.g. no microphone in
        // CI) is irrelevant to the *synchronous* latency we are measuring here.
        recorder
            .start_capture(None, None)
            .expect("start_capture should return immediately");
        worst_start = worst_start.max(t0.elapsed());

        let t1 = Instant::now();
        let _ = recorder.stop().expect("stop should succeed");
        worst_stop = worst_stop.max(t1.elapsed());
    }

    (worst_start, worst_stop)
}

/// Returns the hotkey-to-first-sample latency if an input device produces audio
/// within the timeout, otherwise `None` (e.g. headless CI with no microphone).
fn measure_first_sample_latency() -> Option<Duration> {
    let mut recorder = Recorder::new();
    let buffer = recorder.get_buffer_ref();

    recorder
        .start_capture(None, None)
        .expect("start_capture should return immediately");
    let requested_at = recorder
        .start_requested_at()
        .expect("start_requested_at should be set after start");

    let deadline = Instant::now() + FIRST_SAMPLE_TIMEOUT;
    let latency = loop {
        if !buffer.lock().unwrap().is_empty() {
            break Some(requested_at.elapsed());
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(1));
    };

    let _ = recorder.stop();
    latency
}

fn main() {
    println!("recording latency benchmark ({} iterations)\n", ITERATIONS);

    let (start_latency, stop_latency) = measure_sync_latencies();
    println!(
        "start latency (hotkey -> start returns): {:>8.3} ms  (budget {:.0} ms)",
        start_latency.as_secs_f64() * 1e3,
        START_LATENCY_BUDGET.as_secs_f64() * 1e3,
    );
    println!(
        "stop  latency (hotkey -> stop  returns): {:>8.3} ms  (budget {:.0} ms)",
        stop_latency.as_secs_f64() * 1e3,
        STOP_LATENCY_BUDGET.as_secs_f64() * 1e3,
    );

    match measure_first_sample_latency() {
        Some(d) => println!(
            "first-sample latency (hotkey -> sample): {:>8.3} ms",
            d.as_secs_f64() * 1e3,
        ),
        None => {
            println!("first-sample latency (hotkey -> sample):      n/a  (no input device available)")
        }
    }

    let mut failed = false;
    if start_latency > START_LATENCY_BUDGET {
        eprintln!(
            "\nFAIL: start latency {:.3} ms exceeds budget {:.0} ms",
            start_latency.as_secs_f64() * 1e3,
            START_LATENCY_BUDGET.as_secs_f64() * 1e3,
        );
        failed = true;
    }
    if stop_latency > STOP_LATENCY_BUDGET {
        eprintln!(
            "FAIL: stop latency {:.3} ms exceeds budget {:.0} ms",
            stop_latency.as_secs_f64() * 1e3,
            STOP_LATENCY_BUDGET.as_secs_f64() * 1e3,
        );
        failed = true;
    }

    if failed {
        std::process::exit(1);
    }
    println!("\nOK: synchronous start/stop latencies within budget");
}
