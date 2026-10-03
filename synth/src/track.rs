//! Shared album plumbing: what a rendered track looks like, the mixer every track uses,
//! master-bus helpers and the JSON metadata the dashboard reads.

use crate::dsp::*;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub const ARTIST: &str = "music-soundscape (Rust synth)";
const PEAKS: usize = 1200;

pub struct Section {
    pub name: &'static str,
    pub start: usize,
    pub end: usize,
    pub influence: &'static str,
    pub mood: &'static str,
    pub layers: &'static [&'static str],
}

/// A finished render plus everything the dashboard needs to describe it.
pub struct Track {
    pub slug: &'static str,
    pub title: &'static str,
    pub key: &'static str,
    pub bpm: f32,
    pub bars: usize,
    pub influences: &'static [&'static str],
    pub blurb: &'static str,
    pub sections: &'static [Section],
    /// (bar, chord name) at every chord change.
    pub chords: Vec<(usize, &'static str)>,
    pub audio: Stereo,
}

/// Index of the section containing `bar` (the last one once the arrangement is over).
pub fn section_at(sections: &[Section], bar: usize) -> usize {
    sections.iter().position(|s| bar >= s.start && bar < s.end).unwrap_or(sections.len() - 1)
}

// ---------------------------------------------------------------- mixing

/// Sums buses into the master with sends to a shared reverb and delay, ducking under the kick.
pub struct Mixer {
    pub tag: &'static str,
    pub master: Stereo,
    pub rev: Stereo,
    pub dly: Stereo,
    pub duck: Vec<f32>,
}

impl Mixer {
    pub fn new(tag: &'static str, len: usize, duck: Vec<f32>) -> Self {
        Self { tag, master: Stereo::new(len), rev: Stereo::new(len), dly: Stereo::new(len), duck }
    }

    pub fn add(&mut self, name: &str, mut bus: Stereo, level: f32, duck: f32, rev_send: f32, dly_send: f32) {
        if duck > 0.0 {
            apply_duck(&mut bus, &self.duck, duck);
        }
        eprintln!("  [{}] {name:<8} rms {:.3}  peak {:.3}", self.tag, bus.active_rms() * level, bus.peak() * level);
        self.master.mix_from(&bus, level);
        self.rev.mix_from(&bus, level * rev_send);
        self.dly.mix_from(&bus, level * dly_send);
    }
}

/// Master glue: DC/rumble cleanup, peak normalise, then tape-style saturation.
pub fn glue(master: &mut Stereo, drive: f32) {
    let (mut hl, mut hr) = (OnePole::new(25.0), OnePole::new(25.0));
    for i in 0..master.len() {
        master.l[i] = hl.hp(master.l[i]);
        master.r[i] = hr.hp(master.r[i]);
    }
    master.scale(1.0 / master.peak());
    for x in master.l.iter_mut().chain(master.r.iter_mut()) {
        *x = soft_clip(*x, drive);
    }
}

/// Final level (-1 dBFS peak) and a click-free 50 ms fade in.
pub fn finish(master: &mut Stereo) {
    master.scale(0.89 / master.peak());
    for n in 0..secs(0.05) {
        let g = n as f32 / secs(0.05) as f32;
        master.l[n] *= g;
        master.r[n] *= g;
    }
}

// ---------------------------------------------------------------- metadata

pub fn json_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn json_list(items: &[&str]) -> String {
    items.iter().map(|s| json_str(s)).collect::<Vec<_>>().join(", ")
}

pub fn duration(track: &Track) -> f32 {
    track.audio.len() as f32 / SR
}

pub fn write_metadata(path: &Path, track: &Track, number: usize, render_ms: u128) -> std::io::Result<()> {
    let bar_s = 240.0 / track.bpm;
    let song = &track.audio;
    let mut j = String::from("{\n");
    let _ = writeln!(j, "  \"slug\": {},", json_str(track.slug));
    let _ = writeln!(j, "  \"number\": {number},");
    let _ = writeln!(j, "  \"title\": {},", json_str(track.title));
    let _ = writeln!(j, "  \"artist\": {},", json_str(ARTIST));
    let _ = writeln!(j, "  \"influences\": [{}],", json_list(track.influences));
    let _ = writeln!(j, "  \"blurb\": {},", json_str(track.blurb));
    let _ = writeln!(j, "  \"bpm\": {},", track.bpm);
    let _ = writeln!(j, "  \"key\": {},", json_str(track.key));
    let _ = writeln!(j, "  \"sampleRate\": {},", SR as u32);
    let _ = writeln!(j, "  \"bars\": {},", track.bars);
    let _ = writeln!(j, "  \"duration\": {:.3},", duration(track));
    let _ = writeln!(j, "  \"renderMs\": {render_ms},");
    let _ = writeln!(j, "  \"audio\": {},", json_str(&format!("/audio/{}.wav", track.slug)));
    j.push_str("  \"sections\": [\n");
    for (i, s) in track.sections.iter().enumerate() {
        let _ = write!(
            j,
            "    {{\"name\": {}, \"start\": {:.3}, \"end\": {:.3}, \"startBar\": {}, \"endBar\": {}, \"influence\": {}, \"mood\": {}, \"layers\": [{}]}}{}\n",
            json_str(s.name), s.start as f32 * bar_s, s.end as f32 * bar_s, s.start, s.end,
            json_str(s.influence), json_str(s.mood), json_list(s.layers),
            if i + 1 < track.sections.len() { "," } else { "" }
        );
    }
    j.push_str("  ],\n  \"chords\": [");
    let chords: Vec<String> = track
        .chords
        .iter()
        .map(|&(b, name)| format!("{{\"time\": {:.3}, \"name\": {}}}", b as f32 * bar_s, json_str(name)))
        .collect();
    j.push_str(&chords.join(", "));
    j.push_str("],\n  \"peaks\": [");
    let chunk = song.len().div_ceil(PEAKS);
    let peaks: Vec<String> = (0..PEAKS)
        .map(|k| {
            let (a, b) = ((k * chunk).min(song.len()), ((k + 1) * chunk).min(song.len()));
            let p = (a..b).fold(0.0f32, |m, i| m.max(song.l[i].abs()).max(song.r[i].abs()));
            format!("{p:.3}")
        })
        .collect();
    j.push_str(&peaks.join(","));
    j.push_str("]\n}\n");
    fs::write(path, j)
}
