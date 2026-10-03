//! "Last Train, Polaroid Light": the closer. A voice memo on an empty platform (Fred again..),
//! Burial's 2-step carrying it home, a Fred-style peak, and the "Northern Tape Memory" melody
//! returning on worn tape (Boards of Canada) before the cassette grinds to a stop again.

use crate::dsp::*;
use crate::instruments::*;
use crate::track::*;
use crate::voices::Vowel::*;
use crate::voices::*;

const BPM: f32 = 126.0;
const BEAT: f32 = 60.0 / BPM;
const BARS: usize = 88;
const TAIL: f32 = 7.0;

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
    riff: [f32; 5],
}

/// The opener's i – VI – III – VII, so the album ends where it began.
const PROG: [Chord; 4] = [
    Chord { name: "Fm9", bass: 41, pad: [56, 60, 63, 67], riff: [72.0, 72.0, 68.0, 67.0, 65.0] },
    Chord { name: "Dbmaj7", bass: 37, pad: [56, 60, 61, 65], riff: [72.0, 72.0, 68.0, 65.0, 68.0] },
    Chord { name: "Abmaj7", bass: 44, pad: [55, 60, 63, 68], riff: [72.0, 72.0, 70.0, 68.0, 67.0] },
    Chord { name: "Eb6", bass: 39, pad: [55, 58, 60, 63], riff: [70.0, 70.0, 67.0, 65.0, 63.0] },
];

fn chord_at(bar: usize) -> &'static Chord {
    &PROG[(bar / 2) % 4]
}

/// "Northern Tape Memory"'s melody A, quoted note for note.
const MELODY_A: &[(f32, f32, i32)] = &[
    (0.0, 1.5, 72), (1.5, 0.5, 68), (2.0, 1.0, 67), (3.0, 1.0, 68),
    (4.0, 2.0, 72), (6.0, 1.5, 75), (7.5, 0.5, 73),
    (8.0, 3.0, 72), (11.0, 1.0, 68), (12.0, 2.0, 65), (14.0, 1.0, 68), (15.0, 1.0, 72),
    (16.0, 1.5, 75), (17.5, 0.5, 72), (18.0, 1.0, 70), (19.0, 1.0, 72),
    (20.0, 3.0, 67), (23.0, 0.5, 68), (23.5, 0.5, 70),
    (24.0, 2.0, 67), (26.0, 1.0, 70), (27.0, 1.0, 72),
    (28.0, 1.5, 70), (29.5, 0.5, 67), (30.0, 2.0, 65),
];

/// The voice memo (8 bars).
const LINE: Phrase = &[
    (0.0, 2.0, 68.0, O, A), (2.0, 1.0, 67.0, A, A), (3.0, 1.0, 65.0, A, E), (4.0, 4.0, 72.0, E, A),
    (8.0, 3.0, 68.0, A, O), (11.0, 1.0, 70.0, O, O), (12.0, 4.0, 65.0, O, U),
    (16.0, 2.0, 67.0, A, A), (18.0, 2.0, 68.0, A, E), (20.0, 4.0, 72.0, E, A),
    (24.0, 3.0, 70.0, A, O), (27.0, 1.0, 67.0, O, O), (28.0, 4.0, 63.0, O, U),
];

const RIFF: &[(usize, usize, usize, Vowel, Vowel)] = &[
    (0, 3, 0, A, I), (4, 1, 1, I, I), (6, 2, 2, O, A), (10, 3, 3, A, O), (14, 2, 4, U, U),
];

// ---------------------------------------------------------------- structure

const PLATFORM: usize = 0;
const MEMO: usize = 1;
const TWOSTEP: usize = 2;
const REPRISE: usize = 3;
const HOLD: usize = 4;
const DOORS: usize = 5;
const POLAROID: usize = 6;

