//! Core DSP building blocks: buffers, oscillators, filters, delay, reverb and lo-fi processing.

use std::f32::consts::{PI, TAU};

pub const SR: f32 = 44_100.0;

/// Seconds -> sample count.
pub fn secs(t: f32) -> usize {
    (t.max(0.0) * SR).round() as usize
}

pub fn midi_hz(m: f32) -> f32 {
    440.0 * 2f32.powf((m - 69.0) / 12.0)
}

/// Equal-power pan law; -1 = hard left, 1 = hard right.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (a.cos(), a.sin())
}

pub fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Piecewise-linear automation lane over (x, value) breakpoints.
pub fn automation(points: &[(f32, f32)], x: f32) -> f32 {
    if x <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        if x <= w[1].0 {
            let t = (x - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * t;
        }
    }
    points[points.len() - 1].1
}

/// Deterministic xorshift RNG so every render of the song is identical.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [-1, 1).
    pub fn bi(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

pub struct Stereo {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Stereo {
    pub fn new(len: usize) -> Self {
        Self { l: vec![0.0; len], r: vec![0.0; len] }
    }

    pub fn len(&self) -> usize {
        self.l.len()
    }

    #[inline]
    pub fn add(&mut self, i: usize, l: f32, r: f32) {
        if i < self.l.len() {
            self.l[i] += l;
            self.r[i] += r;
        }
    }

    pub fn mix_from(&mut self, other: &Stereo, gain: f32) {
        let n = self.len().min(other.len());
        for i in 0..n {
            self.l[i] += other.l[i] * gain;
            self.r[i] += other.r[i] * gain;
        }
    }

    pub fn scale(&mut self, g: f32) {
        self.l.iter_mut().chain(self.r.iter_mut()).for_each(|x| *x *= g);
    }

    pub fn peak(&self) -> f32 {
        self.l.iter().chain(self.r.iter()).fold(0.0f32, |m, x| m.max(x.abs()))
    }

    /// RMS measured only where the bus is actually sounding, so sparse parts are judged fairly.
    pub fn active_rms(&self) -> f32 {
        let (mut sum, mut n) = (0f64, 0u64);
        for i in 0..self.len() {
            let (l, r) = (self.l[i], self.r[i]);
            if l.abs().max(r.abs()) > 1e-4 {
                sum += ((l * l + r * r) * 0.5) as f64;
                n += 1;
            }
        }
        if n == 0 { 0.0 } else { (sum / n as f64).sqrt() as f32 }
    }

    pub fn normalize_rms(&mut self, target: f32) {
        let rms = self.active_rms();
        if rms > 0.0 {
            self.scale(target / rms);
        }
    }
}

// ---------------------------------------------------------------- oscillators

#[inline]
pub fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Band-limited sawtooth from a [0,1) phase.
#[inline]
pub fn saw(phase: f32, dt: f32) -> f32 {
    2.0 * phase - 1.0 - poly_blep(phase, dt)
}

#[inline]
pub fn tri(phase: f32) -> f32 {
    4.0 * (phase - 0.5).abs() - 1.0
}

#[inline]
pub fn soft_clip(x: f32, drive: f32) -> f32 {
    (x * drive).tanh() / drive.tanh()
}

// ---------------------------------------------------------------- filters

/// Cytomic (Andy Simper) trapezoidal state-variable filter.
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl Svf {
    pub fn new(cutoff: f32, q: f32) -> Self {
        let mut s = Self::default();
        s.set(cutoff, q);
        s
    }

    pub fn set(&mut self, cutoff: f32, q: f32) {
        let fc = cutoff.clamp(20.0, SR * 0.45);
        let g = (PI * fc / SR).tan();
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    /// Returns (lowpass, bandpass, highpass).
    #[inline]
    pub fn process(&mut self, v0: f32) -> (f32, f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, v0 - self.k * v1 - v2)
    }

    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.process(x).0
    }

    #[inline]
    pub fn bp(&mut self, x: f32) -> f32 {
        self.process(x).1
    }

    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        self.process(x).2
    }
}

#[derive(Clone, Copy)]
pub struct OnePole {
    a: f32,
    z: f32,
}

impl OnePole {
    pub fn new(cutoff: f32) -> Self {
        Self { a: (-TAU * cutoff / SR).exp(), z: 0.0 }
    }

    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.z = x + (self.z - x) * self.a;
        self.z
    }

    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        x - self.lp(x)
    }
}

// ---------------------------------------------------------------- bus effects

/// Stereo ping-pong delay with darkening, thinning feedback (classic dub-tape character).
pub fn pingpong(input: &Stereo, delay_s: f32, feedback: f32, tone_hz: f32) -> Stereo {
    let d = secs(delay_s).max(1);
    let (mut bl, mut br) = (vec![0f32; d], vec![0f32; d]);
    let (mut lpl, mut lpr) = (OnePole::new(tone_hz), OnePole::new(tone_hz));
    let mut hp = OnePole::new(280.0);
    let mut out = Stereo::new(input.len());
    let mut idx = 0;
    for i in 0..input.len() {
        let (dl, dr) = (bl[idx], br[idx]);
        let x = hp.hp((input.l[i] + input.r[i]) * 0.5);
        bl[idx] = x + lpl.lp(dr) * feedback;
        br[idx] = lpr.lp(dl) * feedback;
        out.l[i] = dl;
        out.r[i] = dr;
        idx += 1;
        if idx == d {
            idx = 0;
        }
    }
    out
}

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], idx: 0, store: 0.0 }
    }

    #[inline]
    fn process(&mut self, x: f32, fb: f32, damp: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.idx] = x + self.store * fb;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], idx: 0 }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.idx];
        self.buf[self.idx] = x + b * 0.5;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
        b - x
    }
}

