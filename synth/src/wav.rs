//! Minimal 16-bit PCM stereo WAV writer with TPDF dither.

use crate::dsp::{Rng, Stereo, SR};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

pub fn write_wav16(path: &Path, buf: &Stereo) -> io::Result<()> {
    let frames = buf.len() as u32;
    let data_len = frames * 4;
    let mut w = BufWriter::new(File::create(path)?);
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_len).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&2u16.to_le_bytes())?; // channels
    w.write_all(&(SR as u32).to_le_bytes())?;
    w.write_all(&(SR as u32 * 4).to_le_bytes())?; // byte rate
    w.write_all(&4u16.to_le_bytes())?; // block align
    w.write_all(&16u16.to_le_bytes())?; // bits per sample
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;

    let mut rng = Rng::new(7);
    for i in 0..buf.len() {
        for x in [buf.l[i], buf.r[i]] {
            let dither = (rng.f() - rng.f()) / 32768.0;
            let s = ((x + dither).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            w.write_all(&s.to_le_bytes())?;
        }
    }
    w.flush()
}