static SECTIONS: [Section; 7] = [
    Section { name: "Empty Platform", start: 0, end: 8, influence: "Burial",
        mood: "Rain on the canopy, crackle, a piano heard from the other end of the station.",
        layers: &["rain", "vinyl", "distant piano"] },
    Section { name: "Voice Memo", start: 8, end: 24, influence: "Fred again..",
        mood: "The piano comes close and someone sings into their phone, half to themselves.",
        layers: &["felt piano", "sung vocal", "rain"] },
    Section { name: "Carry Me Home", start: 24, end: 40, influence: "Burial",
        mood: "Clanking 2-step and a deep sub pick it up; the same voice comes back as a ghost.",
        layers: &["2-step drums", "sub", "ghost vocal", "felt piano"] },
    Section { name: "Northern Tape (Reprise)", start: 40, end: 56, influence: "Boards of Canada",
        mood: "The album's first melody returns on a music box and a warped lead, pads wobbling on old tape.",
        layers: &["music box", "tape lead", "tape pad", "2-step drums", "sub"] },
    Section { name: "Hold On To It", start: 56, end: 72, influence: "Fred again.. × Burial",
        mood: "The peak: a four-to-the-floor kick under the 2-step, pumping supersaws, the voice chopped into a riff.",
        layers: &["kick", "2-step drums", "supersaw", "vocal chops", "sub", "ghost vocal", "felt piano"] },
    Section { name: "Doors Closing", start: 72, end: 80, influence: "Burial",
        mood: "The beat stops. Piano, rain and the ghost of the voice.",
        layers: &["felt piano", "ghost vocal", "rain", "tape pad"] },
    Section { name: "Polaroid Light", start: 80, end: 88, influence: "Boards of Canada",
        mood: "One last pass of the melody on the piano, then the tape runs out — same as it started.",
        layers: &["felt piano", "tape pad", "vinyl", "tape stop"] },
];

fn part_at(bar: usize) -> usize {
    section_at(&SECTIONS, bar)
}

// ---------------------------------------------------------------- automation (x = bars)

/// The piano starts at the far end of the station and walks up to the mic.
fn piano_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 450.0), (7.0, 700.0), (8.0, 9000.0), (88.0, 9000.0), (92.0, 2000.0)], bar_at(t))
}

fn pad_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 300.0), (40.0, 900.0), (44.0, 2400.0), (56.0, 2600.0), (72.0, 1500.0), (80.0, 1800.0),
        (88.0, 500.0), (92.0, 300.0)], bar_at(t))
}

fn tape_wow(t: f32) -> f32 {
    let depth = automation(&[(0.0, 0.004), (40.0, 0.007), (56.0, 0.002), (72.0, 0.005), (80.0, 0.008), (92.0, 0.012)], bar_at(t));
    let lfo = 0.6 * (std::f32::consts::TAU * 0.31 * t).sin()
        + 0.3 * (std::f32::consts::TAU * 0.83 * t + 1.1).sin()
        + 0.1 * (std::f32::consts::TAU * 4.7 * t + 0.4).sin();
    1.0 + depth * lfo
}

fn lofi_mix(t: f32) -> f32 {
    automation(&[(0.0, 0.7), (8.0, 0.3), (40.0, 0.55), (56.0, 0.15), (72.0, 0.4), (80.0, 0.6), (92.0, 0.9)], bar_at(t))
}

fn rain_level(t: f32) -> f32 {
    automation(&[(0.0, 1.0), (8.0, 0.6), (24.0, 0.3), (56.0, 0.15), (72.0, 0.8), (80.0, 0.5), (88.0, 0.4)], bar_at(t))
}

fn vinyl_level(t: f32) -> f32 {
    automation(&[(0.0, 0.9), (8.0, 0.4), (40.0, 0.6), (56.0, 0.25), (72.0, 0.6), (80.0, 0.9), (92.0, 1.0)], bar_at(t))
}

fn swing16(step: usize) -> f32 {
    if step % 2 == 1 { 0.08 } else { 0.0 }
}

// ---------------------------------------------------------------- tracks

fn render_drums(len: usize, rng: &mut Rng) -> (Stereo, Vec<f32>) {
    let mut d = Stereo::new(len);
    let mut kicks = Vec::new();
    for b in 0..BARS {
        let part = part_at(b);
        if !matches!(part, TWOSTEP | REPRISE | HOLD) {
            continue;
        }
        let bb = (b * 4) as f32;
        let at = |s: usize, rng: &mut Rng| tb(bb + s as f32 * 0.25 + swing16(s)) + rng.bi() * 0.006;
        if part == HOLD {
            for q in 0..4 {
                let t = tb(bb + q as f32);
                techno_kick(&mut d, t, 0.9, 0.24, rng);
                kicks.push(t);
            }
        } else {
            let steps: &[usize] = if b % 2 == 0 { &[0, 10] } else { &[0, 6, 10] };
            for &s in steps {
                let t = at(s, rng);
                kick(&mut d, t, if s == 0 { 0.95 } else { 0.7 }, false, rng);
                kicks.push(t);
            }
        }
        for s in [4, 12] {
            clank(&mut d, at(s, rng), if part == REPRISE { 0.6 } else { 0.8 }, rng.range(470.0, 540.0), 0.0, rng);
        }
        if part != REPRISE && rng.chance(0.35) {
            let s = [7, 15][(rng.f() * 2.0) as usize];
            clank(&mut d, at(s, rng), rng.range(0.15, 0.28), rng.range(600.0, 700.0), rng.range(-0.4, 0.4), rng);
        }
        let density = if part == REPRISE { 0.2 } else { 0.45 };
        for s in 0..16 {
            let offbeat = s % 4 == 2;
            if offbeat || rng.chance(density) {
                hat(&mut d, at(s, rng), if offbeat { 0.3 } else { rng.range(0.07, 0.18) }, part == HOLD && offbeat, rng.range(0.0, 0.4), rng);
            }
        }
        if part == HOLD {
            clap(&mut d, tb(bb + 1.0), 0.3, rng);
            clap(&mut d, tb(bb + 3.0), 0.3, rng);
        }
    }
    riser(&mut d, tb(52.0 * 4.0), tb(16.0), 0.45, rng);
    impact(&mut d, tb(56.0 * 4.0), 0.7, rng);
    (d, kicks)
}

