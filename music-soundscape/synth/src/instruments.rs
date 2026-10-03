//! Instrument voices. Each function renders one note or hit additively into a stereo buffer.

use crate::dsp::*;
use std::f32::consts::TAU;

/// A time-indexed automation lane (seconds -> value).
pub type Curve = fn(f32) -> f32;

/// Boards of Canada style pad: five detuned saws drifting on wobbly tape, plus a sine body.
#[allow(clippy::too_many_arguments)]
pub fn pad_note(buf: &mut Stereo, t0: f32, dur: f32, midi: i32, vel: f32, cutoff: Curve, wow: Curve, rng: &mut Rng) {
    const DETUNE: [f32; 5] = [-11.0, -4.5, 0.0, 5.0, 12.5];
    const PAN: [f32; 5] = [-0.85, -0.35, 0.0, 0.4, 0.8];
    const ATTACK: f32 = 1.6;
    const RELEASE: f32 = 3.2;
    let base = midi_hz(midi as f32);
    let ratios = DETUNE.map(|c| 2f32.powf(c / 1200.0));
    let gains = PAN.map(pan_gains);
    let mut phases = [0f32; 5];
    for p in phases.iter_mut() {
        *p = rng.f();
    }
    let mut body = 0.0f32;
    let (mut fl, mut fr) = (Svf::new(800.0, 0.85), Svf::new(800.0, 0.85));
    let start = secs(t0);
    let held = smoothstep(dur / ATTACK);
    for n in 0..secs(dur + RELEASE) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let ta = i as f32 / SR;
        let env = if t < dur {
            smoothstep(t / ATTACK)
        } else {
            let r = (t - dur) / RELEASE;
            held * (-r * 4.0).exp() * (1.0 - r)
        };
        if n % 32 == 0 {
            let c = cutoff(ta) * (0.75 + 0.35 * env);
            fl.set(c, 0.85);
            fr.set(c, 0.85);
        }
        let w = wow(ta);
        let (mut l, mut r) = (0.0, 0.0);
        for v in 0..5 {
            let dt = base * ratios[v] * w / SR;
            phases[v] += dt;
            if phases[v] >= 1.0 {
                phases[v] -= 1.0;
            }
            let s = saw(phases[v], dt);
            l += s * gains[v].0;
            r += s * gains[v].1;
        }
        body += base * w / SR;
        if body >= 1.0 {
            body -= 1.0;
        }
        let sine = (body * TAU).sin() * 0.9;
        let amp = env * vel * 0.05;
        buf.add(i, (fl.lp(l) + sine) * amp, (fr.lp(r) + sine) * amp);
    }
}

/// Karplus-Strong plucked string with fractional delay — a dusty harp / guitar.
#[allow(clippy::too_many_arguments)]
pub fn pluck(buf: &mut Stereo, t0: f32, midi: i32, vel: f32, pan: f32, decay: f32, bright: f32, rng: &mut Rng) {
    let f = midi_hz(midi as f32);
    let period = SR / f;
    let size = period.ceil() as usize + 4;
    let mut line = vec![0f32; size];
    let delay = period - 0.5; // the averaging loop filter adds half a sample
    let g = 0.001f32.powf(1.0 / (f * decay));
    let mut ex_lp = OnePole::new(bright);
    let mut dc = OnePole::new(30.0);
    let n_ex = period as usize;
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    let (mut w, mut prev) = (0usize, 0.0f32);
    for n in 0..secs(decay * 1.1 + 0.05) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let mut rp = w as f32 - delay;
        if rp < 0.0 {
            rp += size as f32;
        }
        let i0 = rp as usize % size;
        let i1 = (i0 + 1) % size;
        let fr = rp - rp.floor();
        let y = line[i0] * (1.0 - fr) + line[i1] * fr;
        let fb = (y + prev) * 0.5 * g;
        prev = y;
        let ex = if n < n_ex { ex_lp.lp(rng.bi()) * 1.6 } else { 0.0 };
        let s = ex + fb;
        line[w] = s;
        w = (w + 1) % size;
        let out = dc.hp(s) * vel * 0.6;
        buf.add(i, out * gl, out * gr);
    }
}

/// Four Tet style kalimba / thumb piano: inharmonic tine partials with a woody click.
pub fn kalimba(buf: &mut Stereo, t0: f32, midi: i32, vel: f32, pan: f32, decay: f32, rng: &mut Rng) {
    // (frequency ratio, amplitude, decay speed multiplier)
    const PARTIALS: [(f32, f32, f32); 4] = [(1.0, 1.0, 1.0), (2.0, 0.12, 2.2), (5.4, 0.22, 4.0), (8.9, 0.07, 7.0)];
    let f = midi_hz(midi as f32);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    let phase0: [f32; 4] = [0.0, rng.f(), rng.f(), rng.f()];
    let k = 5.0 / decay;
    for n in 0..secs(decay * 1.2) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let mut s = 0.0;
        for (p, &(ratio, amp, speed)) in PARTIALS.iter().enumerate() {
            if f * ratio < SR * 0.45 {
                s += amp * (TAU * (f * ratio * t + phase0[p])).sin() * (-t * k * speed).exp();
            }
        }
        let attack = (t / 0.002).min(1.0);
        let click = rng.bi() * (-t * 900.0).exp() * 0.25;
        let out = (s * attack + click) * vel * 0.5;
        buf.add(i, out * gl, out * gr);
    }
}

