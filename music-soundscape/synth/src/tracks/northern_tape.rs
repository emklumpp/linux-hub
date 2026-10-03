//! "Northern Tape Memory": Boards of Canada's warped tape nostalgia, Four Tet's shuffling
//! organic percussion and kalimba, and Ben Böhmer's melancholic melodic-house lift.

use crate::dsp::*;
use crate::instruments::*;
use crate::track::*;

const TITLE: &str = "Northern Tape Memory";
const KEY: &str = "F minor";
const BPM: f32 = 120.0;
const BEAT: f32 = 60.0 / BPM;
const BARS: usize = 96;
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
}

/// i – VI – III – VII: the melancholy loop.
const PROG_A: [Chord; 4] = [
    Chord { name: "Fm9", bass: 41, pad: [56, 60, 63, 67] },
    Chord { name: "Dbmaj7", bass: 37, pad: [56, 60, 61, 65] },
    Chord { name: "Abmaj7", bass: 44, pad: [55, 60, 63, 68] },
    Chord { name: "Eb6", bass: 39, pad: [55, 58, 60, 63] },
];

/// VI – VII – v – i: the lift used in the breakdown and second drop.
const PROG_B: [Chord; 4] = [
    Chord { name: "Dbmaj9", bass: 37, pad: [56, 60, 63, 65] },
    Chord { name: "Eb", bass: 39, pad: [55, 58, 63, 67] },
    Chord { name: "Cm7", bass: 36, pad: [55, 58, 60, 63] },
    Chord { name: "Fm(add9)", bass: 41, pad: [56, 60, 65, 67] },
];

/// (start beat, length in beats, midi note) — 8 bars each.
const MELODY_A: &[(f32, f32, i32)] = &[
    (0.0, 1.5, 72), (1.5, 0.5, 68), (2.0, 1.0, 67), (3.0, 1.0, 68),
    (4.0, 2.0, 72), (6.0, 1.5, 75), (7.5, 0.5, 73),
    (8.0, 3.0, 72), (11.0, 1.0, 68), (12.0, 2.0, 65), (14.0, 1.0, 68), (15.0, 1.0, 72),
    (16.0, 1.5, 75), (17.5, 0.5, 72), (18.0, 1.0, 70), (19.0, 1.0, 72),
    (20.0, 3.0, 67), (23.0, 0.5, 68), (23.5, 0.5, 70),
    (24.0, 2.0, 67), (26.0, 1.0, 70), (27.0, 1.0, 72),
    (28.0, 1.5, 70), (29.5, 0.5, 67), (30.0, 2.0, 65),
];

const MELODY_B: &[(f32, f32, i32)] = &[
    (0.0, 3.0, 77), (3.0, 1.0, 75), (4.0, 2.0, 72), (6.0, 2.0, 68),
    (8.0, 2.0, 67), (10.0, 1.0, 70), (11.0, 1.0, 75), (12.0, 3.0, 79), (15.0, 1.0, 77),
    (16.0, 3.0, 75), (19.0, 1.0, 72), (20.0, 2.0, 67), (22.0, 2.0, 70),
    (24.0, 4.0, 72), (28.0, 1.0, 68), (29.0, 1.0, 67), (30.0, 2.0, 65),
];

/// Rolling arpeggio index pattern over 16 sixteenths.
const ARP: [usize; 16] = [0, 2, 1, 3, 2, 4, 3, 1, 0, 2, 4, 5, 3, 2, 1, 2];

// ---------------------------------------------------------------- structure

const INTRO: usize = 0;
const POLAROIDS: usize = 1;
const KALIMBA: usize = 2;
const BREAKDOWN: usize = 3;
const DROP1: usize = 4;
const DROP2: usize = 5;
const DECAY: usize = 6;
const OUTRO: usize = 7;