fn render_piano(len: usize, rng: &mut Rng) -> Stereo {
    let mut p = Stereo::new(len);
    for b in (0..BARS).step_by(2) {
        let part = part_at(b);
        if part == REPRISE {
            continue; // the music box has the floor
        }
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        let vel = match part {
            PLATFORM => 0.45,
            HOLD => 0.35,
            _ => 0.55,
        };
        let hits: &[(f32, f32)] = if part == POLAROID { &[(0.0, 7.8)] } else { &[(0.0, 2.8), (3.0, 0.9), (4.5, 3.3)] };
        for &(at, dur) in hits {
            let t = tb(bb + at) + rng.bi() * 0.006;
            piano(&mut p, t, tb(dur), ch.bass, vel * 0.8, -0.2, rng);
            for (k, &note) in ch.pad.iter().enumerate() {
                piano(&mut p, t + k as f32 * 0.012, tb(dur), note, vel * if at == 3.0 { 0.55 } else { 1.0 }, -0.1 + k as f32 * 0.08, rng);
            }
        }
    }
    // The opener's melody, one last time, on the piano.
    for &(s, d, note) in MELODY_A {
        piano(&mut p, tb(80.0 * 4.0 + s) + rng.bi() * 0.01, tb(d) + 0.2, note, 0.6, 0.2, rng);
    }
    p
}

fn render_vocals(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    let close = VoxStyle { breath: 0.75, ..SUNG };
    for b in (0..BARS).step_by(8) {
        let t = tb((b * 4) as f32);
        match part_at(b) {
            MEMO => sing(&mut v, LINE, t, BEAT, 0.0, 0.7, 0.0, &close, 0.0, rng),
            TWOSTEP => sing(&mut v, LINE, t, BEAT, 0.0, 0.65, 0.0, &GHOST, 0.1, rng),
            HOLD if b >= 64 => sing(&mut v, LINE, t, BEAT, 12.0, 0.3, 0.0, &GHOST, 0.3, rng),
            DOORS => sing(&mut v, LINE, t, BEAT, 0.0, 0.55, 0.0, &VoxStyle { drift: -1.0, ..GHOST }, 0.3, rng),
            _ => {}
        }
    }
    v
}

fn render_chops(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    for b in 56..72 {
        if b < 60 && b % 2 == 0 {
            continue;
        }
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        for &(s, n, k, va, vb) in RIFF {
            vox(&mut v, tb(bb + s as f32 * 0.25 + swing16(s)), tb(n as f32 * 0.25) * 0.9, ch.riff[k], 0.8, 0.0, va, vb, &CHOP, rng);
        }
    }
    v
}

fn render_tape(len: usize, rng: &mut Rng) -> Stereo {
    let mut t = Stereo::new(len);
    for b in (0..BARS).step_by(2) {
        let part = part_at(b);
        if !matches!(part, REPRISE | HOLD | DOORS | POLAROID) {
            continue;
        }
        let ch = chord_at(b);
        let dur = if b + 2 >= BARS { tb(8.0) + 2.5 } else { tb(8.0) + 0.1 };
        for &note in ch.pad.iter().chain(std::iter::once(&(ch.bass + 12))) {
            pad_note(&mut t, tb((b * 4) as f32), dur, note, 0.85, pad_cutoff, tape_wow, rng);
        }
    }
    for bar in [40, 48] {
        for &(s, d, note) in MELODY_A {
            let at = tb((bar * 4) as f32 + s);
            kalimba(&mut t, at + rng.bi() * 0.01, note + 12, 0.5, -0.25, 2.0, rng);
            if bar == 48 {
                lead_note(&mut t, at, tb(d), note, 0.35, 1100.0, 0.2, tape_wow);
            }
        }
    }
    t
}

