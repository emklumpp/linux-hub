//! Sound generated in code, so a landscape never depends on a microphone.
//!
//! Everything here hands back the same `Recording` the mic path produces, so the
//! rest of the pipeline can't tell the difference. The layers are chosen for how
//! they *look* once `analysis` turns them into a heightmap:
//!
//!   * exponential sweeps  → diagonal ridges — they climb the log-frequency axis
//!                           at a constant rate, so they read as straight lines
//!   * sustained tones     → ridges running along the time axis
//!   * percussive bursts   → vertical walls spanning every frequency at once
//!   * a swelling drone    → a bass shelf rising toward the near edge

use crate::audio::Recording;
use std::f32::consts::TAU;

/// A layered test soundscape: two crossing sweeps, octave-spaced drones that
/// breathe, and a repeating percussive pulse.
pub fn soundscape(sample_rate: u32, seconds: f32) -> Recording {
    let sr = sample_rate as f32;
    let n = (sr * seconds) as usize;
    let mut samples = vec![0.0f32; n];
    let mut noise = Lcg::new(0x5EED_1474);

    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let progress = t / seconds; // 0..1 through the clip
        let mut v = 0.0;

        // Two crossing sweeps — the diagonals of the terrain.
        v += 0.30 * sweep_phase(60.0, 8000.0, t, seconds).sin();
        v += 0.22 * sweep_phase(6000.0, 100.0, t, seconds).sin();

        // Octave-spaced sustained tones, each breathing at its own rate, so the
        // ridges they draw undulate instead of running dead straight.
        for (k, &f) in [110.0f32, 440.0, 1760.0].iter().enumerate() {
            let tremolo = 0.6 + 0.4 * (TAU * (0.7 + 0.5 * k as f32) * t).sin();
            v += 0.16 * tremolo * (TAU * f * t).sin();
        }

        // A low drone that fades in, lifting the bass edge toward the end.
        v += 0.18 * progress * (TAU * 55.0 * t).sin();

        // Percussive broadband bursts — sharp decay, so each one is a thin wall.
        let since_hit = t % 0.75;
        v += 0.35 * (-since_hit * 22.0).exp() * noise.next_bipolar();

        *s = v * 0.5; // headroom, so save_wav's clamp never bites
    }

    Recording {
        samples,
        sample_rate,
    }
}

/// Phase of an exponential (constant octaves-per-second) sweep from `f0` to `f1`.
///
/// Exponential rather than linear because the spectrogram's frequency axis is
/// logarithmic — this draws a straight diagonal instead of a curve crushed
/// against the bass edge.
fn sweep_phase(f0: f32, f1: f32, t: f32, duration: f32) -> f32 {
    let k = (f1 / f0).ln();
    TAU * f0 * duration / k * ((k * t / duration).exp() - 1.0)
}

/// A tiny deterministic PRNG, so the same soundscape comes back every run and we
/// don't pull in a dependency just for noise.
struct Lcg(u32);

impl Lcg {
    fn new(seed: u32) -> Self {
        Lcg(seed)
    }

    /// Next sample in -1.0..1.0.
    fn next_bipolar(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::compute_spectrogram;

    /// The point of the synth is *relief*. A clip that analyses to a flat field
    /// draws a flat plane — the exact failure a silent microphone produces — so
    /// assert the heightmap actually has structure in it.
    #[test]
    fn soundscape_has_relief() {
        let rec = soundscape(crate::SYNTH_RATE, crate::RECORD_SECS);
        assert!(rec.samples.iter().all(|s| s.is_finite()), "non-finite sample");
        assert!(rec.samples.iter().any(|s| s.abs() > 0.1), "clip is silent");

        // The real grid, so a resolution change can't quietly break this.
        let spec = compute_spectrogram(
            &rec,
            crate::GRID_COLS,
            crate::GRID_ROWS,
            crate::FFT_SIZE,
        );
        let mean = spec.data.iter().sum::<f32>() / spec.data.len() as f32;
        let sd = (spec.data.iter().map(|v| (v - mean).powi(2)).sum::<f32>()
            / spec.data.len() as f32)
            .sqrt();
        assert!(sd > 0.08, "heightmap too flat: sd {sd}");

        // Silence collapses `normalise` to all-zeros, so a full 0..1 spread is
        // proof the field is genuinely varied rather than degenerate.
        let lo = spec.data.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = spec.data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(hi - lo > 0.9, "heightmap range is only {lo}..{hi}");
    }
}
