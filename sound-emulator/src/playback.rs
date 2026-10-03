//! Audio playback, so you can *hear* the clip that's driving the terrain.
//!
//! A worker thread owns the cpal output stream and the UI only ever reads a
//! shared frame counter. That counter is what keeps the live terrain lined up
//! with what's actually coming out of the speakers — the scroll follows the
//! audio, not a wall clock, so they can't drift apart.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// A looping playback of one clip. Dropping it stops the sound.
pub struct Playback {
    /// Frames of the *clip* consumed so far, counting straight on past each
    /// loop rather than wrapping, so callers can treat it as a monotonic clock.
    played: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Playback {
    /// Start looping `samples` (mono, at `clip_rate`) on the default output.
    /// Blocks only until the stream is confirmed running, so failures surface
    /// here rather than as silence.
    pub fn start(samples: Arc<Vec<f32>>, clip_rate: u32) -> Result<Self> {
        if samples.is_empty() {
            return Err(anyhow!("nothing to play"));
        }

        let played = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        {
            let played = Arc::clone(&played);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || match build_stream(samples, clip_rate, played) {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        let _ = tx.send(Err(format!("could not start output stream: {e}")));
                        return;
                    }
                    let _ = tx.send(Ok(()));
                    // The stream only runs while it's alive, so park here until
                    // the `Playback` handle is dropped.
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            });
        }

        match rx.recv() {
            Ok(Ok(())) => Ok(Playback { played, stop }),
            Ok(Err(e)) => Err(anyhow!(e)),
            Err(_) => Err(anyhow!("playback thread vanished")),
        }
    }

    /// Clip frames played so far — monotonic, counting on through loops.
    pub fn frames_played(&self) -> u64 {
        self.played.load(Ordering::Relaxed)
    }
}

fn build_stream(
    samples: Arc<Vec<f32>>,
    clip_rate: u32,
    played: Arc<AtomicU64>,
) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no default output device"))?;
    let config = device.default_output_config()?;
    let channels = config.channels() as usize;

    // The device rarely runs at the clip's rate, so walk the clip at a fraction
    // of a sample per output frame. That ratio *is* the resample.
    let step = clip_rate as f64 / config.sample_rate().0 as f64;
    let err_fn = |e| eprintln!("playback stream error: {e}");

    macro_rules! build {
        ($sample:ty, $from_f32:expr) => {{
            let samples = Arc::clone(&samples);
            let played = Arc::clone(&played);
            let mut cursor = 0.0f64;
            device.build_output_stream(
                &config.clone().into(),
                move |data: &mut [$sample], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let v = sample_at(&samples, cursor);
                        for slot in frame.iter_mut() {
                            *slot = $from_f32(v);
                        }
                        cursor += step;
                    }
                    played.store(cursor as u64, Ordering::Relaxed);
                },
                err_fn,
                None,
            )?
        }};
    }

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build!(f32, |v: f32| v),
        cpal::SampleFormat::I16 => {
            build!(i16, |v: f32| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        }
        cpal::SampleFormat::U16 => build!(u16, |v: f32| {
            ((v.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16
        }),
        other => return Err(anyhow!("unsupported output sample format: {other:?}")),
    };

    Ok(stream)
}

/// Linearly interpolated read at a fractional position, looping the clip.
fn sample_at(samples: &[f32], cursor: f64) -> f32 {
    let len = samples.len();
    let pos = cursor % len as f64;
    let i = pos as usize;
    let frac = (pos - i as f64) as f32;
    let a = samples[i % len];
    let b = samples[(i + 1) % len];
    a + (b - a) * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live mode scrolls off this counter, so if it doesn't advance in step with
    /// real time the terrain either freezes or races the sound.
    ///
    /// Needs a real output device, so it's opt-in:
    ///   cargo test -- --ignored playback_clock_advances
    #[test]
    #[ignore]
    fn playback_clock_advances() {
        // A silent buffer — this tests the clock, not the sound, and there's no
        // reason to make noise on whoever's machine is running it.
        let samples = Arc::new(vec![0.0f32; 44_100]);
        let playback = Playback::start(samples, 44_100).expect("no output device");

        std::thread::sleep(Duration::from_millis(500));
        let played = playback.frames_played();

        // ~22050 frames in 500ms at 44.1k. Slack is wide because device buffer
        // sizes vary a lot; we're catching "stuck at zero" or "wildly fast".
        assert!(
            (5_000..60_000).contains(&played),
            "clock advanced {played} clip frames in 500ms — expected ~22050"
        );
    }
}
