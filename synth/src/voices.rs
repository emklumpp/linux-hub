//! Voices for the after-dark half of the album: Fred again..'s chopped vocals, felt piano and
//! euphoric supersaws, techno stabs and kicks, and Burial's clanking 2-step snares, ghost vocals,
//! rain and reese / wobble bass. Same convention as `instruments`: each call renders one note or
//! hit additively into a stereo buffer.

use crate::dsp::*;
use crate::instruments::Curve;
use std::f32::consts::TAU;

// ---------------------------------------------------------------- vocals

#[derive(Clone, Copy, PartialEq)]
pub enum Vowel {
    A,
    E,
    I,
    O,
    U,
}

impl Vowel {
    /// First three formants (Hz) of a soft, breathy voice.
    fn formants(self) -> [f32; 3] {
        match self {
            Vowel::A => [800.0, 1180.0, 2850.0],
            Vowel::E => [480.0, 1850.0, 2650.0],
            Vowel::I => [310.0, 2300.0, 3100.0],
            Vowel::O => [500.0, 860.0, 2700.0],
            Vowel::U => [340.0, 760.0, 2450.0],
        }
    }
}

/// How a vocal note is sung and treated.
#[derive(Clone, Copy)]
pub struct VoxStyle {
    pub attack: f32,
    pub release: f32,
    /// Semitones the note scoops up from at its start.
    pub scoop: f32,
    /// Semitones the pitch slides over the whole note — Burial's warped, sinking sample.
    pub drift: f32,
    /// Scales every formant: >1 is a sped-up "chipmunk" sample, <1 a slowed-down one.
    pub formant: f32,
    pub vibrato: f32,
    pub breath: f32,
}

/// Fred again.. vocal chop: hard-gated, dry, bright.
pub const CHOP: VoxStyle = VoxStyle { attack: 0.005, release: 0.05, scoop: 0.0, drift: 0.0, formant: 1.06, vibrato: 0.0, breath: 0.35 };
/// A plainly sung line (voice-memo intimacy).
pub const SUNG: VoxStyle = VoxStyle { attack: 0.07, release: 0.35, scoop: -1.0, drift: 0.0, formant: 1.0, vibrato: 0.18, breath: 0.55 };
/// Burial ghost: pitched-up formants, airy, sinking pitch, long tails.
pub const GHOST: VoxStyle = VoxStyle { attack: 0.16, release: 0.9, scoop: 0.0, drift: -0.5, formant: 1.24, vibrato: 0.1, breath: 0.7 };

/// A formant-synthesised vocal note gliding from one vowel to another.
///
/// What keeps it from sounding robotic: a soft, spectrally tilted glottal source instead of a raw
/// buzz; pitch that wanders slowly and jitters cycle to cycle with matching amplitude shimmer;
/// wide formants; aspiration noise pulsed by each glottal cycle, plus unfiltered "air"; and
/// breath that leads the voice in at the start and outlasts it at the end.
#[allow(clippy::too_many_arguments)]
pub fn vox(buf: &mut Stereo, t0: f32, dur: f32, midi: f32, vel: f32, pan: f32, from: Vowel, to: Vowel, st: &VoxStyle, rng: &mut Rng) {
    const Q: [f32; 3] = [5.0, 7.0, 9.0];
    const GAIN: [f32; 3] = [1.0, 0.5, 0.28];
    let (fa, fb) = (from.formants(), to.formants());
    let mut filt = [Svf::default(); 3];
    let (mut tilt1, mut tilt2) = (OnePole::new(1400.0), OnePole::new(2800.0));
    let mut air = Svf::new(3200.0, 0.6);
    let (gl, gr) = pan_gains(pan);
    let held = (dur / st.attack).min(1.0);
    // Breath arrives before the voice (except on hard-gated chops) and outlasts it.
    let lead_in = (st.attack * 0.5).min(0.06);
    let mut ph = rng.f();
    let (mut wander, mut wander_to) = (0.0f32, rng.bi());
    let (mut jitter, mut shimmer) = (1.0f32, 1.0f32);
    let start = secs(t0);
    for n in 0..secs(dur + st.release * 1.3) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let env = if t < dur { (t / st.attack).min(1.0) } else { held * (-(t - dur) * 5.0 / st.release).exp() };
        let voicing = smoothstep((t - lead_in) / (st.attack * 0.6 + 0.005))
            * if t < dur { 1.0 } else { (-(t - dur) * 9.0 / st.release).exp() };
        let x = (t / dur).min(1.0);
        if n % 32 == 0 {
            let m = smoothstep(x);
            for k in 0..3 {
                filt[k].set(fa[k] * (fb[k] / fa[k]).powf(m) * st.formant, Q[k]);
            }
        }
        if n % 2048 == 0 {
            wander_to = rng.bi();
        }
        wander += (wander_to - wander) * 0.0004;
        let scoop = st.scoop * (1.0 - smoothstep(t / 0.09));
        let vib_rate = 5.2 + 0.4 * (TAU * 0.7 * t).sin();
        let vib = st.vibrato * smoothstep((t - 0.25) / 0.4) * (TAU * vib_rate * t).sin();
        let semis = scoop + st.drift * x + vib + wander * 0.09;
        let dt = midi_hz(midi + semis) / SR * jitter;
        ph += dt;
        if ph >= 1.0 {
            ph -= 1.0;
            jitter = 1.0 + rng.bi() * 0.004;
            shimmer = 1.0 + rng.bi() * 0.08;
        }
        let glottal = tilt2.lp(tilt1.lp(saw(ph, dt))) * shimmer;
        // Aspiration is loudest while the glottis is open (first half of the cycle).
        let noise = rng.bi() * if ph < 0.5 { 1.0 } else { 0.55 };
        let src = glottal * voicing * (1.0 - 0.35 * st.breath) + noise * st.breath * 1.4;
        let mut s = 0.0;
        for k in 0..3 {
            s += filt[k].bp(src) * GAIN[k] / Q[k];
        }
        s += air.hp(rng.bi()) * st.breath * 0.06;
        let out = s * env * vel * 3.0;
        buf.add(i, out * gl, out * gr);
    }
}

