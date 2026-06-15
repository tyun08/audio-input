use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{Manager, Runtime};
use tracing::{error, info, warn};

// cpal::Stream is not Send on all platforms; we control access via Mutex.
struct SendStream(Stream);
unsafe impl Send for SendStream {}

struct CachedStream {
    stream: SendStream,
    preferred_device_name: Option<String>,
}

// RAII guard: dropping a SendStream always pauses the underlying cpal stream
// first. cpal's own Drop does NOT reliably halt the input callback on
// macOS/coreaudio — without this explicit pause the stream becomes a "zombie"
// that keeps appending to the shared buffer (inflating recording duration and
// causing ASR hallucination) and holds the mic open (the OS "mic in use"
// indicator never turns off). Putting it in Drop guarantees cleanup on every
// path: stop(), stream replacement in start(), Recorder teardown, and panics.
impl Drop for SendStream {
    fn drop(&mut self) {
        if let Err(e) = self.0.pause() {
            warn!("Failed to pause recording stream on drop: {}", e);
        }
    }
}

pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct RecordingLatencySnapshot {
    pub epoch: u64,
    pub trigger_received_at: Option<Instant>,
    pub start_requested_at: Option<Instant>,
    pub setup_thread_started_at: Option<Instant>,
    pub host_created_at: Option<Instant>,
    pub device_resolved_at: Option<Instant>,
    pub device_name_resolved_at: Option<Instant>,
    pub config_loaded_at: Option<Instant>,
    pub stream_built_at: Option<Instant>,
    pub stream_play_requested_at: Option<Instant>,
    pub stream_play_returned_at: Option<Instant>,
    pub first_sample_at: Option<Instant>,
    pub used_prebuilt_stream: bool,
}

// Shared, cheaply-clonable handle to the live capture state. Cloning hands a
// background thread everything it needs to probe the device, build the cpal
// stream and start capture *without* blocking the caller (the hotkey handler).
#[derive(Clone)]
struct CaptureState {
    stream: Arc<Mutex<Option<CachedStream>>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: Arc<AtomicU32>,
    channels: Arc<AtomicU32>,
    trigger_received_at: Arc<Mutex<Option<Instant>>>,
    setup_thread_started_at: Arc<Mutex<Option<Instant>>>,
    host_created_at: Arc<Mutex<Option<Instant>>>,
    device_resolved_at: Arc<Mutex<Option<Instant>>>,
    device_name_resolved_at: Arc<Mutex<Option<Instant>>>,
    config_loaded_at: Arc<Mutex<Option<Instant>>>,
    stream_built_at: Arc<Mutex<Option<Instant>>>,
    stream_play_requested_at: Arc<Mutex<Option<Instant>>>,
    stream_play_returned_at: Arc<Mutex<Option<Instant>>>,
    first_sample_at: Arc<Mutex<Option<Instant>>>,
    first_sample_seen: Arc<AtomicBool>,
    used_prebuilt_stream: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    // Monotonically increasing generation counter. Each start()/stop() bumps it.
    // The background setup thread captures the epoch at spawn time and only
    // installs the stream if the epoch is still current — otherwise a slow
    // device probe could resurrect a stream that stop() already tore down.
    epoch: Arc<AtomicU64>,
}

