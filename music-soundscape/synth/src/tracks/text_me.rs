//! "Text Me When You're Home": Fred again..'s voice-memo intimacy — felt piano, a sung line,
//! then the same voice chopped into a riff over pumping supersaws — with Four Tet's shakers,
//! wooden clicks and kalimba threaded through the groove.

use crate::dsp::*;
use crate::instruments::*;
use crate::track::*;
use crate::voices::Vowel::*;
use crate::voices::*;

const BPM: f32 = 132.0;
const BEAT: f32 = 60.0 / BPM;
const BARS: usize = 104;
const TAIL: f32 = 5.0;

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
    /// Notes the chopped vocal riff walks through over this chord.
    riff: [f32; 6],
}

/// IV – I – V – vi in A♭, one bar each: the bittersweet loop.
const PROG: [Chord; 4] = [
    Chord { name: "Dbadd9", bass: 37, pad: [53, 56, 61, 63], riff: [68.0, 68.0, 70.0, 72.0, 70.0, 68.0] },
    Chord { name: "Abmaj7", bass: 32, pad: [56, 60, 63, 67], riff: [72.0, 72.0, 75.0, 72.0, 70.0, 68.0] },
    Chord { name: "Eb(add9)", bass: 39, pad: [55, 58, 63, 65], riff: [70.0, 70.0, 72.0, 75.0, 72.0, 70.0] },
    Chord { name: "Fm7", bass: 41, pad: [56, 60, 63, 65], riff: [68.0, 72.0, 75.0, 77.0, 75.0, 72.0] },
];

fn chord_at(bar: usize) -> &'static Chord {
    &PROG[bar % 4]
}

/// The chopped riff: (sixteenth step, length in steps, index into the chord's riff notes, vowels).
const RIFF: &[(usize, usize, usize, Vowel, Vowel)] = &[
    (0, 2, 0, A, O), (3, 1, 1, O, O), (6, 2, 2, A, I), (8, 1, 3, I, A), (10, 3, 4, O, U), (14, 1, 5, A, A),
];

/// The sung line the riff is cut from (4 bars).
const LINE: Phrase = &[
    (0.0, 1.5, 72.0, O, A), (1.5, 0.5, 70.0, A, A), (2.0, 2.0, 68.0, A, E),
    (4.0, 3.0, 72.0, A, O), (7.0, 1.0, 75.0, O, A),
    (8.0, 2.0, 75.0, A, E), (10.0, 1.0, 72.0, E, I), (11.0, 1.0, 70.0, I, A),
    (12.0, 4.0, 68.0, A, O),
];

// ---------------------------------------------------------------- structure

const VOICE: usize = 0;
const MORNING: usize = 1;
const CHOP_IT: usize = 2;
const TELL: usize = 3;
const DROP1: usize = 4;
const KEYS: usize = 5;
const DROP2: usize = 6;
const HOME: usize = 7;