static SECTIONS: [Section; 8] = [
    Section { name: "Dawn Tape", start: 0, end: 8, influence: "Boards of Canada",
        mood: "Warped pads surface through vinyl hiss like a half-remembered home movie.",
        layers: &["pad", "vinyl", "harp"] },
    Section { name: "Faded Polaroids", start: 8, end: 24, influence: "Boards of Canada",
        mood: "Dusty half-time beat, wobbling tape and a detuned music-box melody.",
        layers: &["pad", "dusty drums", "harp", "sub", "music box"] },
    Section { name: "Kalimba Rain", start: 24, end: 40, influence: "Four Tet",
        mood: "Shuffled shakers, wooden clicks and a cascading kalimba arpeggio.",
        layers: &["pad", "kalimba", "shaker", "rims", "bass", "lead"] },
    Section { name: "Glass Morning", start: 40, end: 48, influence: "Ben Böhmer",
        mood: "The beat falls away; open pads and a longing lead under a vast reverb.",
        layers: &["pad", "lead", "kalimba", "riser"] },
    Section { name: "Northern Pulse", start: 48, end: 64, influence: "Ben Böhmer",
        mood: "Four-to-the-floor, sidechained pads and a rolling bassline that aches.",
        layers: &["pad", "kick", "clap", "hats", "rolling bass", "lead", "kalimba"] },
    Section { name: "Remember Me Slowly", start: 64, end: 80, influence: "Böhmer × Four Tet",
        mood: "The lift: new chords, an octave-doubled melody, everything at once.",
        layers: &["pad", "kick", "clap", "hats", "shaker", "rolling bass", "lead", "kalimba", "harp"] },
    Section { name: "Signal Decay", start: 80, end: 88, influence: "Boards of Canada",
        mood: "Back to the dusty half-time; the melody sinks an octave and starts to warp.",
        layers: &["pad", "dusty drums", "harp", "sub", "lead"] },
    Section { name: "Tape Runs Out", start: 88, end: 96, influence: "Boards of Canada",
        mood: "Only the pad and a few plucks remain before the tape grinds to a stop.",
        layers: &["pad", "harp", "vinyl"] },
];

fn part_at(bar: usize) -> usize {
    section_at(&SECTIONS, bar)
}

fn chord_at(bar: usize) -> &'static Chord {
    let part = part_at(bar);
    let prog = if part == BREAKDOWN || part == DROP2 { &PROG_B } else { &PROG_A };
    &prog[(bar / 2) % 4]
}

fn arp_tones(ch: &Chord) -> [i32; 8] {
    let p = ch.pad;
    [p[0] + 12, p[1] + 12, p[2] + 12, p[3] + 12, p[0] + 24, p[1] + 24, p[2] + 24, p[3] + 24]
}

/// Offset (in beats) for a swung sixteenth step.
fn swing16(step: usize, amount: f32) -> f32 {
    if step % 2 == 1 { amount * 0.25 } else { 0.0 }
}

// ---------------------------------------------------------------- automation (x = bars)

fn pad_cutoff(t: f32) -> f32 {
    automation(&[(0.0, 350.0), (8.0, 1400.0), (24.0, 1800.0), (40.0, 1600.0), (44.0, 3800.0),
        (48.0, 2600.0), (64.0, 3000.0), (80.0, 3200.0), (88.0, 1500.0), (96.0, 400.0), (100.0, 300.0)], bar_at(t))
}

fn wow_depth(t: f32) -> f32 {
    automation(&[(0.0, 0.006), (8.0, 0.007), (24.0, 0.003), (40.0, 0.004), (48.0, 0.0015),
        (80.0, 0.002), (88.0, 0.006), (100.0, 0.010)], bar_at(t))
}

/// Wow & flutter: slow, irregular pitch drift of a worn cassette.
fn tape_wow(t: f32) -> f32 {
    let lfo = 0.6 * (std::f32::consts::TAU * 0.31 * t).sin()
        + 0.3 * (std::f32::consts::TAU * 0.83 * t + 1.1).sin()
        + 0.1 * (std::f32::consts::TAU * 4.7 * t + 0.4).sin();
    1.0 + wow_depth(t) * lfo
}

fn lead_wow(t: f32) -> f32 {
    1.0 + 0.6 * (tape_wow(t) - 1.0)
}

fn no_wow(_: f32) -> f32 {
    1.0
}

fn lofi_mix(t: f32) -> f32 {
    automation(&[(0.0, 0.8), (8.0, 0.65), (24.0, 0.35), (48.0, 0.12), (80.0, 0.35), (88.0, 0.7), (100.0, 0.9)], bar_at(t))
}

fn vinyl_level(t: f32) -> f32 {
    automation(&[(0.0, 1.0), (8.0, 0.7), (24.0, 0.4), (48.0, 0.2), (80.0, 0.45), (88.0, 0.9), (100.0, 1.0)], bar_at(t))
}

// ---------------------------------------------------------------- tracks

