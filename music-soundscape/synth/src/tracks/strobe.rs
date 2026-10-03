//! "Strobe Through Fog": Fred again..'s club side — rumbling techno kick, hypnotic stabs and a
//! stuttering vocal that breaks into a piano-and-voice confession — with Four Tet's percussion
//! scattered in the haze and a Ben Böhmer lead over the euphoric return.

use crate::dsp::*;
use crate::instruments::*;
use crate::track::*;
use crate::voices::Vowel::*;
use crate::voices::*;

const BPM: f32 = 130.0;
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
    /// Notes the vocal riff walks through over this chord.
    riff: [f32; 5],
}

/// The hypnotic one-chord techno sections rock between these two voicings of A minor.
const AM7: Chord = Chord { name: "Am7", bass: 33, pad: [57, 60, 64, 67], riff: [76.0, 76.0, 74.0, 72.0, 69.0] };
const AM9: Chord = Chord { name: "Am9", bass: 33, pad: [55, 59, 60, 64], riff: [76.0, 76.0, 74.0, 72.0, 71.0] };

/// VI – VII – i – v for the breakdown and the drop, two bars each.
const PROG: [Chord; 4] = [
    Chord { name: "Fmaj7", bass: 29, pad: [53, 57, 60, 64], riff: [76.0, 76.0, 74.0, 72.0, 69.0] },
    Chord { name: "G6", bass: 31, pad: [55, 59, 62, 64], riff: [74.0, 74.0, 72.0, 71.0, 67.0] },
    Chord { name: "Am7", bass: 33, pad: [57, 60, 64, 67], riff: [76.0, 76.0, 74.0, 72.0, 69.0] },
    Chord { name: "Em7", bass: 28, pad: [52, 55, 59, 62], riff: [74.0, 74.0, 71.0, 67.0, 64.0] },
];

/// The chopped riff: (sixteenth step, length in steps, riff index, vowels).
const RIFF: &[(usize, usize, usize, Vowel, Vowel)] = &[
    (0, 3, 0, A, I), (4, 1, 1, I, I), (6, 2, 2, O, A), (10, 3, 3, A, O), (14, 2, 4, U, U),
];

/// The confession in the breakdown (8 bars over VI – VII – i – v).
const LINE: Phrase = &[
    (0.0, 3.0, 69.0, O, A), (3.0, 1.0, 72.0, A, A), (4.0, 4.0, 72.0, A, E),
    (8.0, 3.0, 71.0, E, A), (11.0, 1.0, 74.0, A, A), (12.0, 4.0, 74.0, A, O),
    (16.0, 3.0, 76.0, O, A), (19.0, 1.0, 74.0, A, E), (20.0, 4.0, 72.0, E, I),
    (24.0, 4.0, 71.0, A, O), (28.0, 4.0, 67.0, O, U),
];

/// Böhmer counter-melody over the drop.
const LEAD: &[(f32, f32, i32)] = &[
    (0.0, 3.0, 76), (3.0, 1.0, 77), (4.0, 4.0, 72),
    (8.0, 3.0, 74), (11.0, 1.0, 79), (12.0, 4.0, 71),
    (16.0, 3.0, 76), (19.0, 1.0, 79), (20.0, 4.0, 81),
    (24.0, 4.0, 79), (28.0, 4.0, 76),
];

// ---------------------------------------------------------------- structure

const FOG: usize = 0;
const STROBE: usize = 1;
const HEADS: usize = 2;
const ARMS: usize = 3;
const LIGHTS_UP: usize = 4;
const AFTER: usize = 5;
const LIFTS: usize = 6;