static SECTIONS: [Section; 8] = [
    Section { name: "Voice Memo", start: 0, end: 8, influence: "Fred again..",
        mood: "Felt piano and a half-whispered line, recorded too close to the phone.",
        layers: &["felt piano", "sung vocal", "room tone"] },
    Section { name: "Four Tet Morning", start: 8, end: 24, influence: "Four Tet",
        mood: "Shakers and wooden clicks wake up; a kick creeps in from behind a closed door.",
        layers: &["felt piano", "shaker", "rims", "filtered kick", "kalimba"] },
    Section { name: "Chop It Up", start: 24, end: 40, influence: "Fred again..",
        mood: "The voice is cut into a riff; offbeat bass and a shuffling house groove.",
        layers: &["vocal chops", "kick", "clap", "hats", "shaker", "offbeat bass", "felt piano"] },
    Section { name: "Tell Me Again", start: 40, end: 48, influence: "Fred again..",
        mood: "Everything holds its breath: the sung line returns as supersaws swell and the claps roll.",
        layers: &["sung vocal", "felt piano", "supersaw swell", "clap roll", "riser"] },
    Section { name: "Text Me When You're Home", start: 48, end: 72, influence: "Fred again..",
        mood: "The drop: pumping supersaw chords, the chopped voice as the lead, sub on every change.",
        layers: &["supersaw", "vocal chops", "kick", "clap", "open hats", "sub", "offbeat bass", "shaker"] },
    Section { name: "Keys in the Door", start: 72, end: 80, influence: "Fred again.. × Four Tet",
        mood: "Muffled kick, a stuttering voice and the piano, like hearing the party from the hallway.",
        layers: &["filtered kick", "vocal stutter", "felt piano", "sub", "kalimba"] },
    Section { name: "Still Awake", start: 80, end: 96, influence: "Fred again.. × Four Tet",
        mood: "Second drop: chops doubled an octave up, kalimba and clicks scattered across it.",
        layers: &["supersaw", "vocal chops", "kalimba", "rims", "kick", "clap", "open hats", "sub"] },
    Section { name: "Home", start: 96, end: 104, influence: "Fred again..",
        mood: "Just the piano and the voice again. You made it.",
        layers: &["felt piano", "sung vocal", "room tone"] },
];

fn part_at(bar: usize) -> usize {
    section_at(&SECTIONS, bar)
}

// ---------------------------------------------------------------- automation (x = bars)

/// The drum bus opens up out of the "next room" and closes again for the hallway break.
fn drum_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 260.0), (8.0, 260.0), (23.5, 4500.0), (24.0, 18000.0), (72.0, 18000.0), (72.05, 650.0),
        (79.5, 5000.0), (80.0, 18000.0)], bar_at(t))
}

fn room_tone(t: f32) -> f32 {
    automation(&[(0.0, 0.7), (8.0, 0.35), (24.0, 0.1), (96.0, 0.15), (100.0, 0.6), (106.0, 0.8)], bar_at(t))
}

fn piano_lofi(t: f32) -> f32 {
    automation(&[(0.0, 0.55), (8.0, 0.3), (24.0, 0.1), (96.0, 0.2), (104.0, 0.6)], bar_at(t))
}

fn swing16(step: usize, amount: f32) -> f32 {
    if step % 2 == 1 { amount * 0.25 } else { 0.0 }
}

// ---------------------------------------------------------------- tracks