/// Freeverb-style stereo reverb; returns the wet signal only.
pub fn reverb(input: &Stereo, size: f32, feedback: f32, damp: f32, predelay_s: f32) -> Stereo {
    const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
    const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
    const SPREAD: usize = 23;
    let scaled = |n: usize| (n as f32 * size) as usize;
    let mut cl: Vec<Comb> = COMBS.iter().map(|&n| Comb::new(scaled(n))).collect();
    let mut cr: Vec<Comb> = COMBS.iter().map(|&n| Comb::new(scaled(n + SPREAD))).collect();
    let mut al: Vec<Allpass> = ALLPASSES.iter().map(|&n| Allpass::new(scaled(n))).collect();
    let mut ar: Vec<Allpass> = ALLPASSES.iter().map(|&n| Allpass::new(scaled(n + SPREAD))).collect();
    let pd = secs(predelay_s).max(1);
    let mut pre = vec![0f32; pd];
    let mut pidx = 0;
    let mut hp = OnePole::new(180.0);
    let mut out = Stereo::new(input.len());
    for i in 0..input.len() {
        let x_in = hp.hp((input.l[i] + input.r[i]) * 0.5);
        let x = pre[pidx] * 0.03;
        pre[pidx] = x_in;
        pidx = (pidx + 1) % pd;
        let (mut l, mut r) = (0.0, 0.0);
        for c in cl.iter_mut() {
            l += c.process(x, feedback, damp);
        }
        for c in cr.iter_mut() {
            r += c.process(x, feedback, damp);
        }
        for a in al.iter_mut() {
            l = a.process(l);
        }
        for a in ar.iter_mut() {
            r = a.process(r);
        }
        out.l[i] = l;
        out.r[i] = r;
    }
    out
}

/// Sample-rate reduction + bit crush + tape-ish lowpass, blended by a time-varying mix.
pub fn lofi(buf: &mut Stereo, bits: f32, hold: usize, mix: fn(f32) -> f32) {
    let q = 2f32.powf(bits - 1.0);
    let (mut hl, mut hr) = (0.0, 0.0);
    // Steep-ish tape lowpass so the sample-and-hold images (SR / hold) don't whine.
    let (mut sl, mut sr) = (Svf::new(5200.0, 0.7), Svf::new(5200.0, 0.7));
    let (mut tl, mut tr) = (OnePole::new(7000.0), OnePole::new(7000.0));
    let mut m = 0.0;
    for i in 0..buf.len() {
        if i % hold == 0 {
            hl = (buf.l[i] * q).round() / q;
            hr = (buf.r[i] * q).round() / q;
        }
        if i % 256 == 0 {
            m = mix(i as f32 / SR).clamp(0.0, 1.0);
        }
        let (wl, wr) = (tl.lp(sl.lp(hl)), tr.lp(sr.lp(hr)));
        buf.l[i] = buf.l[i] * (1.0 - m) + wl * m;
        buf.r[i] = buf.r[i] * (1.0 - m) + wr * m;
    }
}

/// Sidechain gain curve: dips under every kick then recovers — the house "pump".
pub fn duck_envelope(len: usize, kicks: &[f32], depth: f32, release_s: f32) -> Vec<f32> {
    let mut env = vec![1.0f32; len];
    let rel = secs(release_s);
    let att = secs(0.004).max(1);
    for &k in kicks {
        let start = secs(k);
        for n in 0..rel {
            let i = start + n;
            if i >= len {
                break;
            }
            let x = n as f32 / rel as f32;
            let a = (n as f32 / att as f32).min(1.0);
            let g = 1.0 - depth * a * (1.0 - x) * (1.0 - x);
            env[i] = env[i].min(g);
        }
    }
    env
}

pub fn apply_duck(buf: &mut Stereo, env: &[f32], amount: f32) {
    for i in 0..buf.len().min(env.len()) {
        let g = 1.0 - amount * (1.0 - env[i]);
        buf.l[i] *= g;
        buf.r[i] *= g;
    }
}

/// Slows the tape to a halt from `start` over `dur` samples, silence afterwards.
pub fn tape_stop(buf: &mut Stereo, start: usize, dur: usize) {
    let end = (start + dur).min(buf.len());
    let src_l = buf.l[start..end].to_vec();
    let src_r = buf.r[start..end].to_vec();
    let mut pos = 0.0f32;
    for n in 0..(end - start) {
        let speed = (1.0 - n as f32 / dur as f32).powf(1.6);
        let i0 = pos as usize;
        let fr = pos - i0 as f32;
        let i1 = (i0 + 1).min(src_l.len() - 1);
        let fade = 1.0 - (n as f32 / dur as f32).powf(3.0);
        buf.l[start + n] = (src_l[i0] * (1.0 - fr) + src_l[i1] * fr) * fade;
        buf.r[start + n] = (src_r[i0] * (1.0 - fr) + src_r[i1] * fr) * fade;
        pos += speed;
    }
    for i in end..buf.len() {
        buf.l[i] = 0.0;
        buf.r[i] = 0.0;
    }
}
