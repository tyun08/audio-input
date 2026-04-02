use anyhow::Result;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::io::Cursor;
use tracing::info;

const TARGET_SAMPLE_RATE: u32 = 16000;

/// 将录音采样数据编码为 WAV bytes（16kHz, 单声道, 16-bit PCM）
/// Whisper 的最优输入格式
pub fn encode_wav(samples: &[f32], source_rate: u32, source_channels: u16) -> Result<Vec<u8>> {
    // 下混到单声道
    let mono = if source_channels > 1 {
        downsample_to_mono(samples, source_channels)
    } else {
        samples.to_vec()
    };

    // 重采样到 16kHz
    let resampled = if source_rate != TARGET_SAMPLE_RATE {
        resample(&mono, source_rate, TARGET_SAMPLE_RATE)
    } else {
        mono
    };

    info!(
        "编码 WAV: {} 采样点 @ {}Hz → {} 采样点 @ {}Hz",
        samples.len(),
        source_rate,
        resampled.len(),
        TARGET_SAMPLE_RATE
    );

    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut buf = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(&mut buf, spec)?;

    for sample in &resampled {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(pcm)?;
    }

    writer.finalize()?;
    Ok(buf.into_inner())
}

fn downsample_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels as usize;
    samples
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// 线性插值重采样
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        let s0 = samples.get(src_idx).copied().unwrap_or(0.0);
        let s1 = samples.get(src_idx + 1).copied().unwrap_or(s0);
        out.push(s0 + frac * (s1 - s0));
    }

    out
}