fn render_drums(len: usize, rng: &mut Rng) -> (Stereo, Vec<f32>) {
    let mut d = Stereo::new(len);
    let mut kicks = Vec::new();
    for b in 0..BARS {
        let part = part_at(b);
        let bb = (b * 4) as f32;
        let h = |rng: &mut Rng| rng.bi() * 0.004;
        let four_floor = matches!(part, MORNING | CHOP_IT | DROP1 | KEYS | DROP2) && !(part == MORNING && b < 12);
        if four_floor {
            for q in 0..4 {
                let t = tb(bb + q as f32);
                let vel = match part {
                    MORNING => 0.45 + 0.4 * (b - 12) as f32 / 12.0,
                    KEYS => 0.75,
                    _ => 1.0,
                };
                techno_kick(&mut d, t, vel, 0.22, rng);
                kicks.push(t);
            }
        }
        let shakers = matches!(part, MORNING | CHOP_IT | DROP1 | DROP2) || (part == HOME && b < 100);
        for s in 0..16 {
            let t = tb(bb + s as f32 * 0.25 + swing16(s, 0.28)) + h(rng);
            if shakers {
                let fade = if part == HOME { 1.0 - (b - 96) as f32 / 4.0 } else { 1.0 };
                shaker(&mut d, t, [0.5, 0.2, 0.32, 0.2][s % 4] * fade, -0.35, rng);
            }
            let clicky = match part {
                MORNING => 0.3,
                DROP2 => 0.28,
                CHOP_IT | DROP1 => 0.12,
                _ => 0.0,
            };
            if s % 4 != 0 && rng.chance(clicky) {
                let freq = [900.0, 1300.0, 1900.0, 2600.0][(rng.f() * 4.0) as usize];
                rim(&mut d, t, rng.range(0.2, 0.45), freq, rng.range(-0.8, 0.8), rng);
            }
            if matches!(part, CHOP_IT | DROP1 | DROP2) {
                hat(&mut d, t, [0.12, 0.06, 0.16, 0.06][s % 4], false, 0.25, rng);
            }
        }
        if matches!(part, CHOP_IT | DROP1 | DROP2) {
            clap(&mut d, tb(bb + 1.0) + h(rng), 0.6, rng);
            clap(&mut d, tb(bb + 3.0) + h(rng), 0.6, rng);
            if part != CHOP_IT || b >= 32 {
                for q in 0..4 {
                    hat(&mut d, tb(bb + q as f32 + 0.5), 0.3, true, 0.1, rng);
                }
            }
        }
        // The roll into the drop: eighths, then sixteenths, getting louder.
        if part == TELL && b >= 44 {
            let step = if b >= 46 { 0.25 } else { 0.5 };
            let n = (4.0 / step) as usize;
            for k in 0..n {
                let x = ((b - 44) as f32 * 4.0 + k as f32 * step) / 16.0;
                clap(&mut d, tb(bb + k as f32 * step), 0.15 + 0.55 * x, rng);
            }
        }
        if b == 79 {
            for k in 0..8 {
                clap(&mut d, tb(bb + 2.0 + k as f32 * 0.25), 0.2 + k as f32 * 0.06, rng);
            }
        }
    }
    riser(&mut d, tb(40.0 * 4.0), tb(32.0), 0.5, rng);
    riser(&mut d, tb(76.0 * 4.0), tb(16.0), 0.35, rng);
    impact(&mut d, tb(48.0 * 4.0), 0.75, rng);
    impact(&mut d, tb(80.0 * 4.0), 0.65, rng);
    (d, kicks)
}

fn render_piano(len: usize, rng: &mut Rng) -> Stereo {
    let mut p = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        let vel = match part {
            VOICE | HOME => 0.55,
            TELL | KEYS => 0.6,
            DROP1 | DROP2 => 0.4,
            _ => 0.5,
        };
        // Syncopated Fred-style comping: the chord, a push on the "and" of 2, a lighter top note on 4.
        let hits: &[(f32, f32, f32)] = if part == VOICE && b < 4 { &[(0.0, 3.8, 1.0)] } else { &[(0.0, 1.4, 1.0), (1.5, 2.3, 0.75)] };
        let last = b + 1 == BARS;
        for &(at, dur, v) in hits {
            let dur = if last { 6.0 } else { dur };
            let t = tb(bb + at) + rng.bi() * 0.006;
            piano(&mut p, t, tb(dur), ch.bass + 12, vel * v * 0.8, -0.2, rng);
            for (k, &note) in ch.pad.iter().enumerate() {
                // Roll the chord slightly, like real fingers.
                piano(&mut p, t + k as f32 * 0.008, tb(dur), note, vel * v, -0.1 + k as f32 * 0.1, rng);
            }
        }
        if !last && !matches!(part, DROP1 | DROP2) {
            let top = ch.pad[3] + 12;
            piano(&mut p, tb(bb + 3.0), tb(0.9), top, vel * 0.45, 0.35, rng);
        }
    }
    p
}