fn render_synths(len: usize, rng: &mut Rng) -> Stereo {
    let mut s = Stereo::new(len);
    for b in 56..72 {
        let ch = chord_at(b);
        let notes = [ch.pad[0] + 12, ch.pad[1] + 12, ch.pad[2] + 12, ch.pad[3] + 12, ch.bass + 24];
        supersaw(&mut s, tb((b * 4) as f32), tb(4.0) - 0.02, &notes, 0.9, if b < 64 { 3000.0 } else { 4500.0 }, 0.015, rng);
    }
    s
}

fn render_bass(len: usize) -> Stereo {
    let mut out = Stereo::new(len);
    let mut prev = chord_at(0).bass as f32 - 12.0;
    for b in (0..BARS).step_by(2) {
        let part = part_at(b);
        let root = chord_at(b).bass as f32 - 12.0;
        if matches!(part, TWOSTEP | REPRISE | HOLD) {
            sub(&mut out, tb((b * 4) as f32), tb(8.0) - 0.03, root, prev, 0.12, if part == REPRISE { 0.65 } else { 0.85 });
        }
        prev = root;
    }
    out
}

// ---------------------------------------------------------------- render

pub fn render() -> Track {
    let song_end = tb((BARS * 4) as f32);
    let len = secs(song_end + TAIL);
    let mut rng = Rng::new(0x01A5_77A1);

    let (mut drums, kicks) = render_drums(len, &mut rng);
    let mut mix = Mixer::new("lasttrain", len, duck_envelope(len, &kicks, 0.75, 0.28));
    drums.normalize_rms(0.1);
    lofi(&mut drums, 11.0, 2, |_| 0.45);
    mix.add("drums", drums, 1.0, 0.0, 0.1, 0.02);

    let mut keys = render_piano(len, &mut rng);
    filter_bus(&mut keys, piano_cutoff, 0.7, false);
    keys.normalize_rms(0.1);
    lofi(&mut keys, 12.0, 2, lofi_mix);
    mix.add("piano", keys, 0.6, 0.35, 0.35, 0.08);

    let mut vocals = render_vocals(len, &mut rng);
    vocals.normalize_rms(0.1);
    mix.add("vocals", vocals, 0.55, 0.1, 0.55, 0.28);

    let mut chops = render_chops(len, &mut rng);
    chops.normalize_rms(0.1);
    mix.add("chops", chops, 0.45, 0.25, 0.15, 0.22);

    let mut tape = render_tape(len, &mut rng);
    tape.normalize_rms(0.1);
    lofi(&mut tape, 10.0, 3, lofi_mix);
    mix.add("tape", tape, 0.7, 0.4, 0.35, 0.15);

    let mut synths = render_synths(len, &mut rng);
    synths.normalize_rms(0.1);
    mix.add("supersaw", synths, 0.5, 0.9, 0.25, 0.0);

    let mut bass = render_bass(len);
    bass.normalize_rms(0.1);
    mix.add("bass", bass, 0.85, 0.7, 0.0, 0.0);

    let mut wet = Stereo::new(len);
    rain(&mut wet, rain_level, &mut rng);
    wet.normalize_rms(0.1);
    mix.add("rain", wet, 0.3, 0.0, 0.08, 0.0);

    let Mixer { mut master, mut rev, dly, .. } = mix;
    let echoes = pingpong(&dly, tb(0.75), 0.48, 3000.0);
    master.mix_from(&echoes, 0.65);
    rev.mix_from(&echoes, 0.35);
    drop(dly);
    let hall = reverb(&rev, 1.4, 0.93, 0.4, 0.03);
    drop(rev);
    master.mix_from(&hall, 1.2);
    drop(hall);

    glue(&mut master, 1.7);
    vinyl(&mut master, vinyl_level, &mut rng);
    tape_stop(&mut master, secs(song_end + 3.0), secs(2.6));
    finish(&mut master);

    Track {
        slug: "last-train-polaroid-light",
        title: "Last Train, Polaroid Light",
        key: "F minor",
        bpm: BPM,
        bars: BARS,
        influences: &["Burial", "Fred again..", "Boards of Canada"],
        blurb: "The closer: a voice memo on an empty platform, 2-step carrying it home, and the opening melody on worn tape.",
        sections: &SECTIONS,
        chords: (0..BARS).step_by(2).map(|b| (b, chord_at(b).name)).collect(),
        audio: master,
    }
}