pub struct Recorder {
    capture: CaptureState,
    // Instant the most recent start was requested. Exposed for latency
    // benchmarks (see benches/latency.rs).
    start_requested_at: Option<Instant>,
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(e) => {
            warn!("Failed to list input devices: {}", e);
            Vec::new()
        }
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            capture: CaptureState {
                stream: Arc::new(Mutex::new(None)),
                buffer: Arc::new(Mutex::new(Vec::new())),
                sample_rate: Arc::new(AtomicU32::new(0)),
                channels: Arc::new(AtomicU32::new(0)),
                trigger_received_at: Arc::new(Mutex::new(None)),
                setup_thread_started_at: Arc::new(Mutex::new(None)),
                host_created_at: Arc::new(Mutex::new(None)),
                device_resolved_at: Arc::new(Mutex::new(None)),
                device_name_resolved_at: Arc::new(Mutex::new(None)),
                config_loaded_at: Arc::new(Mutex::new(None)),
                stream_built_at: Arc::new(Mutex::new(None)),
                stream_play_requested_at: Arc::new(Mutex::new(None)),
                stream_play_returned_at: Arc::new(Mutex::new(None)),
                first_sample_at: Arc::new(Mutex::new(None)),
                first_sample_seen: Arc::new(AtomicBool::new(false)),
                used_prebuilt_stream: Arc::new(AtomicBool::new(false)),
                is_recording: Arc::new(AtomicBool::new(false)),
                epoch: Arc::new(AtomicU64::new(0)),
            },
            start_requested_at: None,
        }
    }

    /// Begin recording. Reads the preferred device from config (a fast,
    /// non-blocking lookup) and then delegates to [`Recorder::start_capture`],
    /// which returns immediately — the actual device probe and stream build run
    /// on a background thread so the hotkey handler is never blocked.
    ///
    /// `on_error` is invoked (on the background thread) if the asynchronous
    /// setup fails, so callers can surface a "microphone unavailable" state
    /// without blocking the start path.
    pub fn start<R: Runtime>(
        &mut self,
        app: &tauri::AppHandle<R>,
        on_error: Box<dyn FnOnce(anyhow::Error) + Send>,
    ) -> Result<()> {
        self.start_with_trigger_at(app, on_error, None)
    }

    pub fn start_with_trigger_at<R: Runtime>(
        &mut self,
        app: &tauri::AppHandle<R>,
        on_error: Box<dyn FnOnce(anyhow::Error) + Send>,
        trigger_received_at: Option<Instant>,
    ) -> Result<()> {
        // Read preferred device from config. This is a quick in-memory lock,
        // not a blocking device probe, so it stays on the calling thread.
        let preferred_device_name = {
            let config_state = app.state::<Arc<Mutex<crate::config::AppConfig>>>();
            let name = config_state.lock().unwrap().preferred_device.clone();
            name
        };
        self.start_capture_with_trigger_at(
            preferred_device_name,
            Some(on_error),
            trigger_received_at,
        )
    }

    /// Zero-wait start: prepares the shared capture state synchronously (clear
    /// the buffer, bump the epoch, drop any stale stream) and offloads the
    /// blocking device probe + stream build to a background thread. Returns as
    /// soon as that thread is spawned — typically within microseconds — so the
    /// first audio sample is captured the instant the OS delivers the stream
    /// rather than after a 1–2 s synchronous setup.
    ///
    /// This is the device-agnostic core used by both [`Recorder::start`] and the
    /// latency benchmarks.
    pub fn start_capture(
        &mut self,
        preferred_device_name: Option<String>,
        on_error: Option<Box<dyn FnOnce(anyhow::Error) + Send>>,
    ) -> Result<()> {
        self.start_capture_with_trigger_at(preferred_device_name, on_error, None)
    }

    pub fn start_capture_with_trigger_at(
        &mut self,
        preferred_device_name: Option<String>,
        on_error: Option<Box<dyn FnOnce(anyhow::Error) + Send>>,
        trigger_received_at: Option<Instant>,
    ) -> Result<()> {
        let requested_at = Instant::now();
        self.start_requested_at = Some(requested_at);

        // Bump the generation. This both invalidates any in-flight background
        // setup from a previous start and tags the one we are about to spawn.
        let epoch = self.capture.epoch.fetch_add(1, Ordering::SeqCst) + 1;

        {
            let mut trigger = self.capture.trigger_received_at.lock().unwrap();
            *trigger = trigger_received_at;
        }
        clear_instant(&self.capture.setup_thread_started_at);
        clear_instant(&self.capture.host_created_at);
        clear_instant(&self.capture.device_resolved_at);
        clear_instant(&self.capture.device_name_resolved_at);
        clear_instant(&self.capture.config_loaded_at);
        clear_instant(&self.capture.stream_built_at);
        {
            let mut stream_play = self.capture.stream_play_requested_at.lock().unwrap();
            *stream_play = None;
        }
        clear_instant(&self.capture.stream_play_returned_at);
        {
            let mut first_sample = self.capture.first_sample_at.lock().unwrap();
            *first_sample = None;
        }
        self.capture
            .first_sample_seen
            .store(false, Ordering::SeqCst);
        self.capture
            .used_prebuilt_stream
            .store(false, Ordering::SeqCst);

        // Reset the shared buffer up front so capture begins from a clean slate.
        {
            let mut buf = self.capture.buffer.lock().unwrap();
            buf.clear();
        }

        let mut slot = self.capture.stream.lock().unwrap();
        if let Some(cached) = slot.as_mut() {
            if cached.preferred_device_name.as_deref() == preferred_device_name.as_deref() {
                self.capture
                    .used_prebuilt_stream
                    .store(true, Ordering::SeqCst);
                set_instant(&self.capture.stream_play_requested_at);
                match cached.stream.0.play() {
                    Ok(()) => {
                        set_instant(&self.capture.stream_play_returned_at);
                        self.capture.is_recording.store(true, Ordering::SeqCst);
                        info!("Recording started (prebuilt stream)");
                        return Ok(());
                    }
                    Err(e) => {
                        warn!("Prebuilt recording stream failed to play: {}", e);
                        *slot = None;
                    }
                }
            } else {
                *slot = None;
            }
        }
        drop(slot);

        let capture = self.capture.clone();
        std::thread::Builder::new()
            .name("recorder-start".into())
            .spawn(move || {
                if let Err(e) = build_and_play(&capture, preferred_device_name.as_deref(), epoch) {
                    error!("Background recording start failed: {}", e);
                    if let Some(cb) = on_error {
                        cb(e);
                    }
                }
            })
            .context("Failed to spawn recorder start thread")?;

        Ok(())
    }

    pub fn get_buffer_ref(&self) -> Arc<Mutex<Vec<f32>>> {
        Arc::clone(&self.capture.buffer)
    }

    pub fn sample_rate(&self) -> u32 {
        self.capture.sample_rate.load(Ordering::SeqCst)
    }

    /// Lock-free handle to the live sample rate. The background setup thread
    /// stores the real rate here once the device is configured, so latency-
    /// sensitive pollers (e.g. the audio-level monitor) can read it without
    /// contending on the recorder mutex that `start()`/`stop()` rely on being
    /// snappy.
    pub fn sample_rate_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.capture.sample_rate)
    }

    /// Instant at which the most recent start was requested, if any. Used by the
    /// latency benchmarks to measure hotkey-to-first-sample time.
    pub fn start_requested_at(&self) -> Option<Instant> {
        self.start_requested_at
    }

    pub fn latency_snapshot(&self) -> RecordingLatencySnapshot {
        RecordingLatencySnapshot {
            epoch: self.capture.epoch.load(Ordering::SeqCst),
            trigger_received_at: *self.capture.trigger_received_at.lock().unwrap(),
            start_requested_at: self.start_requested_at,
            setup_thread_started_at: *self.capture.setup_thread_started_at.lock().unwrap(),
            host_created_at: *self.capture.host_created_at.lock().unwrap(),
            device_resolved_at: *self.capture.device_resolved_at.lock().unwrap(),
            device_name_resolved_at: *self.capture.device_name_resolved_at.lock().unwrap(),
            config_loaded_at: *self.capture.config_loaded_at.lock().unwrap(),
            stream_built_at: *self.capture.stream_built_at.lock().unwrap(),
            stream_play_requested_at: *self.capture.stream_play_requested_at.lock().unwrap(),
            stream_play_returned_at: *self.capture.stream_play_returned_at.lock().unwrap(),
            first_sample_at: *self.capture.first_sample_at.lock().unwrap(),
            used_prebuilt_stream: self.capture.used_prebuilt_stream.load(Ordering::SeqCst),
        }
    }

    pub fn stop(&mut self) -> Result<AudioData> {
        // Bump the epoch so any background setup still in flight discards its
        // stream instead of resurrecting capture after we stop.
        self.capture.epoch.fetch_add(1, Ordering::SeqCst);

        // Release the stream; SendStream's Drop pauses the cpal stream, halting
        // the input callback and releasing the mic. (cpal's own Drop is not
        // enough on macOS/coreaudio — see SendStream's Drop impl.)
        {
            let mut slot = self.capture.stream.lock().unwrap();
            if let Some(cached) = slot.as_mut() {
                if let Err(e) = cached.stream.0.pause() {
                    warn!("Failed to pause recording stream: {}", e);
                    *slot = None;
                }
            }
        }
        self.capture.is_recording.store(false, Ordering::SeqCst);
        info!("Recording stopped");

        let samples = {
            let buf = self.capture.buffer.lock().unwrap();
            buf.clone()
        };

        let sample_rate = self.capture.sample_rate.load(Ordering::SeqCst);
        let channels = self.capture.channels.load(Ordering::SeqCst) as u16;

        info!(
            "Recorded {} samples ({:.1}s)",
            samples.len(),
            if sample_rate > 0 {
                samples.len() as f32 / sample_rate as f32
            } else {
                0.0
            }
        );

        Ok(AudioData {
            samples,
            sample_rate,
            channels,
        })
    }

    /// Build and cache a paused input stream in the background. A later start
    /// can reuse it and call `play()` immediately instead of paying the
    /// CoreAudio/cpal stream-construction cost on the hotkey path.
    pub fn prewarm_capture(&self, preferred_device_name: Option<String>) -> Result<()> {
        let capture = self.capture.clone();
        let epoch = capture.epoch.load(Ordering::SeqCst);
        std::thread::Builder::new()
            .name("recorder-prewarm".into())
            .spawn(move || {
                match build_cached_stream(&capture, preferred_device_name.as_deref(), false) {
                    Ok(cached) => {
                        let mut slot = capture.stream.lock().unwrap();
                        if capture.epoch.load(Ordering::SeqCst) == epoch
                            && !capture.is_recording.load(Ordering::SeqCst)
                        {
                            *slot = Some(cached);
                            info!("Recording stream prewarmed");
                        } else {
                            info!("Recording stream prewarm superseded; discarding stream");
                        }
                    }
                    Err(e) => warn!("Recording stream prewarm failed: {}", e),
                }
            })
            .context("Failed to spawn recorder prewarm thread")?;

        Ok(())
    }
}