fn render_chops(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    let step = 0.25;
    for b in 0..BARS {
        let part = part_at(b);
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        let at = |s: usize| tb(bb + s as f32 * step + swing16(s, 0.18));
        match part {
            CHOP_IT | DROP1 | DROP2 => {
                if part == CHOP_IT && b < 28 && b % 2 == 0 {
                    continue; // introduce the riff half at a time
                }
                let stutter = part != CHOP_IT && b % 4 == 3;
                for &(s, n, k, va, vb) in RIFF {
                    if stutter && s >= 12 {
                        continue;
                    }
                    let pan = if part == DROP2 { rng.range(-0.25, 0.25) } else { 0.0 };
                    vox(&mut v, at(s), tb(n as f32 * step) * 0.92, ch.riff[k], 0.8, pan, va, vb, &CHOP, rng);
                    if part == DROP2 {
                        vox(&mut v, at(s), tb(n as f32 * step) * 0.92, ch.riff[k] + 12.0, 0.3, -pan, va, vb, &CHOP, rng);
                    }
                }
                if stutter {
                    for (k, m) in [75.0, 75.0, 77.0, 80.0].into_iter().enumerate() {
                        vox(&mut v, at(12 + k), tb(step * 0.8), m, 0.7, [-0.3, 0.3, -0.15, 0.15][k], A, A, &CHOP, rng);
                    }
                }
            }
            KEYS => {
                // A single syllable stuttering in faster and faster.
                let n = if b < 76 { 4 } else { 8 };
                for k in 0..n {
                    let t = tb(bb + k as f32 * 4.0 / n as f32);
                    let m = if b % 2 == 0 { 72.0 } else { 75.0 };
                    vox(&mut v, t, tb(4.0 / n as f32) * 0.6, m, 0.6, rng.range(-0.4, 0.4), A, O, &CHOP, rng);
                }
            }
            _ => {}
        }
    }
    v
}

fn render_voice(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    let close = VoxStyle { breath: 0.75, ..SUNG };
    for b in (0..BARS).step_by(4) {
        match part_at(b) {
            VOICE if b >= 4 => sing(&mut v, LINE, tb((b * 4) as f32), BEAT, 0.0, 0.7, 0.0, &close, 0.0, rng),
            TELL => sing(&mut v, LINE, tb((b * 4) as f32), BEAT, 0.0, 0.8, 0.0, &SUNG, 0.0, rng),
            HOME => sing(&mut v, LINE, tb((b * 4) as f32), BEAT, 0.0, 0.65, 0.0, &close, 0.0, rng),
            _ => {}
        }
    }
    v
}

fn render_synths(len: usize, rng: &mut Rng) -> Stereo {
    let mut s = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        let ch = chord_at(b);
        let notes = [ch.pad[0] + 12, ch.pad[1] + 12, ch.pad[2] + 12, ch.pad[3] + 12, ch.bass + 24];
        let t = tb((b * 4) as f32);
        match part {
            DROP1 => supersaw(&mut s, t, tb(4.0) - 0.02, &notes, 0.9, 3600.0, 0.015, rng),
            DROP2 => supersaw(&mut s, t, tb(4.0) - 0.02, &notes, 1.0, 5200.0, 0.015, rng),
            TELL if b >= 44 => {
                let x = (b - 44) as f32 / 4.0;
                supersaw(&mut s, t, tb(4.0), &notes, 0.4 + 0.5 * x, 700.0 + 2500.0 * x, tb(3.0), rng);
            }
            _ => {}
        }
    }
    s
}

fn render_bass(len: usize, rng: &mut Rng) -> Stereo {
    let mut out = Stereo::new(len);
    let mut prev = chord_at(0).bass as f32;
    for b in 0..BARS {
        let part = part_at(b);
        let root = chord_at(b).bass;
        let bb = (b * 4) as f32;
        if matches!(part, DROP1 | DROP2 | KEYS) {
            sub(&mut out, tb(bb), tb(4.0) - 0.02, root as f32, prev, 0.05, if part == KEYS { 0.6 } else { 0.85 });
        }
        if matches!(part, CHOP_IT | DROP1 | DROP2) {
            for q in 0..4 {
                let note = if q == 3 && b % 2 == 1 { root + 12 } else { root };
                bass_note(&mut out, tb(bb + q as f32 + 0.5) + rng.bi() * 0.003, tb(0.38), note + 12, 0.5, 1.8);
            }
        }
        prev = root as f32;
    }
    out
}

