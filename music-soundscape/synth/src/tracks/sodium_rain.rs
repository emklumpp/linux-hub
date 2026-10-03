//! "Sodium Rain": Burial-era dubstep — half-time 140, clanking snares, tempo-synced wobble and
//! a ghost vocal in the rain — lifted by Ben Böhmer's longing lead, rolling arps and wide pads.

use crate::dsp::*;
use crate::instruments::*;
use crate::track::*;
use crate::voices::Vowel::*;
use crate::voices::*;

const BPM: f32 = 140.0;
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

/// i – VI – III – VII in G minor, two bars each.
const PROG: [Chord; 4] = [
    Chord { name: "Gm9", bass: 31, pad: [58, 62, 65, 69] },
    Chord { name: "Ebmaj9", bass: 39, pad: [55, 58, 62, 65] },
    Chord { name: "Bbmaj9", bass: 34, pad: [57, 60, 62, 65] },
    Chord { name: "F(add9)", bass: 29, pad: [57, 60, 65, 67] },
];

fn chord_at(bar: usize) -> &'static Chord {
    &PROG[(bar / 2) % 4]
}

/// Böhmer lead, 8 bars: (beat, length, note).
const LEAD: &[(f32, f32, i32)] = &[
    (0.0, 3.0, 74), (3.0, 1.0, 72), (4.0, 2.0, 70), (6.0, 2.0, 69),
    (8.0, 3.0, 70), (11.0, 1.0, 67), (12.0, 4.0, 74),
    (16.0, 2.0, 72), (18.0, 2.0, 74), (20.0, 2.0, 77), (22.0, 2.0, 74),
    (24.0, 4.0, 72), (28.0, 2.0, 69), (30.0, 2.0, 67),
];

const GHOST_LINE: Phrase = &[
    (0.0, 2.0, 70.0, O, A), (2.0, 2.0, 74.0, A, E), (4.0, 3.5, 72.0, E, I),
    (8.0, 1.5, 70.0, A, O), (9.5, 0.5, 67.0, O, O), (10.0, 4.0, 74.0, O, A),
    (16.0, 2.0, 77.0, A, E), (18.0, 2.0, 74.0, E, A), (20.0, 4.0, 72.0, A, O),
    (24.0, 3.0, 72.0, O, U), (27.0, 1.0, 69.0, U, A), (28.0, 4.0, 70.0, A, O),
];

/// Wobble phrasing per two-bar chord: (beat, length, semitones above root, LFO cycles per beat).
type Wobble = &'static [(f32, f32, f32, f32)];
const WOB_A: Wobble = &[(0.0, 1.5, 0.0, 1.0), (1.5, 0.5, 0.0, 4.0), (2.0, 2.0, 0.0, 2.0), (4.0, 1.0, 12.0, 3.0), (5.0, 1.0, 0.0, 2.0), (6.0, 2.0, 0.0, 1.5)];
const WOB_B: Wobble = &[(0.0, 1.0, 0.0, 2.0), (1.0, 1.0, 0.0, 4.0), (2.0, 0.5, 12.0, 4.0), (2.5, 1.5, 0.0, 3.0), (4.0, 2.0, 0.0, 1.0), (6.0, 1.0, 0.0, 4.0), (7.0, 1.0, 7.0, 6.0)];

// ---------------------------------------------------------------- structure

const LIGHTS: usize = 0;
const HALFTIME: usize = 1;
const WOBBLE: usize = 2;
const GLASS: usize = 3;
const SODIUM: usize = 4;
const GHOSTBOX: usize = 5;
const FLOOD: usize = 6;
const DRY: usize = 7;