/// Blocking device probe + stream build + play. Runs on a background thread so
/// it never delays the caller. Installs the stream only if `epoch` is still the
/// current generation, guaranteeing a stopped/restarted recorder can't be
/// resurrected by a slow probe.
fn build_and_play(
    capture: &CaptureState,
    preferred_device_name: Option<&str>,
    epoch: u64,
) -> Result<()> {
    set_instant(&capture.setup_thread_started_at);

    let stream = build_cached_stream(capture, preferred_device_name, true)?;

    // Install the stream atomically with the epoch check so a stop()/start()
    // that happened while we were probing wins the race. Holding the slot lock
    // across play() means stop() either runs fully before us (epoch bumped →
    // we bail) or fully after us (it tears our stream down).
    let mut slot = capture.stream.lock().unwrap();
    if capture.epoch.load(Ordering::SeqCst) != epoch {
        info!("Recording start superseded before stream install; discarding stream");
        // Dropping `stream` here pauses it via cpal's own Drop (it was never
        // played, so nothing to halt).
        return Ok(());
    }
    set_instant(&capture.stream_play_requested_at);
    stream
        .stream
        .0
        .play()
        .context("Failed to start recording stream")?;
    set_instant(&capture.stream_play_returned_at);
    capture.is_recording.store(true, Ordering::SeqCst);
    *slot = Some(stream);
    info!("Recording started");
    Ok(())
}