fn render_drums(len: usize, rng: &mut Rng) -> (Stereo, Vec<f32>) {
    let mut d = Stereo::new(len);
    let mut sidechain = Vec::new();
    for b in 0..BARS {
        let part = part_at(b);
        let bb = (b * 4) as f32;
        let h = |rng: &mut Rng| rng.bi() * 0.006;
        match part {
            POLAROIDS | DECAY => {
                kick(&mut d, tb(bb) + h(rng), 0.9, true, rng);
                let second = if b % 2 == 1 { 2.75 } else { 2.5 };
                kick(&mut d, tb(bb + second) + h(rng), 0.6, true, rng);
                snare(&mut d, tb(bb + 2.0) + h(rng), 0.7, true, rng);
                if b % 4 == 3 {
                    snare(&mut d, tb(bb + 3.75), 0.22, true, rng);
                }
                if b >= 12 || part == DECAY {
                    for s in 0..8 {
                        let swing = if s % 2 == 1 { 0.12 } else { 0.0 };
                        let vel = if s % 2 == 0 { 0.3 } else { 0.18 };
                        hat(&mut d, tb(bb + s as f32 * 0.5 + swing) + h(rng), vel, false, 0.25, rng);
                    }
                }
            }
            KALIMBA => {
                kick(&mut d, tb(bb), 0.85, false, rng);
                kick(&mut d, tb(bb + 2.5), 0.6, false, rng);
                if b % 2 == 1 {
                    kick(&mut d, tb(bb + 3.25), 0.4, false, rng);
                }
                snare(&mut d, tb(bb + 2.0) + h(rng), 0.45, false, rng);
                const RIMS: [[usize; 5]; 2] = [[3, 6, 10, 13, 15], [2, 7, 9, 11, 14]];
                for s in 0..16 {
                    let t = tb(bb + s as f32 * 0.25 + swing16(s, 0.3)) + h(rng);
                    let accent = [0.55, 0.25, 0.4, 0.25][s % 4];
                    shaker(&mut d, t, accent, -0.3, rng);
                    if RIMS[b % 2].contains(&s) {
                        let freq = [1100.0, 1500.0, 2200.0][(rng.f() * 3.0) as usize];
                        rim(&mut d, t, rng.range(0.25, 0.5), freq, rng.range(-0.7, 0.7), rng);
                    }
                }
                for q in 0..4 {
                    hat(&mut d, tb(bb + q as f32 + 0.5) + h(rng), 0.22, false, 0.35, rng);
                }
            }
            DROP1 | DROP2 => {
                for q in 0..4 {
                    let t = tb(bb + q as f32);
                    kick(&mut d, t, 1.0, false, rng);
                    sidechain.push(t);
                    hat(&mut d, tb(bb + q as f32 + 0.5) + h(rng), 0.32, true, 0.15, rng);
                }
                clap(&mut d, tb(bb + 1.0), 0.6, rng);
                clap(&mut d, tb(bb + 3.0), 0.6, rng);
                for s in 0..16 {
                    let t = tb(bb + s as f32 * 0.25 + swing16(s, 0.12)) + h(rng);
                    hat(&mut d, t, [0.14, 0.07, 0.2, 0.07][s % 4], false, -0.2, rng);
                    if part == DROP2 {
                        shaker(&mut d, t, [0.35, 0.15, 0.25, 0.15][s % 4], 0.4, rng);
                    }
                    if s % 4 != 0 && rng.chance(0.15) {
                        let freq = rng.range(900.0, 2400.0);
                        rim(&mut d, t, rng.range(0.2, 0.4), freq, rng.range(-0.8, 0.8), rng);
                    }
                }
                if b == 63 || b == 79 {
                    for s in 0..8 {
                        snare(&mut d, tb(bb + 2.0 + s as f32 * 0.25), 0.2 + s as f32 * 0.07, false, rng);
                    }
                }
            }
            _ => {}
        }
    }
    riser(&mut d, tb(46.0 * 4.0), tb(8.0), 0.5, rng);
    riser(&mut d, tb(63.0 * 4.0), tb(4.0), 0.35, rng);
    impact(&mut d, tb(48.0 * 4.0), 0.7, rng);
    impact(&mut d, tb(64.0 * 4.0), 0.6, rng);
    (d, sidechain)
}