/// Warm sub bass: sine fundamental with a filtered saw for growl, gently saturated.
pub fn bass_note(buf: &mut Stereo, t0: f32, dur: f32, midi: i32, vel: f32, drive: f32) {
    const ATTACK: f32 = 0.006;
    const RELEASE: f32 = 0.12;
    let f = midi_hz(midi as f32);
    let dt = f / SR;
    let mut lp = Svf::new(420.0, 0.9);
    let (mut ph, mut sp) = (0.0f32, 0.0f32);
    let start = secs(t0);
    for n in 0..secs(dur + RELEASE) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let env = (t / ATTACK).min(1.0) * if t < dur { 1.0 } else { (-(t - dur) * 30.0).exp() };
        ph = (ph + dt).fract();
        sp = (sp + dt).fract();
        let s = (ph * TAU).sin() + 0.4 * lp.lp(saw(sp, dt));
        let out = soft_clip(s, drive) * env * vel * 0.5;
        buf.add(i, out * 0.707, out * 0.707);
    }
}

/// Ben Böhmer style melancholic lead: detuned saws + triangle, filter envelope, delayed vibrato.
#[allow(clippy::too_many_arguments)]
pub fn lead_note(buf: &mut Stereo, t0: f32, dur: f32, midi: i32, vel: f32, bright: f32, pan: f32, wow: Curve) {
    const ATTACK: f32 = 0.04;
    const RELEASE: f32 = 0.7;
    let base = midi_hz(midi as f32);
    let (d1, d2) = (2f32.powf(7.0 / 1200.0), 2f32.powf(-6.0 / 1200.0));
    let (gl, gr) = pan_gains(pan);
    let mut filt = Svf::new(bright, 1.1);
    let (mut p1, mut p2, mut p3) = (0.0f32, 0.37f32, 0.0f32);
    let level = |t: f32| (t / ATTACK).min(1.0) * (0.8 + 0.2 * (-t * 2.5).exp());
    let held = level(dur);
    let start = secs(t0);
    for n in 0..secs(dur + RELEASE) {
        let i = start + n;
        if i >= buf.len() {
            break;
        }
        let t = n as f32 / SR;
        let env = if t < dur { level(t) } else { held * (-(t - dur) * 6.0).exp() };
        let vib_depth = smoothstep((t - 0.3) / 0.6) * 0.15;
        let vib = 2f32.powf(vib_depth * (TAU * 5.2 * t).sin() / 12.0);
        let dt = base * vib * wow(i as f32 / SR) / SR;
        p1 = (p1 + dt * d1).fract();
        p2 = (p2 + dt * d2).fract();
        p3 = (p3 + dt).fract();
        if n % 16 == 0 {
            filt.set(200.0 + bright * (0.55 + 0.9 * (-t * 3.5).exp()) * (0.6 + 0.4 * vel), 1.1);
        }
        let s = filt.lp(0.5 * saw(p1, dt * d1) + 0.5 * saw(p2, dt * d2) + 0.6 * tri(p3)) * env * vel * 0.3;
        buf.add(i, s * gl, s * gr);
    }
}

// ---------------------------------------------------------------- drums

pub fn kick(buf: &mut Stereo, t0: f32, vel: f32, dusty: bool, rng: &mut Rng) {
    let decay = if dusty { 7.5 } else { 5.5 };
    let start = secs(t0);
    let mut ph = 0.0f32;
    for n in 0..secs(0.7) {
        let i = start + n;
        let t = n as f32 / SR;
        let f = 44.0 + 130.0 * (-t * 32.0).exp();
        ph += f / SR;
        let body = (ph * TAU).sin() * (-t * decay).exp();
        let click = rng.bi() * (-t * 400.0).exp() * 0.25;
        let s = if dusty { (body + click) * 2.2 } else { (body + click) * 1.3 };
        let out = s.tanh() * vel * if dusty { 0.75 } else { 0.9 };
        buf.add(i, out * 0.707, out * 0.707);
    }
}

pub fn snare(buf: &mut Stereo, t0: f32, vel: f32, dusty: bool, rng: &mut Rng) {
    let mut bp = Svf::new(if dusty { 1600.0 } else { 2200.0 }, 0.7);
    let noise_decay = if dusty { 16.0 } else { 13.0 };
    let (gl, gr) = pan_gains(0.05);
    let start = secs(t0);
    for n in 0..secs(0.4) {
        let t = n as f32 / SR;
        let tone = (TAU * 185.0 * t).sin() * (-t * 28.0).exp() * 0.6 + (TAU * 330.0 * t).sin() * (-t * 35.0).exp() * 0.25;
        let noise = bp.bp(rng.bi()) * (-t * noise_decay).exp() * 1.4;
        let mut s = tone + noise;
        if dusty {
            s = (s * 1.8).tanh();
        }
        let out = s * vel * 0.8;
        buf.add(start + n, out * gl, out * gr);
    }
}