fn build_cached_stream(
    capture: &CaptureState,
    preferred_device_name: Option<&str>,
    record_timings: bool,
) -> Result<CachedStream> {
    let host = cpal::default_host();
    set_instant_if(record_timings, &capture.host_created_at);

    let device = if let Some(name) = preferred_device_name {
        let found = host
            .input_devices()
            .ok()
            .and_then(|mut devs| devs.find(|d| d.name().as_deref().ok() == Some(name)));

        if let Some(d) = found {
            info!("Using configured recording device: {}", name);
            d
        } else {
            warn!(
                "Configured device '{}' unavailable, falling back to default",
                name
            );
            host.default_input_device()
                .context("No default microphone found")?
        }
    } else {
        host.default_input_device()
            .context("No default microphone found")?
    };
    set_instant_if(record_timings, &capture.device_resolved_at);

    info!(
        "Using recording device: {}",
        device.name().unwrap_or_default()
    );
    set_instant_if(record_timings, &capture.device_name_resolved_at);

    let config = device
        .default_input_config()
        .context("Cannot get default recording config")?;
    set_instant_if(record_timings, &capture.config_loaded_at);

    capture
        .sample_rate
        .store(config.sample_rate().0, Ordering::SeqCst);
    capture
        .channels
        .store(config.channels() as u32, Ordering::SeqCst);

    info!(
        "Recording config: {}Hz, {} channels, {:?}",
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    );

    let stream = match config.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config.into(),
            Arc::clone(&capture.buffer),
            Arc::clone(&capture.first_sample_at),
            Arc::clone(&capture.first_sample_seen),
        )?,
        SampleFormat::I16 => build_stream_i16(
            &device,
            &config.into(),
            Arc::clone(&capture.buffer),
            Arc::clone(&capture.first_sample_at),
            Arc::clone(&capture.first_sample_seen),
        )?,
        SampleFormat::U16 => build_stream_u16(
            &device,
            &config.into(),
            Arc::clone(&capture.buffer),
            Arc::clone(&capture.first_sample_at),
            Arc::clone(&capture.first_sample_seen),
        )?,
        fmt => anyhow::bail!("Unsupported audio format: {:?}", fmt),
    };
    set_instant_if(record_timings, &capture.stream_built_at);

    Ok(CachedStream {
        stream: SendStream(stream),
        preferred_device_name: preferred_device_name.map(str::to_string),
    })
}