static SECTIONS: [Section; 7] = [
    Section { name: "Fog Machine", start: 0, end: 16, influence: "Fred again..",
        mood: "Just the kick and its rumble filling the room, hats creeping in through the haze.",
        layers: &["kick", "rumble", "hats", "stab (closed)"] },
    Section { name: "Strobe", start: 16, end: 32, influence: "Fred again..",
        mood: "Hypnotic stabs open up over a rolling bassline; a single syllable stutters at the end of each bar.",
        layers: &["kick", "rumble", "stabs", "rolling bass", "clap", "hats", "vocal stutter"] },
    Section { name: "Heads Up", start: 32, end: 48, influence: "Fred again.. × Four Tet",
        mood: "The voice becomes the riff; shakers, wooden clicks and a ride scatter around it.",
        layers: &["kick", "rumble", "stabs", "rolling bass", "vocal chops", "shaker", "rims", "ride"] },
    Section { name: "Somebody's Arms", start: 48, end: 64, influence: "Fred again.. × Ben Böhmer",
        mood: "The kick drops out: felt piano, a sung line and pads, then supersaws swelling back up.",
        layers: &["felt piano", "sung vocal", "pad", "supersaw swell", "clap roll", "riser"] },
    Section { name: "Lights Up", start: 64, end: 88, influence: "Fred again.. × Ben Böhmer",
        mood: "Euphoric return: pumping supersaws, the chopped voice over new chords, a Böhmer lead on top.",
        layers: &["kick", "rumble", "supersaw", "vocal chops", "lead", "rolling bass", "pad", "hats", "shaker"] },
    Section { name: "Afterhours", start: 88, end: 104, influence: "Fred again..",
        mood: "Back to the bare techno as the stabs close and the stutter thins out.",
        layers: &["kick", "rumble", "stabs", "rolling bass", "vocal stutter", "hats"] },
    Section { name: "Fog Lifts", start: 104, end: 112, influence: "Ben Böhmer",
        mood: "Kick gone, the rumble ebbs away under one last pad and the voice.",
        layers: &["pad", "felt piano", "sung vocal", "rumble tail"] },
];

fn part_at(bar: usize) -> usize {
    section_at(&SECTIONS, bar)
}

fn chord_at(bar: usize) -> &'static Chord {
    match part_at(bar) {
        ARMS | LIGHTS_UP | LIFTS => &PROG[(bar / 2) % 4],
        _ => if (bar / 2) % 2 == 0 { &AM7 } else { &AM9 },
    }
}

// ---------------------------------------------------------------- automation (x = bars)

fn stab_bright(bar: f32) -> f32 {
    automation(&[(0.0, 150.0), (8.0, 200.0), (16.0, 600.0), (32.0, 2600.0), (48.0, 3800.0),
        (88.0, 3000.0), (104.0, 500.0)], bar)
}

fn pad_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 600.0), (48.0, 900.0), (56.0, 2600.0), (64.0, 3000.0), (88.0, 2000.0),
        (104.0, 1400.0), (112.0, 400.0)], bar_at(t))
}

fn no_wow(_: f32) -> f32 {
    1.0
}

fn swing16(step: usize) -> f32 {
    if step % 2 == 1 { 0.03 } else { 0.0 }
}

// ---------------------------------------------------------------- tracks