static SECTIONS: [Section; 8] = [
    Section { name: "Sodium Lights", start: 0, end: 8, influence: "Burial",
        mood: "Orange streetlight on wet tarmac; rain, a far-off voice and a pad opening slowly.",
        layers: &["rain", "pad", "ghost vocal", "vinyl"] },
    Section { name: "Halftime Steps", start: 8, end: 24, influence: "Burial",
        mood: "Half-time dubstep: one kick, one clanking snare, shuffled hats and a sub you feel more than hear.",
        layers: &["half-time drums", "clank snare", "sub", "pad", "rain"] },
    Section { name: "Wobble Weather", start: 24, end: 40, influence: "Burial",
        mood: "The bass starts to talk — tempo-synced wobble under the ghost vocal.",
        layers: &["half-time drums", "wobble bass", "ghost vocal", "pad"] },
    Section { name: "Glass Raindrops", start: 40, end: 48, influence: "Ben Böhmer",
        mood: "Drums out: a longing lead and plucked arps under a huge reverb.",
        layers: &["lead", "arp", "pad", "riser"] },
    Section { name: "Sodium Rain", start: 48, end: 72, influence: "Burial × Ben Böhmer",
        mood: "The drop: wobble and half-time weight with the Böhmer lead soaring over it.",
        layers: &["half-time drums", "wobble bass", "lead", "arp", "pad", "ghost vocal"] },
    Section { name: "Ghost Box", start: 72, end: 88, influence: "Burial",
        mood: "The beat flips to skippy 2-step; a reese breathes and the vocal sinks.",
        layers: &["2-step drums", "reese", "sub", "ghost vocal", "pad"] },
    Section { name: "Flooded Underpass", start: 88, end: 104, influence: "Burial × Ben Böhmer",
        mood: "Everything at once: faster wobble, the lead an octave up, the voice doubled.",
        layers: &["half-time drums", "wobble bass", "lead", "arp", "ghost vocal", "pad"] },
    Section { name: "Dry Out", start: 104, end: 112, influence: "Burial",
        mood: "The rain eases off and the last chord hangs in the orange light.",
        layers: &["rain", "pad", "vinyl"] },
];

fn part_at(bar: usize) -> usize {
    section_at(&SECTIONS, bar)
}

// ---------------------------------------------------------------- automation (x = bars)

fn pad_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 400.0), (8.0, 1500.0), (24.0, 1700.0), (40.0, 3200.0), (48.0, 2400.0), (72.0, 1600.0),
        (88.0, 2800.0), (104.0, 1200.0), (112.0, 400.0)], bar_at(t))
}

fn slight_wow(t: f32) -> f32 {
    1.0 + 0.0025 * (0.7 * (std::f32::consts::TAU * 0.29 * t).sin() + 0.3 * (std::f32::consts::TAU * 0.77 * t).sin())
}

fn no_wow(_: f32) -> f32 {
    1.0
}

fn rain_level(t: f32) -> f32 {
    automation(&[(0.0, 1.0), (8.0, 0.5), (24.0, 0.3), (40.0, 0.7), (48.0, 0.25), (88.0, 0.3), (104.0, 1.0),
        (110.0, 0.8), (115.0, 0.0)], bar_at(t))
}

fn vinyl_level(t: f32) -> f32 {
    automation(&[(0.0, 0.8), (8.0, 0.35), (104.0, 0.6), (112.0, 0.9)], bar_at(t))
}

fn swing16(step: usize) -> f32 {
    if step % 2 == 1 { 0.075 } else { 0.0 }
}

// ---------------------------------------------------------------- tracks

fn render_drums(len: usize, rng: &mut Rng) -> (Stereo, Vec<f32>) {
    let mut d = Stereo::new(len);
    let mut kicks = Vec::new();
    for b in 0..BARS {
        let part = part_at(b);
        if matches!(part, LIGHTS | GLASS | DRY) {
            continue;
        }
        let bb = (b * 4) as f32;
        let at = |s: usize, rng: &mut Rng| tb(bb + s as f32 * 0.25 + swing16(s)) + rng.bi() * 0.006;
        let two_step = part == GHOSTBOX;
        let kick_steps: &[usize] = if two_step {
            if b % 2 == 0 { &[0, 10] } else { &[0, 7, 10] }
        } else if b % 4 == 3 {
            &[0, 11, 14]
        } else if b % 2 == 1 {
            &[0, 14]
        } else {
            &[0]
        };
        let snare_steps: &[usize] = if two_step { &[4, 12] } else { &[8] };
        for &s in kick_steps {
            let t = at(s, rng);
            kick(&mut d, t, if s == 0 { 1.0 } else { 0.7 }, false, rng);
            kicks.push(t);
        }
        for &s in snare_steps {
            let t = at(s, rng);
            clank(&mut d, t, 0.85, rng.range(430.0, 500.0), 0.0, rng);
            if !two_step {
                clap(&mut d, t + 0.004, 0.35, rng);
            }
        }
        if rng.chance(0.4) {
            let s = [3, 13, 15][(rng.f() * 3.0) as usize];
            clank(&mut d, at(s, rng), rng.range(0.12, 0.25), rng.range(620.0, 720.0), rng.range(-0.5, 0.5), rng);
        }
        let density = match part {
            HALFTIME => 0.35,
            FLOOD => 0.7,
            _ => 0.5,
        };
        for s in 0..16 {
            let anchor = s % 4 == 2;
            if anchor || rng.chance(density) {
                hat(&mut d, at(s, rng), if anchor { 0.28 } else { rng.range(0.07, 0.18) }, false, rng.range(-0.1, 0.4), rng);
            }
            if part == FLOOD && s % 4 != 0 && rng.chance(0.15) {
                rim(&mut d, at(s, rng), rng.range(0.15, 0.3), rng.range(900.0, 2200.0), rng.range(-0.8, 0.8), rng);
            }
        }
        if part == SODIUM || part == FLOOD {
            hat(&mut d, at(14, rng), 0.22, true, -0.2, rng);
        }
    }
    riser(&mut d, tb(44.0 * 4.0), tb(16.0), 0.5, rng);
    riser(&mut d, tb(86.0 * 4.0), tb(8.0), 0.35, rng);
    impact(&mut d, tb(48.0 * 4.0), 0.8, rng);
    impact(&mut d, tb(88.0 * 4.0), 0.6, rng);
    (d, kicks)
}