fn clear_instant(slot: &Arc<Mutex<Option<Instant>>>) {
    *slot.lock().unwrap() = None;
}

fn set_instant(slot: &Arc<Mutex<Option<Instant>>>) {
    *slot.lock().unwrap() = Some(Instant::now());
}

fn set_instant_if(enabled: bool, slot: &Arc<Mutex<Option<Instant>>>) {
    if enabled {
        set_instant(slot);
    }
}

fn mark_first_sample(
    first_sample_at: &Arc<Mutex<Option<Instant>>>,
    first_sample_seen: &Arc<AtomicBool>,
    sample_count: usize,
) {
    if sample_count > 0 && !first_sample_seen.swap(true, Ordering::SeqCst) {
        let mut first_sample = first_sample_at.lock().unwrap();
        *first_sample = Some(Instant::now());
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    first_sample_at: Arc<Mutex<Option<Instant>>>,
    first_sample_seen: Arc<AtomicBool>,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: FromSample<T>,
{
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            mark_first_sample(&first_sample_at, &first_sample_seen, data.len());
            let mut buf = buffer.lock().unwrap();
            for &sample in data {
                buf.push(<f32 as FromSample<T>>::from_sample_(sample));
            }
        },
        move |err| error!("Recording error: {}", err),
        None,
    )?;
    Ok(stream)
}

fn build_stream_i16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    first_sample_at: Arc<Mutex<Option<Instant>>>,
    first_sample_seen: Arc<AtomicBool>,
) -> Result<Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            mark_first_sample(&first_sample_at, &first_sample_seen, data.len());
            let mut buf = buffer.lock().unwrap();
            for &sample in data {
                buf.push(sample as f32 / i16::MAX as f32);
            }
        },
        move |err| error!("Recording error: {}", err),
        None,
    )?;
    Ok(stream)
}

fn build_stream_u16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    first_sample_at: Arc<Mutex<Option<Instant>>>,
    first_sample_seen: Arc<AtomicBool>,
) -> Result<Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _: &cpal::InputCallbackInfo| {
            mark_first_sample(&first_sample_at, &first_sample_seen, data.len());
            let mut buf = buffer.lock().unwrap();
            for &sample in data {
                // u16: 0..=65535, center at 32768
                buf.push((sample as f32 - 32768.0) / 32768.0);
            }
        },
        move |err| error!("Recording error: {}", err),
        None,
    )?;
    Ok(stream)
}
