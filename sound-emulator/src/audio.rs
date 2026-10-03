//! Microphone capture and WAV persistence.
//!
//! We record a short mono clip of environmental sound into a plain `Vec<f32>`
//! (samples normalised to roughly -1.0..1.0). Everything downstream — the FFT,
//! the spectrogram, the terrain — only ever sees that buffer, so this module is
//! the single place that knows about audio hardware.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A finished recording: interleaved-to-mono samples plus the rate they were
/// captured at (needed so the FFT can label its frequency bins in real Hz).
#[derive(Clone)]
pub struct Recording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Recording {
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f32 / self.sample_rate as f32
        }
    }
}

/// Record `seconds` of audio from the default input device. Blocking — call it
/// from a worker thread so the UI stays responsive.
pub fn record(seconds: f32) -> Result<Recording> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device — is a microphone connected?"))?;
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    // Shared buffer the audio callback appends into.
    let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let err_fn = |e| eprintln!("audio stream error: {e}");

    // The callback runs on a real-time thread; keep it cheap — just downmix to
    // mono (average the channels) and push.
    macro_rules! build {
        ($sample:ty, $to_f32:expr) => {{
            let buffer = Arc::clone(&buffer);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[$sample], _: &cpal::InputCallbackInfo| {
                    let mut buf = buffer.lock().unwrap();
                    for frame in data.chunks(channels) {
                        let sum: f32 = frame.iter().copied().map($to_f32).sum();
                        buf.push(sum / channels as f32);
                    }
                },
                err_fn,
                None,
            )?
        }};
    }

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build!(f32, |s| s),
        cpal::SampleFormat::I16 => build!(i16, |s| s as f32 / i16::MAX as f32),
        cpal::SampleFormat::U16 => build!(u16, |s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0),
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play()?;
    std::thread::sleep(Duration::from_secs_f32(seconds));
    drop(stream); // stops capture

    let samples = Arc::try_unwrap(buffer)
        .map_err(|_| anyhow!("recording buffer still shared"))?
        .into_inner()
        .unwrap();

    Ok(Recording {
        samples,
        sample_rate,
    })
}

/// Save a recording to a 16-bit mono WAV so a clip can be reused later.
pub fn save_wav(rec: &Recording, path: impl AsRef<Path>) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rec.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in &rec.samples {
        let clamped = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(clamped)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Load a previously saved WAV back into a `Recording`.
pub fn load_wav(path: impl AsRef<Path>) -> Result<Recording> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;

    // Read every sample as f32 regardless of on-disk format, then downmix.
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max)
                .collect()
        }
    };

    let samples = if channels <= 1 {
        raw
    } else {
        raw.chunks(channels)
            .map(|f| f.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok(Recording {
        samples,
        sample_rate: spec.sample_rate,
    })
}