/// Returns (drums, kick-only bus for the rumble, kick times).
fn render_drums(len: usize, rng: &mut Rng) -> (Stereo, Stereo, Vec<f32>) {
    let mut d = Stereo::new(len);
    let mut kb = Stereo::new(len);
    let mut kicks = Vec::new();
    for b in 0..BARS {
        let part = part_at(b);
        let bb = (b * 4) as f32;
        let at = |s: usize, rng: &mut Rng| tb(bb + s as f32 * 0.25 + swing16(s)) + rng.bi() * 0.003;
        if !matches!(part, ARMS | LIFTS) {
            for q in 0..4 {
                let t = tb(bb + q as f32);
                techno_kick(&mut kb, t, 1.0, 0.3, rng);
                kicks.push(t);
            }
        }
        let offbeat_hats = match part {
            FOG => b >= 8,
            STROBE | HEADS | LIGHTS_UP => true,
            AFTER => b < 100,
            _ => false,
        };
        if offbeat_hats {
            for q in 0..4 {
                hat(&mut d, tb(bb + q as f32 + 0.5), if part == FOG { 0.18 } else { 0.3 }, true, 0.15, rng);
            }
        }
        let sixteenths = matches!(part, STROBE | HEADS | LIGHTS_UP) || (part == AFTER && b < 96);
        for s in 0..16 {
            if sixteenths {
                hat(&mut d, at(s, rng), [0.13, 0.06, 0.1, 0.06][s % 4], false, -0.25, rng);
            }
            if matches!(part, HEADS | LIGHTS_UP) {
                shaker(&mut d, at(s, rng), [0.4, 0.15, 0.25, 0.15][s % 4], 0.4, rng);
            }
            let clicks = match part {
                HEADS => 0.25,
                LIGHTS_UP => 0.12,
                STROBE => 0.06,
                _ => 0.0,
            };
            if s % 4 != 0 && rng.chance(clicks) {
                rim(&mut d, at(s, rng), rng.range(0.2, 0.4), rng.range(800.0, 2600.0), rng.range(-0.85, 0.85), rng);
            }
        }
        // A ride on the eighths once things are moving.
        if part == HEADS && b >= 40 || part == LIGHTS_UP {
            for e in 0..8 {
                hat(&mut d, tb(bb + e as f32 * 0.5), 0.08, true, 0.5, rng);
            }
        }
        let claps = (part == STROBE && b >= 20) || matches!(part, HEADS | LIGHTS_UP) || (part == AFTER && b < 100);
        if claps {
            clap(&mut d, tb(bb + 1.0), 0.55, rng);
            clap(&mut d, tb(bb + 3.0), 0.55, rng);
        }
        if part == ARMS && b >= 60 {
            let step = if b >= 62 { 0.25 } else { 0.5 };
            for k in 0..(4.0 / step) as usize {
                let x = ((b - 60) as f32 * 4.0 + k as f32 * step) / 16.0;
                clap(&mut d, tb(bb + k as f32 * step), 0.12 + 0.6 * x, rng);
            }
        }
    }
    riser(&mut d, tb(56.0 * 4.0), tb(32.0), 0.5, rng);
    riser(&mut d, tb(30.0 * 4.0), tb(8.0), 0.25, rng);
    impact(&mut d, tb(64.0 * 4.0), 0.8, rng);
    impact(&mut d, tb(32.0 * 4.0), 0.4, rng);
    (d, kb, kicks)
}

fn render_stabs(len: usize, rng: &mut Rng) -> Stereo {
    const STEPS: [usize; 4] = [3, 6, 10, 13];
    let mut s = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        if matches!(part, ARMS | LIFTS) || (part == FOG && b < 8) {
            continue;
        }
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        for (k, &st) in STEPS.iter().enumerate() {
            if part == LIGHTS_UP && k % 2 == 1 {
                continue;
            }
            let bright = stab_bright(b as f32 + st as f32 / 16.0) * if k == 2 { 1.3 } else { 1.0 };
            let vel = if part == LIGHTS_UP { 0.5 } else { 0.9 };
            stab(&mut s, tb(bb + st as f32 * 0.25), &ch.pad, vel, bright, 0.13, [-0.3, 0.25, 0.0, 0.35][k], rng);
        }
    }
    s
}

fn render_bass(len: usize, rng: &mut Rng) -> Stereo {
    let mut out = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        if !matches!(part, STROBE | HEADS | LIGHTS_UP | AFTER) || (part == AFTER && b >= 102) {
            continue;
        }
        let root = chord_at(b).bass;
        let bb = (b * 4) as f32;
        for q in 0..4 {
            for s in 1..4 {
                let note = if s == 3 && q % 2 == 1 { root + 12 } else { root };
                let vel = if s == 2 { 0.75 } else { 0.55 };
                bass_note(&mut out, tb(bb + q as f32 + s as f32 * 0.25) + rng.bi() * 0.002, tb(0.2), note, vel, 2.0);
            }
        }
    }
    out
}