/// A vocal phrase: (start beat, length in beats, midi note, vowel from, vowel to).
pub type Phrase = &'static [(f32, f32, f32, Vowel, Vowel)];

/// Sings `phrase` starting at `t0`, skipping each note with probability `skip` (fragmentary ghosts).
#[allow(clippy::too_many_arguments)]
pub fn sing(buf: &mut Stereo, phrase: Phrase, t0: f32, beat: f32, transpose: f32, vel: f32, pan: f32, style: &VoxStyle, skip: f32, rng: &mut Rng) {
    for &(s, d, m, a, b) in phrase {
        if !rng.chance(skip) {
            vox(buf, t0 + s * beat, d * beat, m + transpose, vel, pan, a, b, style, rng);
        }
    }
}

// ---------------------------------------------------------------- keys & synths

/// Quadrature oscillator: a sine/cosine pair advanced by rotation instead of calling sin() per sample.
#[derive(Clone, Copy)]
struct Rotor {
    c: f32,
    s: f32,
    cw: f32,
    sw: f32,
}

impl Rotor {
    fn new(hz: f32, phase: f32) -> Self {
        let w = TAU * hz / SR;
        Self { c: (TAU * phase).cos(), s: (TAU * phase).sin(), cw: w.cos(), sw: w.sin() }
    }

    #[inline]
    fn tick(&mut self) -> f32 {
        let (c, s) = (self.c * self.cw - self.s * self.sw, self.s * self.cw + self.c * self.sw);
        self.c = c;
        self.s = s;
        s
    }

    fn renormalize(&mut self) {
        let m = (self.c * self.c + self.s * self.s).sqrt();
        self.c /= m;
        self.s /= m;
    }
}

