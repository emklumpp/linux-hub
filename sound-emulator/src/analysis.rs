//! Turning a recording into a spectrogram (the heightmap of our landscape).
//!
//! A spectrogram slices the audio into overlapping time frames and runs an FFT
//! on each, giving a 2D field: one axis is time, the other is frequency, and the
//! value is loudness. That is *already* a terrain — we just have to sample it
//! onto a regular grid.
//!
//! Two ways in:
//!   * [`compute_spectrogram`] analyses a whole clip at once (what `R`/`G`/`L` do)
//!   * [`FrameAnalyzer`] analyses one column at a time, which is what live mode
//!     needs — re-analysing the entire clip every frame would be far too slow.

use crate::audio::Recording;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;

/// A normalised height field. `data[row * cols + col]` is in 0.0..1.0, where
/// `col` walks forward in time and `row` walks up in frequency (row 0 = bass).
pub struct Spectrogram {
    pub cols: usize,
    pub rows: usize,
    pub data: Vec<f32>,
}

impl Spectrogram {
    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.data[row * self.cols + col]
    }
}

/// Analyses single frames, reusing its FFT plan, window and band edges across
/// calls. Build one and keep it — the setup is the expensive part.
pub struct FrameAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    /// Hann window, to reduce spectral leakage between frames.
    window: Vec<f32>,
    /// Log-spaced band edges: `edges[row]..edges[row + 1]` are the FFT bins that
    /// collapse into that row. Log-spaced because pitch is perceived that way,
    /// which keeps detail from crushing into the bass edge.
    edges: Vec<usize>,
    scratch: Vec<Complex<f32>>,
    fft_size: usize,
    rows: usize,
}

impl FrameAnalyzer {
    pub fn new(rows: usize, fft_size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);

        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let x = std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0);
                x.sin().powi(2)
            })
            .collect();

        // Skip bin 0 (DC) and start at bin 1.
        let half = fft_size / 2;
        let (min_bin, max_bin) = (1.0f32, half as f32);
        let edges: Vec<usize> = (0..=rows)
            .map(|r| {
                let t = r as f32 / rows as f32;
                (min_bin * (max_bin / min_bin).powf(t)).round() as usize
            })
            .collect();

        FrameAnalyzer {
            fft,
            window,
            edges,
            scratch: vec![Complex::new(0.0, 0.0); fft_size],
            fft_size,
            rows,
        }
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Analyse the frame beginning at `start`, returning one **un-normalised**
    /// dB value per row. `None` if the frame would run past the end of the clip.
    ///
    /// Values are raw so callers can choose how to map them to 0..1: a whole
    /// clip normalises against its own min/max, while live mode needs a range
    /// that changes smoothly or the terrain flickers every frame.
    pub fn column(&mut self, samples: &[f32], start: usize) -> Option<Vec<f32>> {
        if start + self.fft_size > samples.len() {
            return None;
        }

        for i in 0..self.fft_size {
            self.scratch[i] = Complex::new(samples[start + i] * self.window[i], 0.0);
        }
        self.fft.process(&mut self.scratch);

        let half = self.fft_size / 2;
        let mut out = Vec::with_capacity(self.rows);
        for row in 0..self.rows {
            let lo = self.edges[row].max(1);
            let hi = self.edges[row + 1].max(lo + 1).min(half);
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            for b in lo..hi {
                sum += self.scratch[b].norm();
                count += 1.0;
            }
            let mag = if count > 0.0 { sum / count } else { 0.0 };
            // Log/dB compression so quiet detail survives next to loud peaks.
            out.push((mag + 1e-6).log10());
        }
        Some(out)
    }
}

/// Build a `cols` x `rows` spectrogram from a whole recording.
///
/// - `fft_size` samples per frame (power of two works best; 2048 ≈ 46ms @ 44.1k).
/// - Frames are spread evenly across the whole clip so `cols` frames span it.
pub fn compute_spectrogram(
    rec: &Recording,
    cols: usize,
    rows: usize,
    fft_size: usize,
) -> Spectrogram {
    let n = rec.samples.len();
    let mut data = vec![0.0f32; cols * rows];

    // Guard: too little audio to analyse — hand back a flat field.
    if n < fft_size || cols == 0 || rows == 0 {
        return Spectrogram { cols, rows, data };
    }

    let mut analyzer = FrameAnalyzer::new(rows, fft_size);
    let last_start = n - fft_size;

    for col in 0..cols {
        // Where this frame begins in the sample buffer.
        let start = if cols == 1 {
            0
        } else {
            (col as f32 / (cols - 1) as f32 * last_start as f32) as usize
        };

        if let Some(column) = analyzer.column(&rec.samples, start) {
            for (row, db) in column.into_iter().enumerate() {
                data[row * cols + col] = db;
            }
        }
    }

    normalise(&mut data);
    Spectrogram { cols, rows, data }
}

/// Rescale a field to 0.0..1.0 based on its own min/max.
fn normalise(data: &mut [f32]) {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in data.iter() {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let range = (hi - lo).max(1e-6);
    for v in data.iter_mut() {
        *v = (*v - lo) / range;
    }
}