fn render_chops(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        let at = |s: usize| tb(bb + s as f32 * 0.25 + swing16(s));
        let stutter = |v: &mut Stereo, n: usize, from: usize, m: f32, rng: &mut Rng| {
            for k in 0..n {
                vox(v, at(from + k), tb(0.2), m, 0.65, [-0.35, 0.35][k % 2], A, A, &CHOP, rng);
            }
        };
        match part {
            STROBE => stutter(&mut v, if b % 4 == 3 { 4 } else { 2 }, if b % 4 == 3 { 12 } else { 14 }, 76.0, rng),
            HEADS | LIGHTS_UP => {
                for &(s, n, k, va, vb) in RIFF {
                    vox(&mut v, at(s), tb(n as f32 * 0.25) * 0.9, ch.riff[k], 0.8, 0.0, va, vb, &CHOP, rng);
                    if part == LIGHTS_UP && b >= 80 {
                        vox(&mut v, at(s), tb(n as f32 * 0.25) * 0.9, ch.riff[k] - 12.0, 0.3, 0.3, va, vb, &CHOP, rng);
                    }
                }
            }
            AFTER => {
                let n = if b < 96 { 4 } else if b < 100 { 2 } else { 0 };
                if n > 0 && b % 2 == 1 {
                    stutter(&mut v, n, 16 - n, 76.0, rng);
                }
            }
            _ => {}
        }
    }
    v
}

fn render_voice(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    let faint = VoxStyle { breath: 0.8, release: 0.8, ..SUNG };
    sing(&mut v, LINE, tb(48.0 * 4.0), BEAT, 0.0, 0.75, 0.0, &SUNG, 0.0, rng);
    sing(&mut v, LINE, tb(56.0 * 4.0), BEAT, 0.0, 0.8, 0.0, &SUNG, 0.0, rng);
    sing(&mut v, LINE, tb(80.0 * 4.0), BEAT, 0.0, 0.45, 0.0, &SUNG, 0.2, rng);
    sing(&mut v, LINE, tb(104.0 * 4.0), BEAT, 0.0, 0.55, 0.0, &faint, 0.3, rng);
    v
}

fn render_keys(len: usize, rng: &mut Rng) -> Stereo {
    let mut k = Stereo::new(len);
    for b in (0..BARS).step_by(2) {
        let part = part_at(b);
        if !matches!(part, ARMS | LIFTS) && !(part == LIGHTS_UP && b >= 80) {
            continue;
        }
        let ch = chord_at(b);
        let bb = (b * 4) as f32;
        let vel = if part == LIGHTS_UP { 0.4 } else { 0.6 };
        let last = b + 2 >= BARS;
        for (at, dur) in [(0.0, 2.8), (3.0, 1.0), (4.5, 3.4)] {
            let dur = if last && at > 4.0 { 8.0 } else { dur };
            let t = tb(bb + at) + rng.bi() * 0.005;
            piano(&mut k, t, tb(dur), ch.bass + 24, vel * 0.8, -0.2, rng);
            for (n, &note) in ch.pad.iter().enumerate() {
                piano(&mut k, t + n as f32 * 0.01, tb(dur), note + 12, vel * if at == 3.0 { 0.6 } else { 1.0 }, -0.15 + n as f32 * 0.1, rng);
            }
        }
    }
    k
}

fn render_synths(len: usize, rng: &mut Rng) -> Stereo {
    let mut s = Stereo::new(len);
    for b in 48..BARS {
        let part = part_at(b);
        let ch = chord_at(b);
        let notes = [ch.pad[0] + 12, ch.pad[1] + 12, ch.pad[2] + 12, ch.pad[3] + 12, ch.bass + 36];
        let t = tb((b * 4) as f32);
        match part {
            ARMS if b >= 56 && b % 2 == 0 => {
                let x = (b - 56) as f32 / 8.0;
                supersaw(&mut s, t, tb(8.0), &notes, 0.35 + 0.6 * x, 600.0 + 3000.0 * x, tb(6.0), rng);
            }
            LIGHTS_UP => supersaw(&mut s, t, tb(4.0) - 0.02, &notes, 1.0, 4800.0, 0.012, rng),
            _ => {}
        }
    }
    s
}

fn render_pads(len: usize, rng: &mut Rng) -> Stereo {
    let mut p = Stereo::new(len);
    for b in (48..BARS).step_by(2) {
        let part = part_at(b);
        if part == AFTER {
            continue;
        }
        let ch = chord_at(b);
        let dur = if b + 2 >= BARS { tb(8.0) + 3.0 } else { tb(8.0) + 0.1 };
        for &note in &ch.pad {
            pad_note(&mut p, tb((b * 4) as f32), dur, note, 0.8, pad_cutoff, no_wow, rng);
        }
    }
    p
}