fn render_pads(len: usize, rng: &mut Rng) -> Stereo {
    const VEL: [f32; 8] = [0.85, 0.7, 0.65, 1.0, 0.7, 0.75, 0.7, 0.9];
    let mut p = Stereo::new(len);
    for b in (0..BARS).step_by(2) {
        let ch = chord_at(b);
        let dur = if b + 2 >= BARS { tb(8.0) + 3.0 } else { tb(8.0) + 0.1 };
        for &note in ch.pad.iter().chain(std::iter::once(&(ch.bass + 24))) {
            pad_note(&mut p, tb((b * 4) as f32), dur, note, VEL[part_at(b)], pad_cutoff, slight_wow, rng);
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
        let bb = (b * 4) as f32;
        let wobble = |out: &mut Stereo, pattern: Wobble, hi: f32, rng: &mut Rng| {
            for &(s, d, semis, rate) in pattern {
                reese(out, tb(bb + s), tb(d) - 0.01, root + semis, 0.8, 75.0, hi, rate * BPM / 60.0, 2.6, rng);
            }
        };
        match part {
            HALFTIME => sub(&mut out, tb(bb), tb(8.0) - 0.03, root, prev, 0.1, 0.9),
            WOBBLE => wobble(&mut out, WOB_A, if b < 32 { 700.0 } else { 1300.0 }, rng),
            SODIUM => wobble(&mut out, WOB_A, 1100.0, rng),
            FLOOD => wobble(&mut out, WOB_B, 1600.0, rng),
            GHOSTBOX => {
                sub(&mut out, tb(bb), tb(8.0) - 0.03, root, prev, 0.1, 0.75);
                reese(&mut out, tb(bb), tb(8.0) - 0.03, root + 12.0, 0.5, 160.0, 800.0, 0.0, 2.0, rng);
            }
            _ => {}
        }
        prev = root;
    }
    out
}

fn render_lead(len: usize) -> Stereo {
    let mut out = Stereo::new(len);
    let mut play = |bar: usize, transpose: i32, vel: f32, bright: f32, pan: f32| {
        let bb = (bar * 4) as f32;
        for &(s, d, note) in LEAD {
            lead_note(&mut out, tb(bb + s), tb(d), note + transpose, vel, bright, pan, no_wow);
        }
    };
    play(40, 0, 0.5, 2400.0, 0.0);
    for bar in [48, 56, 64] {
        play(bar, 0, 0.55, 3000.0, 0.0);
    }
    play(64, -12, 0.25, 1800.0, 0.25);
    for bar in [88, 96] {
        play(bar, 0, 0.5, 3200.0, -0.1);
        play(bar, 12, 0.25, 2600.0, 0.3);
    }
    out
}

fn render_arps(len: usize, rng: &mut Rng) -> Stereo {
    const ARP: [usize; 16] = [0, 2, 1, 3, 2, 4, 3, 1, 0, 2, 4, 5, 3, 2, 1, 2];
    let mut a = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        let sixteenths = matches!(part, SODIUM | FLOOD);
        if !sixteenths && part != GLASS {
            continue;
        }
        let ch = chord_at(b);
        let tones = [ch.pad[0] + 12, ch.pad[1] + 12, ch.pad[2] + 12, ch.pad[3] + 12, ch.pad[0] + 24, ch.pad[1] + 24];
        let bb = (b * 4) as f32;
        let steps = if sixteenths { 16 } else { 8 };
        for s in 0..steps {
            let beat = s as f32 * 4.0 / steps as f32;
            let note = tones[ARP[s * 16 / steps]];
            let pan = (s as f32 * 0.7 + b as f32 * 0.3).sin() * 0.55;
            pluck(&mut a, tb(bb + beat) + rng.bi() * 0.003, note, if s % 4 == 0 { 0.4 } else { 0.28 }, pan, 1.1, 3800.0, rng);
        }
    }
    a
}