fn render_kalimba(len: usize, rng: &mut Rng) -> Stereo {
    const ARP: [usize; 16] = [0, 2, 1, 3, 2, 4, 3, 1, 0, 2, 4, 3, 1, 2, 3, 4];
    let mut k = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        if !matches!(part, MORNING | KEYS | DROP2) {
            continue;
        }
        let ch = chord_at(b);
        let tones = [ch.pad[0] + 12, ch.pad[1] + 12, ch.pad[2] + 12, ch.pad[3] + 12, ch.pad[1] + 24];
        for s in 0..16 {
            if rng.chance(if part == MORNING && b < 16 { 0.55 } else { 0.25 }) {
                continue;
            }
            let t = tb((b * 4) as f32 + s as f32 * 0.25 + swing16(s, 0.28)) + rng.bi() * 0.006;
            let pan = (s as f32 * 0.8 + b as f32).sin() * 0.65;
            kalimba(&mut k, t, tones[ARP[s]], if s % 4 == 0 { 0.4 } else { 0.28 }, pan, 1.3, rng);
        }
    }
    k
}

// ---------------------------------------------------------------- render

pub fn render() -> Track {
    let song_end = tb((BARS * 4) as f32);
    let len = secs(song_end + TAIL);
    let mut rng = Rng::new(0x7E57_0E);

    let (mut drums, kicks) = render_drums(len, &mut rng);
    filter_bus(&mut drums, drum_cutoff, 0.9, false);
    let mut mix = Mixer::new("textme", len, duck_envelope(len, &kicks, 0.85, 0.28));
    drums.normalize_rms(0.1);
    mix.add("drums", drums, 1.05, 0.0, 0.06, 0.0);

    let mut keys = render_piano(len, &mut rng);
    keys.normalize_rms(0.1);
    lofi(&mut keys, 12.0, 2, piano_lofi);
    mix.add("piano", keys, 0.6, 0.5, 0.3, 0.08);

    let mut chops = render_chops(len, &mut rng);
    chops.normalize_rms(0.1);
    mix.add("chops", chops, 0.55, 0.25, 0.15, 0.18);

    let mut voice = render_voice(len, &mut rng);
    voice.normalize_rms(0.1);
    mix.add("voice", voice, 0.55, 0.0, 0.4, 0.22);

    let mut synths = render_synths(len, &mut rng);
    synths.normalize_rms(0.1);
    mix.add("supersaw", synths, 0.6, 0.9, 0.25, 0.0);

    let mut bass = render_bass(len, &mut rng);
    bass.normalize_rms(0.1);
    mix.add("bass", bass, 0.8, 0.85, 0.0, 0.0);

    let mut kal = render_kalimba(len, &mut rng);
    kal.normalize_rms(0.1);
    mix.add("kalimba", kal, 0.4, 0.3, 0.3, 0.3);

    let Mixer { mut master, mut rev, dly, .. } = mix;
    let echoes = pingpong(&dly, tb(0.75), 0.42, 3600.0);
    master.mix_from(&echoes, 0.6);
    rev.mix_from(&echoes, 0.3);
    drop(dly);
    let hall = reverb(&rev, 1.2, 0.9, 0.3, 0.02);
    drop(rev);
    master.mix_from(&hall, 1.1);
    drop(hall);

    glue(&mut master, 1.6);
    vinyl(&mut master, room_tone, &mut rng);
    fade_out(&mut master, song_end + 1.0, song_end + TAIL);
    finish(&mut master);

    Track {
        slug: "text-me-when-youre-home",
        title: "Text Me When You're Home",
        key: "A♭ major",
        bpm: BPM,
        bars: BARS,
        influences: &["Fred again..", "Four Tet"],
        blurb: "A voice memo becomes a riff: felt piano, chopped vocals and pumping supersaws, with Four Tet clicks in the cracks.",
        sections: &SECTIONS,
        chords: (0..BARS).map(|b| (b, chord_at(b).name)).collect(),
        audio: master,
    }
}