/// Soft felt piano: stretched (inharmonic) partials on two detuned strings, a two-stage decay,
/// hammer-felt noise and a damper when the key is released.
pub fn piano(buf: &mut Stereo, t0: f32, dur: f32, midi: i32, vel: f32, pan: f32, rng: &mut Rng) {
    const B: f32 = 0.0004;
    let f = midi_hz(midi as f32);
    let sustain = (4.5 * (130.0 / f).sqrt()).clamp(0.9, 7.0);
    // (rotor, amplitude, fast decay per sample, slow decay per sample) for both strings of every partial.
    let mut parts = Vec::new();
    for n in 1..=12 {
        let nf = n as f32;
        let hz = nf * f * (1.0 + B * nf * nf).sqrt();
        if hz > 9000.0 {
            break;
        }
        let amp = nf.powf(-1.3) * (-(nf - 1.0) * 0.35 * (1.25 - vel)).exp();
        let rate = (1.0 + 0.45 * (nf - 1.0)) / sustain;
        let (fast, slow) = ((-rate * 2.4 / SR).exp(), (-rate * 0.5 / SR).exp());
        for cents in [-0.8f32, 0.9] {
            parts.push((Rotor::new(hz * 2f32.powf(cents / 1200.0), rng.f()), amp, fast, slow, 1.0f32, 1.0f32));
        }
    }
    let mut hammer = OnePole::new(1800.0);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    for n in 0..secs((dur + 0.45).min(sustain * 3.0)) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let mut s = 0.0;
        for p in parts.iter_mut() {
            p.4 *= p.2;
            p.5 *= p.3;
            s += p.0.tick() * p.1 * (0.55 * p.4 + 0.45 * p.5);
        }
        if n % 1024 == 0 {
            parts.iter_mut().for_each(|p| p.0.renormalize());
        }
        let damper = if t < dur { 1.0 } else { (-(t - dur) * 12.0).exp() };
        let felt = hammer.lp(rng.bi()) * (-t * 90.0).exp() * 0.3;
        let out = (s * (t / 0.003).min(1.0) * damper + felt) * vel * 0.22;
        buf.add(i, out * gl, out * gr);
    }
}

/// Wide 7-voice supersaw chord — Fred again..'s euphoric drop swell.
#[allow(clippy::too_many_arguments)]
pub fn supersaw(buf: &mut Stereo, t0: f32, dur: f32, notes: &[i32], vel: f32, cutoff: f32, attack: f32, rng: &mut Rng) {
    const CENTS: [f32; 7] = [-21.0, -12.5, -5.0, 0.0, 5.5, 12.0, 20.0];
    const PAN: [f32; 7] = [-0.9, -0.6, -0.25, 0.0, 0.3, 0.6, 0.9];
    const RELEASE: f32 = 0.5;
    let gains = PAN.map(pan_gains);
    let mut osc: Vec<(f32, f32, usize)> = Vec::new(); // (phase, dt, voice)
    for &m in notes {
        for v in 0..7 {
            osc.push((rng.f(), midi_hz(m as f32) * 2f32.powf(CENTS[v] / 1200.0) / SR, v));
        }
    }
    let (mut fl, mut fr) = (Svf::new(cutoff, 0.8), Svf::new(cutoff, 0.8));
    let held = smoothstep(dur / attack);
    let norm = 0.05 / (notes.len() as f32).sqrt();
    let start = secs(t0);
    for n in 0..secs(dur + RELEASE) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let env = if t < dur { smoothstep(t / attack) } else { held * (-(t - dur) * 8.0).exp() };
        if n % 32 == 0 {
            let c = cutoff * (0.5 + 0.5 * env);
            fl.set(c, 0.8);
            fr.set(c * 1.04, 0.8);
        }
        let (mut l, mut r) = (0.0, 0.0);
        for o in osc.iter_mut() {
            o.0 += o.1;
            if o.0 >= 1.0 {
                o.0 -= 1.0;
            }
            let s = saw(o.0, o.1);
            l += s * gains[o.2].0;
            r += s * gains[o.2].1;
        }
        let amp = env * vel * norm;
        buf.add(i, fl.lp(l) * amp, fr.lp(r) * amp);
    }
}

/// Techno chord stab: two detuned saws per note through a lowpass that snaps shut.
#[allow(clippy::too_many_arguments)]
pub fn stab(buf: &mut Stereo, t0: f32, notes: &[i32], vel: f32, bright: f32, decay: f32, pan: f32, rng: &mut Rng) {
    let mut osc: Vec<(f32, f32)> = Vec::new();
    for &m in notes {
        for cents in [-9.0f32, 8.0] {
            osc.push((rng.f(), midi_hz(m as f32) * 2f32.powf(cents / 1200.0) / SR));
        }
    }
    let mut filt = Svf::new(bright, 1.6);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    for n in 0..secs(decay * 6.0) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        if n % 16 == 0 {
            filt.set(150.0 + bright * (-t / (decay * 0.4)).exp(), 1.6);
        }
        let mut s = 0.0;
        for o in osc.iter_mut() {
            o.0 += o.1;
            if o.0 >= 1.0 {
                o.0 -= 1.0;
            }
            s += saw(o.0, o.1);
        }
        let env = (t / 0.002).min(1.0) * (-t / decay).exp();
        let out = filt.lp(s) * env * vel * 0.12;
        buf.add(i, out * gl, out * gr);
    }
}