fn render_vocals(len: usize, rng: &mut Rng) -> Stereo {
    let mut v = Stereo::new(len);
    let sinking = VoxStyle { drift: -1.2, formant: 1.15, ..GHOST };
    for b in (0..BARS).step_by(8) {
        let t = tb((b * 4) as f32);
        match part_at(b) {
            LIGHTS => sing(&mut v, GHOST_LINE, t, BEAT, 0.0, 0.5, 0.15, &GHOST, 0.5, rng),
            WOBBLE => sing(&mut v, GHOST_LINE, t, BEAT, 0.0, 0.7, 0.0, &GHOST, 0.15, rng),
            SODIUM if b >= 56 => sing(&mut v, GHOST_LINE, t, BEAT, 12.0, 0.35, -0.3, &GHOST, 0.35, rng),
            GHOSTBOX => sing(&mut v, GHOST_LINE, t, BEAT, 0.0, 0.7, 0.0, &sinking, 0.1, rng),
            FLOOD => {
                sing(&mut v, GHOST_LINE, t, BEAT, 0.0, 0.65, -0.2, &GHOST, 0.0, rng);
                sing(&mut v, GHOST_LINE, t, BEAT, 12.0, 0.3, 0.3, &GHOST, 0.3, rng);
            }
            DRY => sing(&mut v, GHOST_LINE, t, BEAT, 0.0, 0.4, 0.0, &sinking, 0.6, rng),
            _ => {}
        }
    }
    v
}

// ---------------------------------------------------------------- render

pub fn render() -> Track {
    let song_end = tb((BARS * 4) as f32);
    let len = secs(song_end + TAIL);
    let mut rng = Rng::new(0x50D1_0A);

    let (mut drums, kicks) = render_drums(len, &mut rng);
    let mut mix = Mixer::new("sodium", len, duck_envelope(len, &kicks, 0.7, 0.3));
    drums.normalize_rms(0.1);
    lofi(&mut drums, 11.0, 2, |_| 0.4);
    mix.add("drums", drums, 1.05, 0.0, 0.12, 0.02);

    let mut pads = render_pads(len, &mut rng);
    pads.normalize_rms(0.1);
    mix.add("pads", pads, 0.75, 0.5, 0.4, 0.0);

    let mut bass = render_bass(len, &mut rng);
    bass.normalize_rms(0.1);
    mix.add("bass", bass, 0.85, 0.6, 0.0, 0.0);

    let mut lead = render_lead(len);
    lead.normalize_rms(0.1);
    mix.add("lead", lead, 0.5, 0.2, 0.45, 0.3);

    let mut arps = render_arps(len, &mut rng);
    arps.normalize_rms(0.1);
    mix.add("arps", arps, 0.4, 0.35, 0.3, 0.35);

    let mut vocals = render_vocals(len, &mut rng);
    vocals.normalize_rms(0.1);
    mix.add("vocals", vocals, 0.5, 0.15, 0.7, 0.3);

    let mut wet = Stereo::new(len);
    rain(&mut wet, rain_level, &mut rng);
    wet.normalize_rms(0.1);
    mix.add("rain", wet, 0.3, 0.0, 0.08, 0.0);

    let Mixer { mut master, mut rev, dly, .. } = mix;
    let echoes = pingpong(&dly, tb(0.75), 0.5, 3000.0);
    master.mix_from(&echoes, 0.65);
    rev.mix_from(&echoes, 0.35);
    drop(dly);
    let hall = reverb(&rev, 1.45, 0.935, 0.4, 0.03);
    drop(rev);
    master.mix_from(&hall, 1.2);
    drop(hall);

    glue(&mut master, 1.8);
    vinyl(&mut master, vinyl_level, &mut rng);
    fade_out(&mut master, song_end, song_end + TAIL);
    finish(&mut master);

    Track {
        slug: "sodium-rain",
        title: "Sodium Rain",
        key: "G minor",
        bpm: BPM,
        bars: BARS,
        influences: &["Burial", "Ben Böhmer"],
        blurb: "Half-time dubstep under orange streetlight: wobble bass, ghost vocals and a Böhmer lead soaring over the rain.",
        sections: &SECTIONS,
        chords: (0..BARS).step_by(2).map(|b| (b, chord_at(b).name)).collect(),
        audio: master,
    }
}
