use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, Stream};
use std::sync::{Arc, Mutex};
use tauri::{Manager, Runtime};
use tracing::{error, info, warn};

// cpal::Stream is not Send on all platforms; we control access via Mutex
#[allow(dead_code)]
struct SendStream(Stream);
unsafe impl Send for SendStream {}

pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct Recorder {
    stream: Option<SendStream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices
            .filter_map(|d| d.name().ok())
            .collect(),
        Err(e) => {
            warn!("Failed to list input devices: {}", e);
            Vec::new()
        }
    }
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: 0,
            channels: 0,
        }
    }

    pub fn start<R: Runtime>(&mut self, app: &tauri::AppHandle<R>) -> Result<()> {
        let host = cpal::default_host();

        // Read preferred device from config
        let preferred_device_name = {
            let config_state = app.state::<Arc<Mutex<crate::config::AppConfig>>>();
            let name = config_state.lock().unwrap().preferred_device.clone();
            name
        };

        let device = if let Some(ref name) = preferred_device_name {
            // Try to find the preferred device
            let found = host
                .input_devices()
                .ok()
                .and_then(|mut devs| devs.find(|d| d.name().as_deref().ok() == Some(name.as_str())));

            if let Some(d) = found {
                info!("Using configured recording device: {}", name);
                d
            } else {
                warn!("Configured device '{}' unavailable, falling back to default", name);
                host.default_input_device()
                    .context("No default microphone found")?
            }
        } else {
            host.default_input_device()
                .context("No default microphone found")?
        };

        info!("Using recording device: {}", device.name().unwrap_or_default());

        let config = device
            .default_input_config()
            .context("Cannot get default recording config")?;

        self.sample_rate = config.sample_rate().0;
        self.channels = config.channels();

        info!(
            "Recording config: {}Hz, {} channels, {:?}",
            self.sample_rate,
            self.channels,
            config.sample_format()
        );

        let buffer = Arc::clone(&self.buffer);
        {
            let mut buf = buffer.lock().unwrap();
            buf.clear();
        }

        let stream = match config.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), buffer)?,
            SampleFormat::I16 => build_stream_i16(&device, &config.into(), buffer)?,
            SampleFormat::U16 => build_stream_u16(&device, &config.into(), buffer)?,
            fmt => anyhow::bail!("Unsupported audio format: {:?}", fmt),
        };

        stream.play().context("Failed to start recording stream")?;
        self.stream = Some(SendStream(stream));
        info!("Recording started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<AudioData> {
        // Drop the stream to stop recording
        self.stream.take();
        info!("Recording stopped");

        let samples = {
            let buf = self.buffer.lock().unwrap();
            buf.clone()
        };

        info!("Recorded {} samples ({:.1}s)", samples.len(), samples.len() as f32 / self.sample_rate as f32);

        Ok(AudioData {
            samples,
            sample_rate: self.sample_rate,
            channels: self.channels,
        })
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: FromSample<T>,
{
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
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
) -> Result<Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
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
) -> Result<Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _: &cpal::InputCallbackInfo| {
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
