//! "Night Bus Hymn": Burial's crackling 2-step, rain and pitched-up ghost vocals, wrapped in
//! Boards of Canada's wobbling tape pads and a detuned music box.

use crate::dsp::*;
use crate::instruments::*;
use crate::track::*;
use crate::voices::Vowel::*;
use crate::voices::*;

const BPM: f32 = 138.0;
const BEAT: f32 = 60.0 / BPM;
const BARS: usize = 112;
const TAIL: f32 = 6.0;

fn tb(beats: f32) -> f32 {
    beats * BEAT
}

fn bar_at(t: f32) -> f32 {
    t / (4.0 * BEAT)
}

// ---------------------------------------------------------------- harmony

struct Chord {
    name: &'static str,
    bass: i32,
    pad: [i32; 4],
}

/// i – VI – iv – V(sus): two bars each, with the semitone rubs left in.
const PROG: [Chord; 4] = [
    Chord { name: "Dm9", bass: 38, pad: [57, 60, 64, 65] },
    Chord { name: "Bbmaj7#11", bass: 34, pad: [57, 62, 64, 65] },
    Chord { name: "Gm9", bass: 31, pad: [58, 62, 65, 69] },
    Chord { name: "A7sus4", bass: 33, pad: [57, 62, 64, 67] },
];

fn chord_at(bar: usize) -> &'static Chord {
    &PROG[(bar / 2) % 4]
}

/// The ghost hook over Dm → Bb (4 bars) ...
const HOOK_A: Phrase = &[
    (0.0, 1.5, 69.0, O, A), (1.5, 0.5, 72.0, A, A), (2.0, 2.5, 74.0, A, E),
    (6.0, 0.75, 72.0, E, I), (6.75, 1.25, 69.0, O, U),
    (8.0, 3.0, 77.0, A, O), (11.0, 1.0, 76.0, O, U), (12.0, 3.5, 74.0, U, A),
];

/// ... and its answer over Gm → A.
const HOOK_B: Phrase = &[
    (0.0, 1.5, 70.0, O, A), (1.5, 0.5, 74.0, A, A), (2.0, 2.5, 74.0, A, E),
    (6.0, 0.75, 72.0, E, I), (6.75, 1.25, 70.0, O, U),
    (8.0, 3.0, 76.0, A, O), (11.0, 1.0, 74.0, O, U), (12.0, 3.5, 69.0, U, A),
];

/// Music-box line, 8 bars.
const MUSIC_BOX: &[(f32, i32)] = &[
    (0.0, 74), (2.0, 72), (3.0, 69), (4.0, 72), (6.0, 69),
    (8.0, 70), (10.0, 69), (11.0, 65), (12.0, 69), (15.0, 70),
    (16.0, 74), (18.0, 70), (19.0, 67), (20.0, 70), (22.0, 74),
    (24.0, 76), (27.0, 74), (28.0, 69),
];

// ---------------------------------------------------------------- structure

const RAIN: usize = 0;
const SHUFFLE: usize = 1;
const GHOSTS: usize = 2;
const UNDERPASS: usize = 3;
const HYMN: usize = 4;
const LIGHTS: usize = 5;
const LAST: usize = 6;

static SECTIONS: [Section; 8] = [
    Section { name: "Rain on the Top Deck", start: 0, end: 8, influence: "Burial",
        mood: "Rain on the glass, vinyl crackle, and a pitched-up voice somewhere down the street.",
        layers: &["rain", "vinyl", "tape pad", "ghost vocal"] },
    Section { name: "Shuffle Home", start: 8, end: 24, influence: "Burial",
        mood: "Skippy 2-step with clanking snares and a sub that leans against the kick.",
        layers: &["2-step drums", "clank snare", "sub", "tape pad", "rain"] },
    Section { name: "Polaroid Ghosts", start: 24, end: 40, influence: "Burial × Boards of Canada",
        mood: "The ghost vocal finds its hook while a detuned music box drifts on worn tape.",
        layers: &["2-step drums", "sub", "ghost vocal", "music box", "tape pad"] },
    Section { name: "Underpass", start: 40, end: 48, influence: "Boards of Canada",
        mood: "The drums drop away; a reese breathes under open pads, the voice slowed to half speed.",
        layers: &["reese", "tape pad", "slowed vocal", "rain", "riser"] },
    Section { name: "Night Bus Hymn", start: 48, end: 72, influence: "Burial",
        mood: "Full 2-step: reese and sub, crackling snares, the hook doubled an octave up.",
        layers: &["2-step drums", "reese", "sub", "ghost vocal", "vocal shards", "tape pad"] },
    Section { name: "Lights Going Past", start: 72, end: 88, influence: "Boards of Canada × Burial",
        mood: "Swung harp arpeggios and the music box over a sparser, half-time beat.",
        layers: &["half-time drums", "sub", "harp", "music box", "tape pad", "ghost vocal"] },
    Section { name: "Last Stop", start: 88, end: 104, influence: "Burial",
        mood: "The beat thins out; vocal fragments echo down the empty deck.",
        layers: &["hats", "clank snare", "ghost vocal", "tape pad", "rain"] },
    Section { name: "Rain Again", start: 104, end: 112, influence: "Burial",
        mood: "Just the rain and the last of the pad.",
        layers: &["rain", "vinyl", "tape pad"] },
];