// ---------------------------------------------------------------- bass

/// Reese / wobble bass: three detuned saws through a resonant lowpass whose cutoff an LFO sweeps
/// between `lo` and `hi` (`lfo_hz` = 0 gives Burial's slowly breathing reese), saturated, with a
/// clean mono sine sub underneath.
#[allow(clippy::too_many_arguments)]
pub fn reese(buf: &mut Stereo, t0: f32, dur: f32, midi: f32, vel: f32, lo: f32, hi: f32, lfo_hz: f32, drive: f32, rng: &mut Rng) {
    const CENTS: [f32; 3] = [-16.0, 0.0, 15.0];
    const RELEASE: f32 = 0.08;
    let f = midi_hz(midi);
    let dts = CENTS.map(|c| f * 2f32.powf(c / 1200.0) / SR);
    let mut ph = [rng.f(), rng.f(), rng.f()];
    let mut sub = 0.0f32;
    let (mut fl, mut fr) = (Svf::new(lo, 2.2), Svf::new(lo, 2.2));
    let (mut hl, mut hr) = (OnePole::new(70.0), OnePole::new(70.0));
    let start = secs(t0);
    for n in 0..secs(dur + RELEASE) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let env = (t / 0.008).min(1.0) * if t < dur { 1.0 } else { (-(t - dur) / RELEASE * 5.0).exp() };
        if n % 16 == 0 {
            let lfo = if lfo_hz > 0.0 { 0.5 - 0.5 * (TAU * lfo_hz * t).cos() } else { 0.35 + 0.25 * (TAU * 0.23 * t).sin() };
            let c = lo * (hi / lo).powf(lfo);
            fl.set(c, 2.2);
            fr.set(c * 1.03, 2.2);
        }
        let mut s = [0.0f32; 3];
        for k in 0..3 {
            ph[k] += dts[k];
            if ph[k] >= 1.0 {
                ph[k] -= 1.0;
            }
            s[k] = saw(ph[k], dts[k]);
        }
        sub = (sub + f / SR).fract();
        let body = (sub * TAU).sin() * 0.8;
        let l = soft_clip(fl.lp(hl.hp(s[0] * 0.7 + s[1] * 0.5 + s[2] * 0.3)), drive) * 0.45;
        let r = soft_clip(fr.lp(hr.hp(s[0] * 0.3 + s[1] * 0.5 + s[2] * 0.7)), drive) * 0.45;
        let a = env * vel * 0.5;
        buf.add(i, (l + body) * a, (r + body) * a);
    }
}

/// Pure sine sub that can glide in from another pitch, with a touch of 2nd harmonic so small speakers hear it.
pub fn sub(buf: &mut Stereo, t0: f32, dur: f32, midi: f32, from: f32, glide: f32, vel: f32) {
    const RELEASE: f32 = 0.07;
    let mut ph = 0.0f32;
    let start = secs(t0);
    for n in 0..secs(dur + RELEASE) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let m = if glide > 0.0 { from + (midi - from) * smoothstep(t / glide) } else { midi };
        ph = (ph + midi_hz(m) / SR).fract();
        let env = (t / 0.005).min(1.0) * if t < dur { 1.0 } else { (-(t - dur) / RELEASE * 5.0).exp() };
        let s = soft_clip((ph * TAU).sin() + 0.12 * (ph * 2.0 * TAU).sin(), 1.3);
        let out = s * env * vel * 0.5;
        buf.add(i, out * 0.707, out * 0.707);
    }
}

// ---------------------------------------------------------------- drums & foley

/// Techno kick: a fast pitch dive into a long ~46 Hz tail, driven hard.
pub fn techno_kick(buf: &mut Stereo, t0: f32, vel: f32, tail: f32, rng: &mut Rng) {
    let mut hp = OnePole::new(2500.0);
    let start = secs(t0);
    let mut ph = 0.0f32;
    for n in 0..secs(tail * 5.0) {
        let i = start + n;
        let t = n as f32 / SR;
        ph += (46.0 + 170.0 * (-t * 40.0).exp() + 25.0 * (-t * 8.0).exp()) / SR;
        let body = (ph * TAU).sin() * (-t / tail).exp();
        let click = hp.hp(rng.bi()) * (-t * 350.0).exp() * 0.5;
        let out = ((body + click) * 2.4).tanh() * vel * 0.9;
        buf.add(i, out * 0.707, out * 0.707);
    }
}