fn render_lead(len: usize) -> Stereo {
    let mut out = Stereo::new(len);
    for bar in [72, 80] {
        for &(s, d, note) in LEAD {
            lead_note(&mut out, tb((bar * 4) as f32 + s), tb(d), note, 0.55, 3200.0, 0.1, no_wow);
        }
    }
    out
}

// ---------------------------------------------------------------- render

pub fn render() -> Track {
    let song_end = tb((BARS * 4) as f32);
    let len = secs(song_end + TAIL);
    let mut rng = Rng::new(0x0057_B0BE);

    let (mut drums, kick_bus, kicks) = render_drums(len, &mut rng);
    let mut mix = Mixer::new("strobe", len, duck_envelope(len, &kicks, 0.9, 0.3));

    // The techno rumble: the kick through a dark reverb, lowpassed, driven and pumped by the kick.
    let mut rumble = reverb(&kick_bus, 1.1, 0.9, 0.55, 0.01);
    filter_bus(&mut rumble, |_| 140.0, 0.8, false);
    rumble.normalize_rms(0.1);
    saturate(&mut rumble, 2.0);
    rumble.normalize_rms(0.1);
    mix.add("rumble", rumble, 0.45, 1.0, 0.0, 0.0);

    drums.mix_from(&kick_bus, 2.2);
    drop(kick_bus);
    drums.normalize_rms(0.1);
    mix.add("drums", drums, 1.05, 0.0, 0.06, 0.02);

    let mut stabs = render_stabs(len, &mut rng);
    stabs.normalize_rms(0.1);
    // Tame the stab transients so they don't set the master's peak level.
    stabs.scale(0.6);
    saturate(&mut stabs, 3.0);
    stabs.normalize_rms(0.1);
    mix.add("stabs", stabs, 0.5, 0.4, 0.25, 0.4);

    let mut bass = render_bass(len, &mut rng);
    bass.normalize_rms(0.1);
    mix.add("bass", bass, 0.7, 0.8, 0.0, 0.0);

    let mut chops = render_chops(len, &mut rng);
    chops.normalize_rms(0.1);
    mix.add("chops", chops, 0.5, 0.25, 0.15, 0.25);

    let mut voice = render_voice(len, &mut rng);
    voice.normalize_rms(0.1);
    mix.add("voice", voice, 0.55, 0.1, 0.45, 0.2);

    let mut keys = render_keys(len, &mut rng);
    keys.normalize_rms(0.1);
    lofi(&mut keys, 12.0, 2, |_| 0.2);
    mix.add("piano", keys, 0.55, 0.4, 0.35, 0.05);

    let mut synths = render_synths(len, &mut rng);
    synths.normalize_rms(0.1);
    mix.add("supersaw", synths, 0.6, 0.9, 0.25, 0.0);

    let mut pads = render_pads(len, &mut rng);
    pads.normalize_rms(0.1);
    mix.add("pads", pads, 0.55, 0.6, 0.4, 0.0);

    let mut lead = render_lead(len);
    lead.normalize_rms(0.1);
    mix.add("lead", lead, 0.45, 0.3, 0.4, 0.3);

    let Mixer { mut master, mut rev, dly, .. } = mix;
    let echoes = pingpong(&dly, tb(0.75), 0.45, 3400.0);
    master.mix_from(&echoes, 0.6);
    rev.mix_from(&echoes, 0.3);
    drop(dly);
    let hall = reverb(&rev, 1.3, 0.92, 0.35, 0.02);
    drop(rev);
    master.mix_from(&hall, 1.1);
    drop(hall);

    glue(&mut master, 1.6);
    fade_out(&mut master, song_end, song_end + TAIL);
    finish(&mut master);

    Track {
        slug: "strobe-through-fog",
        title: "Strobe Through Fog",
        key: "A minor",
        bpm: BPM,
        bars: BARS,
        influences: &["Fred again..", "Ben Böhmer", "Four Tet"],
        blurb: "Rumbling techno and a stuttering voice, a piano confession in the breakdown, then the lights come up.",
        sections: &SECTIONS,
        chords: (0..BARS).step_by(2).map(|b| (b, chord_at(b).name)).collect(),
        audio: master,
    }
}