fn part_at(bar: usize) -> usize {
    section_at(&SECTIONS, bar)
}

// ---------------------------------------------------------------- automation (x = bars)

fn pad_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 300.0), (8.0, 900.0), (24.0, 1300.0), (40.0, 2400.0), (48.0, 1800.0),
        (72.0, 2200.0), (88.0, 1200.0), (104.0, 600.0), (112.0, 300.0), (116.0, 250.0)], bar_at(t))
}

fn tape_wow(t: f32) -> f32 {
    let depth = automation(&[(0.0, 0.007), (48.0, 0.004), (72.0, 0.006), (104.0, 0.009)], bar_at(t));
    let lfo = 0.6 * (std::f32::consts::TAU * 0.27 * t).sin()
        + 0.3 * (std::f32::consts::TAU * 0.71 * t + 0.9).sin()
        + 0.1 * (std::f32::consts::TAU * 4.3 * t + 0.2).sin();
    1.0 + depth * lfo
}

fn lofi_mix(t: f32) -> f32 {
    automation(&[(0.0, 0.6), (48.0, 0.35), (88.0, 0.5), (112.0, 0.8)], bar_at(t))
}

fn rain_level(t: f32) -> f32 {
    automation(&[(0.0, 1.0), (8.0, 0.55), (24.0, 0.35), (40.0, 0.8), (48.0, 0.3), (88.0, 0.6),
        (104.0, 1.0), (110.0, 1.0), (115.0, 0.0)], bar_at(t))
}

fn vinyl_level(t: f32) -> f32 {
    automation(&[(0.0, 0.9), (8.0, 0.5), (48.0, 0.35), (104.0, 0.9), (112.0, 1.0)], bar_at(t))
}

/// Offset (beats) of a swung sixteenth — 2-step swing is heavy.
fn swing16(step: usize) -> f32 {
    if step % 2 == 1 { 0.085 } else { 0.0 }
}

// ---------------------------------------------------------------- tracks

fn render_drums(len: usize, rng: &mut Rng) -> (Stereo, Vec<f32>) {
    let mut d = Stereo::new(len);
    let mut kicks = Vec::new();
    for b in 0..BARS {
        let part = part_at(b);
        let bb = (b * 4) as f32;
        let at = |s: usize, rng: &mut Rng| tb(bb + s as f32 * 0.25 + swing16(s)) + rng.bi() * 0.007;
        let (kick_steps, snare_steps, hat_density): (&[usize], &[usize], f32) = match part {
            SHUFFLE | GHOSTS | HYMN => (if b % 2 == 0 { &[0, 10] } else { &[0, 6, 10] }, &[4, 12], if part == HYMN { 0.55 } else { 0.4 }),
            LIGHTS => (&[0, 10], &[12], 0.3),
            LAST if b < 96 => (&[0], &[4, 12], 0.35),
            LAST => (&[], &[12], 0.25),
            _ => continue,
        };
        let fade = if part == LAST { 1.0 - (b - 88) as f32 / 18.0 } else { 1.0 };
        for &s in kick_steps {
            let t = at(s, rng);
            kick(&mut d, t, if s == 0 { 0.95 } else { 0.7 } * fade, false, rng);
            kicks.push(t);
        }
        for &s in snare_steps {
            let t = at(s, rng);
            clank(&mut d, t, 0.75 * fade, rng.range(480.0, 560.0), rng.range(-0.08, 0.08), rng);
        }
        // Ghost clanks that land just off the grid.
        if part != LAST && rng.chance(0.35) {
            let s = [7, 15, 9][(rng.f() * 3.0) as usize];
            clank(&mut d, at(s, rng), rng.range(0.15, 0.3), rng.range(600.0, 700.0), rng.range(-0.4, 0.4), rng);
        }
        for s in 0..16 {
            let offbeat = s % 4 == 2;
            if offbeat || rng.chance(hat_density) {
                let vel = if offbeat { 0.32 } else { rng.range(0.08, 0.2) };
                let open = part == HYMN && s == 14 && b % 2 == 1;
                hat(&mut d, at(s, rng), vel * fade, open, rng.range(0.1, 0.45), rng);
            }
            if (s == 3 || s == 11) && part != LAST && rng.chance(0.4) {
                rim(&mut d, at(s, rng), rng.range(0.15, 0.3), 1750.0, -0.5, rng);
            }
        }
    }
    riser(&mut d, tb(44.0 * 4.0), tb(16.0), 0.45, rng);
    impact(&mut d, tb(48.0 * 4.0), 0.6, rng);
    (d, kicks)
}