/// Burial-style snare: a crunchy noise crack with a metallic, slightly out-of-tune "dropped tin" ring.
pub fn clank(buf: &mut Stereo, t0: f32, vel: f32, pitch: f32, pan: f32, rng: &mut Rng) {
    const RING: [(f32, f32, f32); 4] = [(1.0, 0.45, 20.0), (1.52, 0.32, 26.0), (2.17, 0.22, 34.0), (2.83, 0.14, 45.0)];
    let mut body = Svf::new(1900.0, 0.9);
    let mut sizzle = Svf::new(6500.0, 0.7);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    for n in 0..secs(0.4) {
        let t = n as f32 / SR;
        let noise = body.bp(rng.bi()) * (-t * 20.0).exp() * 1.1 + sizzle.hp(rng.bi()) * (-t * 45.0).exp() * 0.45;
        let ring: f32 = RING.iter().map(|&(r, a, d)| (TAU * pitch * r * t).sin() * a * (-t * d).exp()).sum();
        let out = ((noise + ring) * 1.8).tanh() * vel * 0.7;
        buf.add(start + n, out * gl, out * gr);
    }
}

/// Rain on a window over a distant city rumble: sparse droplets, a soft wash and low traffic swell.
pub fn rain(buf: &mut Stereo, level: Curve, rng: &mut Rng) {
    let (mut dl, mut dr) = (Svf::new(3200.0, 0.9), Svf::new(2800.0, 0.9));
    let (mut wl, mut wr) = (Svf::new(1800.0, 0.5), Svf::new(2000.0, 0.5));
    let (mut r1, mut r2) = (OnePole::new(110.0), OnePole::new(110.0));
    let (mut drop_l, mut drop_r) = (0.0f32, 0.0f32);
    let mut lev = 0.0;
    for i in 0..buf.len() {
        if i % 256 == 0 {
            lev = level(i as f32 / SR);
        }
        if lev <= 0.0 {
            continue;
        }
        if rng.chance(0.0025 * lev) {
            drop_l = rng.range(0.03, 0.16);
        }
        if rng.chance(0.0025 * lev) {
            drop_r = rng.range(0.03, 0.16);
        }
        drop_l *= 0.93;
        drop_r *= 0.93;
        let t = i as f32 / SR;
        let swell = 0.6 + 0.4 * (TAU * 0.045 * t).sin();
        let rumble = r2.lp(r1.lp(rng.bi())) * 0.5 * swell;
        let l = dl.bp(rng.bi() * drop_l) + wl.bp(rng.bi()) * 0.05 + rumble;
        let r = dr.bp(rng.bi() * drop_r) + wr.bp(rng.bi()) * 0.05 + rumble;
        buf.add(i, l * lev * 0.25, r * lev * 0.25);
    }
}

// ---------------------------------------------------------------- bus processing

/// Time-varying 12 dB/oct filter over a whole bus: sweeps, "heard through the wall" muffling.
pub fn filter_bus(buf: &mut Stereo, cutoff: Curve, q: f32, highpass: bool) {
    let (mut fl, mut fr) = (Svf::new(cutoff(0.0), q), Svf::new(cutoff(0.0), q));
    for i in 0..buf.len() {
        if i % 64 == 0 {
            let c = cutoff(i as f32 / SR);
            fl.set(c, q);
            fr.set(c, q);
        }
        let (l, r) = (fl.process(buf.l[i]), fr.process(buf.r[i]));
        if highpass {
            buf.l[i] = l.2;
            buf.r[i] = r.2;
        } else {
            buf.l[i] = l.0;
            buf.r[i] = r.0;
        }
    }
}

/// Gentle bus saturation.
pub fn saturate(buf: &mut Stereo, drive: f32) {
    for x in buf.l.iter_mut().chain(buf.r.iter_mut()) {
        *x = soft_clip(*x, drive);
    }
}

/// Fade the bus to silence between two times (seconds), with an equal-power curve.
pub fn fade_out(buf: &mut Stereo, from: f32, to: f32) {
    let (a, b) = (secs(from), secs(to));
    for i in a..buf.len() {
        let g = if i >= b { 0.0 } else { (1.0 - (i - a) as f32 / (b - a) as f32).sqrt() };
        buf.l[i] *= g;
        buf.r[i] *= g;
    }
}