pub fn clap(buf: &mut Stereo, t0: f32, vel: f32, rng: &mut Rng) {
    const BURSTS: [f32; 3] = [0.0, 0.011, 0.023];
    let (mut fl, mut fr) = (Svf::new(1300.0, 1.2), Svf::new(1400.0, 1.2));
    let start = secs(t0);
    for n in 0..secs(0.35) {
        let t = n as f32 / SR;
        let mut env = BURSTS.iter().filter(|&&o| t >= o).map(|&o| (-(t - o) * 180.0).exp()).fold(0.0, f32::max);
        if t > 0.023 {
            env = env.max((-(t - 0.023) * 14.0).exp() * 0.6);
        }
        let g = env * vel * 1.6;
        buf.add(start + n, fl.bp(rng.bi()) * g, fr.bp(rng.bi()) * g);
    }
}

pub fn hat(buf: &mut Stereo, t0: f32, vel: f32, open: bool, pan: f32, rng: &mut Rng) {
    let decay = if open { 0.22 } else { 0.04 };
    let mut hp = Svf::new(7000.0, 0.7);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    for n in 0..secs(decay * 5.0) {
        let t = n as f32 / SR;
        let out = hp.hp(rng.bi()) * (-t / decay).exp() * vel * 0.6;
        buf.add(start + n, out * gl, out * gr);
    }
}

/// Wooden click / rim — the little organic percussion Four Tet scatters everywhere.
pub fn rim(buf: &mut Stereo, t0: f32, vel: f32, freq: f32, pan: f32, rng: &mut Rng) {
    let mut bp = Svf::new(freq * 2.1, 4.0);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    for n in 0..secs(0.12) {
        let t = n as f32 / SR;
        let s = (TAU * freq * t).sin() * (-t * 55.0).exp() + bp.bp(rng.bi()) * (-t * 120.0).exp() * 0.8;
        let out = s * vel * 0.5;
        buf.add(start + n, out * gl, out * gr);
    }
}

pub fn shaker(buf: &mut Stereo, t0: f32, vel: f32, pan: f32, rng: &mut Rng) {
    let mut bp = Svf::new(5500.0, 1.0);
    let (gl, gr) = pan_gains(pan);
    let start = secs(t0);
    for n in 0..secs(0.12) {
        let t = n as f32 / SR;
        let env = (t / 0.012).min(1.0) * (-t * 35.0).exp();
        let out = bp.bp(rng.bi()) * env * vel * 1.2;
        buf.add(start + n, out * gl, out * gr);
    }
}

/// Filtered noise sweep building into a drop.
pub fn riser(buf: &mut Stereo, t0: f32, dur: f32, vel: f32, rng: &mut Rng) {
    let (mut fl, mut fr) = (Svf::new(300.0, 2.0), Svf::new(300.0, 2.0));
    let start = secs(t0);
    for n in 0..secs(dur) {
        let x = n as f32 / secs(dur) as f32;
        if n % 32 == 0 {
            let c = 300.0 * 20f32.powf(x);
            fl.set(c, 2.0);
            fr.set(c * 1.08, 2.0);
        }
        let g = x.powf(2.2) * vel;
        buf.add(start + n, fl.bp(rng.bi()) * g, fr.bp(rng.bi()) * g);
    }
}

/// Low boom plus a wide noise wash marking the downbeat of a drop.
pub fn impact(buf: &mut Stereo, t0: f32, vel: f32, rng: &mut Rng) {
    let (mut ll, mut lr) = (OnePole::new(3000.0), OnePole::new(3000.0));
    let start = secs(t0);
    let mut ph = 0.0f32;
    for n in 0..secs(3.0) {
        let t = n as f32 / SR;
        ph += (38.0 + 20.0 * (-t * 4.0).exp()) / SR;
        let boom = (ph * TAU).sin() * (-t * 1.8).exp() * 0.8;
        let wash = (-t * 3.0).exp() * 0.35;
        buf.add(start + n, (boom + ll.lp(rng.bi()) * wash) * vel, (boom + lr.lp(rng.bi()) * wash) * vel);
    }
}

/// Record-player hiss and crackle, scaled by a time-varying level.
pub fn vinyl(buf: &mut Stereo, level: Curve, rng: &mut Rng) {
    let (mut lp, mut hp) = (OnePole::new(5000.0), OnePole::new(300.0));
    let mut pop = 0.0f32;
    let mut lev = 0.0;
    for i in 0..buf.len() {
        if i % 256 == 0 {
            lev = level(i as f32 / SR);
        }
        if rng.chance(0.00012 * (0.5 + lev)) {
            pop = rng.range(0.02, 0.12) * lev;
        }
        pop *= 0.93;
        let hiss = hp.hp(lp.lp(rng.bi())) * 0.012 * lev;
        let crackle = rng.bi() * pop;
        buf.add(i, hiss + crackle, hiss + crackle * 0.8);
    }
}