fn render_pads(len: usize, rng: &mut Rng) -> Stereo {
    const VEL: [f32; 8] = [0.8, 0.7, 0.75, 1.0, 0.85, 0.8, 0.75, 0.85];
    let mut p = Stereo::new(len);
    for b in (0..BARS).step_by(2) {
        let ch = chord_at(b);
        let dur = if b + 2 >= BARS { tb(8.0) + 3.0 } else { tb(8.0) + 0.1 };
        for &note in ch.pad.iter().chain(std::iter::once(&(ch.bass + 24))) {
            pad_note(&mut p, tb((b * 4) as f32), dur, note, VEL[part_at(b)], pad_cutoff, tape_wow, rng);
        }
    }
    p
}

fn render_bass(len: usize, rng: &mut Rng) -> Stereo {
    let mut out = Stereo::new(len);
    let mut prev = chord_at(0).bass as f32;
    for b in (0..BARS).step_by(2) {
        let part = part_at(b);
        let root = chord_at(b).bass as f32;
        let t0 = tb((b * 4) as f32);
        let len2 = tb(8.0) - 0.04;
        match part {
            SHUFFLE | GHOSTS | LIGHTS => sub(&mut out, t0, len2, root, prev, 0.12, 0.85),
            LAST if b < 96 => sub(&mut out, t0, len2, root, prev, 0.12, 0.6),
            UNDERPASS => reese(&mut out, t0, len2, root + 12.0, 0.6, 140.0, 650.0, 0.0, 1.6, rng),
            HYMN => {
                sub(&mut out, t0, len2, root, prev, 0.12, 0.75);
                reese(&mut out, t0, len2, root + 12.0, 0.5, 180.0, 900.0, 0.0, 2.0, rng);
            }
            _ => {}
        }
        prev = root;
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn sing_at(out: &mut Stereo, phrase: Phrase, bar: usize, transpose: f32, vel: f32, pan: f32, style: &VoxStyle, skip: f32, rng: &mut Rng) {
    sing(out, phrase, tb((bar * 4) as f32), BEAT, transpose, vel, pan, style, skip, rng);
}

fn render_vocals(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    let slowed = VoxStyle { formant: 0.92, drift: -0.3, breath: 0.6, ..GHOST };
    let shard = VoxStyle { formant: 1.45, release: 0.12, ..CHOP };
    let sinking = VoxStyle { drift: -1.5, ..GHOST };
    for b in (0..BARS).step_by(8) {
        match part_at(b) {
            RAIN => sing_at(&mut v, HOOK_A, b + 2, 0.0, 0.45, 0.2, &GHOST, 0.6, rng),
            SHUFFLE => sing_at(&mut v, HOOK_B, b + 4, 0.0, 0.4, -0.2, &GHOST, 0.7, rng),
            GHOSTS => {
                sing_at(&mut v, HOOK_A, b, 0.0, 0.7, 0.0, &GHOST, 0.12, rng);
                sing_at(&mut v, HOOK_B, b + 4, 0.0, 0.7, 0.0, &GHOST, 0.12, rng);
            }
            UNDERPASS => {
                sing_at(&mut v, HOOK_A, b, -12.0, 0.6, 0.0, &slowed, 0.0, rng);
                sing_at(&mut v, HOOK_B, b + 4, -12.0, 0.6, 0.0, &slowed, 0.0, rng);
            }
            HYMN => {
                sing_at(&mut v, HOOK_A, b, 0.0, 0.75, 0.0, &GHOST, 0.0, rng);
                sing_at(&mut v, HOOK_B, b + 4, 0.0, 0.75, 0.0, &GHOST, 0.0, rng);
                if b >= 56 {
                    sing_at(&mut v, HOOK_A, b, 12.0, 0.3, -0.35, &GHOST, 0.2, rng);
                    sing_at(&mut v, HOOK_B, b + 4, 12.0, 0.3, 0.35, &GHOST, 0.2, rng);
                }
                // Pitched-up shards on the last sixteenth of every other bar.
                for k in (1..8).step_by(2) {
                    let t = tb(((b + k) * 4) as f32 + 3.5 + 0.085);
                    vox(&mut v, t, tb(0.3), 81.0 + if k == 7 { 3.0 } else { 0.0 }, 0.45, rng.range(-0.6, 0.6), A, E, &shard, rng);
                }
            }
            LIGHTS => sing_at(&mut v, HOOK_B, b + 4, 0.0, 0.5, 0.25, &GHOST, 0.5, rng),
            LAST => {
                sing_at(&mut v, HOOK_A, b, 0.0, 0.55, -0.3, &sinking, 0.4, rng);
                sing_at(&mut v, HOOK_B, b + 4, 0.0, 0.45, 0.3, &sinking, 0.55, rng);
            }
            _ => {}
        }
    }
    v
}

fn render_keys(len: usize, rng: &mut Rng) -> Stereo {
    let mut k = Stereo::new(len);
    for b in (0..BARS).step_by(8) {
        let part = part_at(b);
        if (part == GHOSTS && b >= 32) || part == LIGHTS {
            for &(s, note) in MUSIC_BOX {
                kalimba(&mut k, tb((b * 4) as f32 + s) + rng.bi() * 0.01, note + 12, 0.45, rng.range(-0.3, 0.3), 2.2, rng);
            }
        }
    }
    // Dusty harp arpeggios through "Lights Going Past".
    const ARP: [usize; 8] = [0, 2, 1, 3, 2, 4, 3, 1];
    for b in 72..88 {
        let ch = chord_at(b);
        let tones = [ch.pad[0] + 12, ch.pad[1] + 12, ch.pad[2] + 12, ch.pad[3] + 12, ch.pad[0] + 24];
        for s in 0..8 {
            if rng.chance(0.2) {
                continue;
            }
            let swing = if s % 2 == 1 { 0.17 } else { 0.0 };
            let t = tb((b * 4) as f32 + s as f32 * 0.5 + swing) + rng.bi() * 0.008;
            pluck(&mut k, t, tones[ARP[s]], 0.3, if s % 2 == 0 { -0.45 } else { 0.45 }, 1.6, 2800.0, rng);
        }
    }
    k
}

// ---------------------------------------------------------------- render

pub fn render() -> Track {
    let song_end = tb((BARS * 4) as f32);
    let len = secs(song_end + TAIL);
    let mut rng = Rng::new(0x0034_3B05);

    let (mut drums, kicks) = render_drums(len, &mut rng);
    let mut mix = Mixer::new("nightbus", len, duck_envelope(len, &kicks, 0.6, 0.25));
    drums.normalize_rms(0.1);
    lofi(&mut drums, 11.0, 2, |_| 0.5);
    mix.add("drums", drums, 1.0, 0.0, 0.12, 0.03);

    let mut pads = render_pads(len, &mut rng);
    pads.normalize_rms(0.1);
    lofi(&mut pads, 10.0, 3, lofi_mix);
    mix.add("pads", pads, 0.8, 0.4, 0.4, 0.0);

    let mut bass = render_bass(len, &mut rng);
    bass.normalize_rms(0.1);
    mix.add("bass", bass, 0.85, 0.7, 0.0, 0.0);

    let mut vocals = render_vocals(len, &mut rng);
    vocals.normalize_rms(0.1);
    mix.add("vocals", vocals, 0.55, 0.15, 0.7, 0.35);

    let mut keys = render_keys(len, &mut rng);
    keys.normalize_rms(0.1);
    lofi(&mut keys, 11.0, 2, lofi_mix);
    mix.add("keys", keys, 0.45, 0.25, 0.35, 0.3);

    let mut wet = Stereo::new(len);
    rain(&mut wet, rain_level, &mut rng);
    wet.normalize_rms(0.1);
    mix.add("rain", wet, 0.3, 0.0, 0.08, 0.0);

    let Mixer { mut master, mut rev, dly, .. } = mix;
    let echoes = pingpong(&dly, tb(0.75), 0.5, 2800.0);
    master.mix_from(&echoes, 0.65);
    rev.mix_from(&echoes, 0.4);
    drop(dly);
    let hall = reverb(&rev, 1.5, 0.94, 0.45, 0.04);
    drop(rev);
    master.mix_from(&hall, 1.2);
    drop(hall);

    glue(&mut master, 1.8);
    vinyl(&mut master, vinyl_level, &mut rng);
    fade_out(&mut master, song_end, song_end + TAIL);
    finish(&mut master);

    Track {
        slug: "night-bus-hymn",
        title: "Night Bus Hymn",
        key: "D minor",
        bpm: BPM,
        bars: BARS,
        influences: &["Burial", "Boards of Canada"],
        blurb: "Rain-streaked 2-step on the top deck: clanking snares, ghost vocals, a music box on worn tape.",
        sections: &SECTIONS,
        chords: (0..BARS).step_by(2).map(|b| (b, chord_at(b).name)).collect(),
        audio: master,
    }
}