fn render_pads(len: usize, rng: &mut Rng) -> Stereo {
    const VEL: [f32; 8] = [0.9, 0.8, 0.7, 1.0, 0.75, 0.85, 0.8, 0.9];
    let mut p = Stereo::new(len);
    for b in (0..BARS).step_by(2) {
        let ch = chord_at(b);
        let vel = VEL[part_at(b)];
        let dur = if b + 2 >= BARS { tb(8.0) + 2.0 } else { tb(8.0) + 0.1 };
        let t0 = tb((b * 4) as f32);
        for &note in ch.pad.iter().chain(std::iter::once(&(ch.bass + 12))) {
            pad_note(&mut p, t0, dur, note, vel, pad_cutoff, tape_wow, rng);
        }
    }
    p
}

fn render_plucks(len: usize, rng: &mut Rng) -> Stereo {
    let mut p = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        let tones = arp_tones(chord_at(b));
        let bb = (b * 4) as f32;
        let jitter = |rng: &mut Rng| rng.bi() * 0.008;
        match part {
            INTRO if b >= 4 => {
                for q in [0.0, 2.0] {
                    let note = tones[4 + (rng.f() * 4.0) as usize];
                    pluck(&mut p, tb(bb + q), note, 0.3, rng.range(-0.6, 0.6), 2.5, 2500.0, rng);
                }
            }
            POLAROIDS | DECAY => {
                for s in 0..8 {
                    if rng.chance(0.22) {
                        continue;
                    }
                    let swing = if s % 2 == 1 { 0.12 } else { 0.0 };
                    let pan = if s % 2 == 0 { -0.45 } else { 0.45 };
                    let t = tb(bb + s as f32 * 0.5 + swing) + jitter(rng);
                    pluck(&mut p, t, tones[ARP[s * 2]], 0.32, pan, 1.6, 3000.0, rng);
                }
            }
            KALIMBA | DROP1 | DROP2 => {
                for s in 0..16 {
                    if rng.chance(0.2) {
                        continue;
                    }
                    let accent = if s % 4 == 0 { 1.0 } else { 0.7 };
                    let pan = (s as f32 * 0.9 + b as f32).sin() * 0.6;
                    let t = tb(bb + s as f32 * 0.25 + swing16(s, if part == KALIMBA { 0.3 } else { 0.12 })) + jitter(rng);
                    kalimba(&mut p, t, tones[ARP[s]], 0.35 * accent, pan, 1.4, rng);
                }
                if part == KALIMBA {
                    pluck(&mut p, tb(bb), tones[4 + b % 4], 0.25, 0.5, 2.2, 3500.0, rng);
                }
                if part == DROP2 {
                    for q in 0..4 {
                        let note = tones[4 + (q + b) % 4] + 12;
                        pluck(&mut p, tb(bb + q as f32 + 0.5), note, 0.13, -0.6, 1.2, 4000.0, rng);
                    }
                }
            }
            BREAKDOWN => {
                for q in 0..4 {
                    if rng.chance(0.3) {
                        continue;
                    }
                    kalimba(&mut p, tb(bb + q as f32), tones[ARP[q * 4]], 0.3, rng.range(-0.5, 0.5), 3.0, rng);
                }
            }
            OUTRO if b < 94 => {
                let fade = 1.0 - (b - 88) as f32 / 6.0;
                for q in [0.0, 2.0] {
                    if rng.chance(0.3) {
                        continue;
                    }
                    let note = tones[(rng.f() * 6.0) as usize];
                    pluck(&mut p, tb(bb + q), note, 0.3 * fade, rng.range(-0.6, 0.6), 3.0, 2000.0, rng);
                }
            }
            _ => {}
        }
    }
    p
}

fn render_bass(len: usize) -> Stereo {
    let mut out = Stereo::new(len);
    for b in 0..BARS {
        let part = part_at(b);
        let root = chord_at(b).bass;
        let bb = (b * 4) as f32;
        match part {
            POLAROIDS | DECAY if b % 2 == 0 => bass_note(&mut out, tb(bb), tb(7.5), root, 0.6, 1.2),
            BREAKDOWN if b >= 44 && b % 2 == 0 => bass_note(&mut out, tb(bb), tb(7.5), root, 0.4, 1.0),
            KALIMBA => {
                for (start, dur, oct) in [(0.0, 1.25, 0), (1.5, 0.5, 0), (2.5, 1.0, 0), (3.5, 0.4, 12)] {
                    bass_note(&mut out, tb(bb + start), tb(dur), root + oct, 0.6, 1.4);
                }
            }
            DROP1 | DROP2 => {
                // Rolling sixteenths between the kicks.
                for q in 0..4 {
                    for s in 1..4 {
                        let note = if q == 3 && s == 3 && b % 2 == 1 { root + 12 } else { root };
                        let vel = if s == 2 { 0.75 } else { 0.55 };
                        bass_note(&mut out, tb(bb + q as f32 + s as f32 * 0.25), tb(0.2), note, vel, 1.6);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn render_lead(len: usize) -> Stereo {
    let mut out = Stereo::new(len);
    let mut play = |mel: &[(f32, f32, i32)], bar: usize, transpose: i32, vel: f32, bright: f32, pan: f32, wow: Curve| {
        let bb = (bar * 4) as f32;
        for &(start, dur, note) in mel {
            lead_note(&mut out, tb(bb + start), tb(dur), note + transpose, vel, bright, pan, wow);
        }
    };
    play(MELODY_A, 16, -12, 0.35, 900.0, -0.15, tape_wow); // music box through warped tape
    play(MELODY_A, 32, 0, 0.45, 1800.0, 0.1, lead_wow);
    play(MELODY_B, 40, 0, 0.5, 2600.0, 0.0, lead_wow);
    play(MELODY_A, 48, 0, 0.55, 3000.0, 0.0, no_wow);
    play(MELODY_A, 56, 0, 0.55, 3000.0, 0.0, no_wow);
    play(MELODY_A, 56, 12, 0.2, 2500.0, 0.3, no_wow);
    play(MELODY_B, 64, 0, 0.55, 3200.0, 0.0, no_wow);
    play(MELODY_B, 72, 0, 0.55, 3200.0, 0.0, no_wow);
    play(MELODY_B, 72, 12, 0.22, 2600.0, -0.3, no_wow);
    play(MELODY_A, 80, -12, 0.35, 1000.0, 0.15, tape_wow);
    out
}

// ---------------------------------------------------------------- render

pub fn render() -> Track {
    let song_end = tb((BARS * 4) as f32);
    let len = secs(song_end + TAIL);
    let mut rng = Rng::new(0x0B0C_4E7B);

    let (mut drums, kicks) = render_drums(len, &mut rng);
    let mut mix = Mixer::new("northern", len, duck_envelope(len, &kicks, 0.8, 0.3));
    drums.normalize_rms(0.1);
    mix.add("drums", drums, 1.1, 0.0, 0.07, 0.0);

    let mut pads = render_pads(len, &mut rng);
    pads.normalize_rms(0.1);
    lofi(&mut pads, 10.0, 3, lofi_mix);
    mix.add("pads", pads, 0.85, 0.75, 0.35, 0.0);

    let mut plucks = render_plucks(len, &mut rng);
    plucks.normalize_rms(0.1);
    lofi(&mut plucks, 12.0, 2, lofi_mix);
    mix.add("plucks", plucks, 0.55, 0.35, 0.3, 0.3);

    let mut bass = render_bass(len);
    bass.normalize_rms(0.1);
    mix.add("bass", bass, 0.8, 0.85, 0.0, 0.0);

    let mut lead = render_lead(len);
    lead.normalize_rms(0.1);
    mix.add("lead", lead, 0.55, 0.2, 0.45, 0.32);

    let Mixer { mut master, mut rev, dly, .. } = mix;
    let echoes = pingpong(&dly, tb(0.75), 0.48, 3200.0);
    master.mix_from(&echoes, 0.7);
    rev.mix_from(&echoes, 0.35);
    drop(dly);
    let hall = reverb(&rev, 1.35, 0.93, 0.35, 0.03);
    drop(rev);
    master.mix_from(&hall, 1.25);
    drop(hall);

    // Master: tape saturation glue, vinyl bed, tape stop, final level.
    glue(&mut master, 1.6);
    vinyl(&mut master, vinyl_level, &mut rng);
    tape_stop(&mut master, secs(song_end + 3.5), secs(2.2));
    finish(&mut master);

    Track {
        slug: "northern-tape-memory",
        title: TITLE,
        key: KEY,
        bpm: BPM,
        bars: BARS,
        influences: &["Boards of Canada", "Four Tet", "Ben Böhmer"],
        blurb: "Where it starts: warped cassette pads, kalimba rain and a melodic-house lift.",
        sections: &SECTIONS,
        chords: (0..BARS).step_by(2).map(|b| (b, chord_at(b).name)).collect(),
        audio: master,
    }
}
