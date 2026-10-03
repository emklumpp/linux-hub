'use strict';

const $ = (id) => document.getElementById(id);
const audio = $('audio');
const playBtn = $('play');
const vizCanvas = $('viz');
const waveCanvas = $('wave');
const specCanvas = $('spec');

const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const listenerId = (crypto.randomUUID && crypto.randomUUID()) || String(Math.random()).slice(2);
const INFLUENCE_VARS = {
  'Boards of Canada': '--amber',
  'Four Tet': '--teal',
  'Ben Böhmer': '--rose',
  'Fred again..': '--lime',
  Burial: '--violet',
};

let album = null;
let trackIndex = -1;
let loadSeq = 0;
let meta = null;
let ctx = null;
let analyser, analyserL, analyserR, freqData, timeData, timeL, timeR;
let currentSection = -1;
let hoverX = null;
let dragging = false;
let countedPlay = false;
let pendingSeconds = 0;
let lastTime = 0;
const meterPeaks = [0, 0];
const particles = [];

// ---------------------------------------------------------------- helpers

const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const COLORS = Object.fromEntries(['amber', 'teal', 'rose', 'sky', 'violet', 'lime'].map((k) => [k, css(`--${k}`)]));

const esc = (v) => String(v).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

function fmt(t) {
  if (!Number.isFinite(t)) return '0:00';
  const m = Math.floor(t / 60);
  const s = Math.floor(t % 60);
  return `${m}:${String(s).padStart(2, '0')}`;
}

function fmtDuration(sec) {
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ${sec % 60}s`;
  const h = Math.floor(sec / 3600);
  return `${h}h ${Math.floor((sec % 3600) / 60)}m`;
}

/** Colour for an influence; blends ("Burial × Ben Böhmer") take their first named artist. */
function influenceVar(influence) {
  const first = String(influence || '').split(' × ')[0];
  return INFLUENCE_VARS[first] || '--sky';
}

function sectionColorVar(section) {
  if (!section) return '--amber';
  return influenceVar(section.influence);
}

function sectionColor(section) {
  return COLORS[sectionColorVar(section).slice(2)];
}

function hexToRgb(hex) {
  const n = parseInt(hex.replace('#', ''), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function fitCanvas(c) {
  const r = c.getBoundingClientRect();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.round(r.width * dpr));
  const h = Math.max(1, Math.round(r.height * dpr));
  if (c.width !== w || c.height !== h) {
    c.width = w;
    c.height = h;
  }
  return { w, h, dpr };
}

function store(key, value) {
  try {
    if (value === undefined) return localStorage.getItem(key);
    localStorage.setItem(key, value);
  } catch {
    return null;
  }
  return null;
}

// ---------------------------------------------------------------- telemetry

function sendEvent(type) {
  const body = JSON.stringify({ id: listenerId, track: meta?.slug, type, seconds: Math.round(pendingSeconds * 10) / 10 });
  pendingSeconds = 0;
  if (navigator.sendBeacon && navigator.sendBeacon('/api/events', body)) return;
  fetch('/api/events', { method: 'POST', body, keepalive: true }).catch(() => {});
}

setInterval(() => {
  if (!audio.paused) sendEvent('heartbeat');
}, 10_000);

async function pollStats() {
  try {
    const s = await fetch('/api/stats').then((r) => r.json());
    $('st-listening').textContent = s.listening;
    $('st-plays').textContent = s.plays;
    $('st-complete').textContent = s.completions;
    $('st-time').textContent = fmtDuration(s.listenSeconds);
    $('st-uptime').textContent = fmtDuration(s.uptimeSeconds);
    document.querySelectorAll('.trk .plays').forEach((el) => {
      const n = s.trackPlays?.[el.dataset.slug] || 0;
      el.textContent = `${n} play${n === 1 ? '' : 's'}`;
    });
    $('status').className = 'status ok';
    $('status-text').textContent = 'server online';
  } catch {
    $('status').className = 'status err';
    $('status-text').textContent = 'server unreachable';
  }
}

// ---------------------------------------------------------------- audio graph

function ensureAudioGraph() {
  if (ctx) return;
  ctx = new (window.AudioContext || window.webkitAudioContext)();
  const src = ctx.createMediaElementSource(audio);
  analyser = ctx.createAnalyser();
  analyser.fftSize = 4096;
  analyser.smoothingTimeConstant = 0.78;
  analyserL = ctx.createAnalyser();
  analyserR = ctx.createAnalyser();
  analyserL.fftSize = analyserR.fftSize = 1024;
  const splitter = ctx.createChannelSplitter(2);
  src.connect(analyser);
  analyser.connect(ctx.destination);
  src.connect(splitter);
  splitter.connect(analyserL, 0);
  splitter.connect(analyserR, 1);
  freqData = new Uint8Array(analyser.frequencyBinCount);
  timeData = new Float32Array(analyser.fftSize);
  timeL = new Float32Array(analyserL.fftSize);
  timeR = new Float32Array(analyserR.fftSize);
}

async function togglePlay() {
  if (audio.paused) {
    ensureAudioGraph();
    if (ctx.state === 'suspended') await ctx.resume();
    try {
      await audio.play();
    } catch (err) {
      console.error(err);
    }
  } else {
    audio.pause();
  }
}

function seekTo(t, autoplay = false) {
  const d = audio.duration || meta?.duration || 0;
  audio.currentTime = Math.max(0, Math.min(d - 0.05, t));
  lastTime = audio.currentTime;
  if (autoplay && audio.paused) togglePlay();
}

audio.addEventListener('play', () => {
  playBtn.classList.add('playing');
  playBtn.setAttribute('aria-label', 'Pause');
  if (!countedPlay) {
    countedPlay = true;
    sendEvent('play');
  } else {
    sendEvent('heartbeat');
  }
});

audio.addEventListener('pause', () => {
  playBtn.classList.remove('playing');
  playBtn.setAttribute('aria-label', 'Play');
  if (!audio.ended) sendEvent('pause');
});

audio.addEventListener('ended', () => {
  sendEvent('complete');
  countedPlay = false;
  if ($('loop').getAttribute('aria-pressed') === 'true') {
    seekTo(0);
    audio.play();
  } else if (album && trackIndex + 1 < album.tracks.length) {
    loadTrack(trackIndex + 1, { autoplay: true });
  }
});

audio.addEventListener('timeupdate', () => {
  const delta = audio.currentTime - lastTime;
  if (!audio.paused && delta > 0 && delta < 2) pendingSeconds += delta;
  lastTime = audio.currentTime;
});

audio.addEventListener('seeked', () => {
  lastTime = audio.currentTime;
});

audio.addEventListener('loadedmetadata', () => {
  $('dur').textContent = fmt(audio.duration);
  waveCanvas.setAttribute('aria-valuemax', Math.round(audio.duration));
});

window.addEventListener('pagehide', () => {
  if (!audio.paused) sendEvent('pause');
});

// ---------------------------------------------------------------- controls

playBtn.addEventListener('click', togglePlay);
$('next').addEventListener('click', nextTrack);
$('prev').addEventListener('click', prevTrack);

function nextTrack() {
  if (album && trackIndex + 1 < album.tracks.length) loadTrack(trackIndex + 1);
}

/** Like a CD player: restart the track unless we're within its first few seconds. */
function prevTrack() {
  if (audio.currentTime > 3 || trackIndex <= 0) seekTo(0);
  else loadTrack(trackIndex - 1);
}

const volume = $('volume');
function setVolume(v) {
  const vol = Math.max(0, Math.min(1, v));
  audio.volume = vol;
  volume.value = vol;
  volume.style.setProperty('--fill', `${vol * 100}%`);
  store('soundscape.volume', String(vol));
}
volume.addEventListener('input', () => setVolume(Number(volume.value)));
setVolume(Number(store('soundscape.volume') ?? 0.85));

$('loop').addEventListener('click', (e) => {
  const on = e.currentTarget.getAttribute('aria-pressed') !== 'true';
  e.currentTarget.setAttribute('aria-pressed', String(on));
});

function waveTimeAt(clientX) {
  const r = waveCanvas.getBoundingClientRect();
  const x = Math.max(0, Math.min(1, (clientX - r.left) / r.width));
  return x * (audio.duration || meta?.duration || 0);
}

waveCanvas.addEventListener('pointerdown', (e) => {
  dragging = true;
  waveCanvas.setPointerCapture(e.pointerId);
  seekTo(waveTimeAt(e.clientX));
});
waveCanvas.addEventListener('pointermove', (e) => {
  const r = waveCanvas.getBoundingClientRect();
  hoverX = (e.clientX - r.left) / r.width;
  if (dragging) seekTo(waveTimeAt(e.clientX));
});
waveCanvas.addEventListener('pointerup', () => { dragging = false; });
waveCanvas.addEventListener('pointerleave', () => { hoverX = null; });

document.addEventListener('keydown', (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const tag = e.target.tagName;
  const inRange = tag === 'INPUT';
  if (e.code === 'Space' && tag !== 'BUTTON' && tag !== 'A') {
    e.preventDefault();
    togglePlay();
  } else if (e.key === 'ArrowRight' && !inRange) {
    e.preventDefault();
    seekTo(audio.currentTime + 5);
  } else if (e.key === 'ArrowLeft' && !inRange) {
    e.preventDefault();
    seekTo(audio.currentTime - 5);
  } else if (e.key === 'ArrowUp' && !inRange) {
    e.preventDefault();
    setVolume(audio.volume + 0.05);
  } else if (e.key === 'ArrowDown' && !inRange) {
    e.preventDefault();
    setVolume(audio.volume - 0.05);
  } else if (e.key.toLowerCase() === 'l') {
    $('loop').click();
  } else if (e.key.toLowerCase() === 'n') {
    nextTrack();
  } else if (e.key.toLowerCase() === 'p') {
    prevTrack();
  } else if (/^[1-8]$/.test(e.key) && meta) {
    const s = meta.sections[Number(e.key) - 1];
    if (s) seekTo(s.start, true);
  }
});

if ('mediaSession' in navigator) {
  navigator.mediaSession.setActionHandler('play', togglePlay);
  navigator.mediaSession.setActionHandler('pause', () => audio.pause());
  navigator.mediaSession.setActionHandler('seekbackward', () => seekTo(audio.currentTime - 10));
  navigator.mediaSession.setActionHandler('seekforward', () => seekTo(audio.currentTime + 10));
  navigator.mediaSession.setActionHandler('seekto', (d) => seekTo(d.seekTime));
  navigator.mediaSession.setActionHandler('nexttrack', nextTrack);
  navigator.mediaSession.setActionHandler('previoustrack', prevTrack);
}

// ---------------------------------------------------------------- metadata UI

function renderAlbum() {
  $('album-title').textContent = album.title;
  const total = album.tracks.reduce((sum, t) => sum + t.duration, 0);
  $('album-hint').textContent = `${album.tracks.length} tracks · ${fmt(total)} · N / P to skip`;
  $('track-list').innerHTML = album.tracks
    .map((t, i) => `
      <li>
        <button class="sec trk" data-i="${i}" style="--c: var(${influenceVar(t.influences[0])})">
          <span class="row"><span class="num">${String(i + 1).padStart(2, '0')}</span><span>${fmt(t.duration)}</span></span>
          <span class="name">${esc(t.title)}</span>
          <span class="tags">${t.influences.map((n) => `<span style="color: var(${influenceVar(n)})">${esc(n)}</span>`).join('')}</span>
          <span class="desc">${esc(t.blurb)}</span>
          <span class="meta">${t.bpm} BPM · ${esc(t.key)} · <span class="plays" data-slug="${esc(t.slug)}">0 plays</span></span>
          <span class="bar"></span>
        </button>
      </li>`)
    .join('');
  document.querySelectorAll('.trk').forEach((btn) => {
    btn.addEventListener('click', () => {
      const i = Number(btn.dataset.i);
      if (i === trackIndex) togglePlay();
      else loadTrack(i, { autoplay: true });
    });
  });
}

/** Fetches a track's metadata and swaps it into the player, keeping playback going if it was. */
async function loadTrack(i, { autoplay = false, startAt = 0 } = {}) {
  const entry = album?.tracks[i];
  if (!entry) return;
  const id = ++loadSeq;
  let next;
  try {
    const res = await fetch(`/api/tracks/${encodeURIComponent(entry.slug)}`);
    if (!res.ok) throw new Error((await res.json()).error || res.statusText);
    next = await res.json();
  } catch (err) {
    $('section-name').textContent = 'Track unavailable';
    $('section-mood').textContent = String(err.message || err);
    return;
  }
  if (id !== loadSeq) return; // a later click won the race

  const resume = autoplay || !audio.paused;
  if (!audio.paused && pendingSeconds > 0) sendEvent('heartbeat');
  meta = next;
  trackIndex = i;
  currentSection = -1;
  countedPlay = false;
  lastTime = 0;
  audio.src = meta.audio;
  renderMeta();
  try {
    history.replaceState(null, '', `?track=${encodeURIComponent(meta.slug)}`);
  } catch {
    /* sandboxed previews may forbid it */
  }

  const go = () => {
    if (id !== loadSeq) return;
    if (startAt > 0) seekTo(startAt);
    if (resume) togglePlay();
  };
  if (audio.readyState >= 1) go();
  else audio.addEventListener('loadedmetadata', go, { once: true });
}

function renderMeta() {
  document.title = `${meta.title} · ${album?.title ?? ''}`;
  $('title').textContent = meta.title;
  $('influences').textContent = meta.influences.join(' × ');
  $('track-pos').textContent = album ? `track ${trackIndex + 1} of ${album.tracks.length}` : '';
  $('download').href = `/download/${encodeURIComponent(meta.slug)}`;
  $('next').disabled = !album || trackIndex + 1 >= album.tracks.length;
  document.querySelectorAll('.trk').forEach((el, k) => {
    el.classList.toggle('active', k === trackIndex);
    el.style.setProperty('--p', '0%');
  });
  document.documentElement.style.setProperty('--accent', `var(${influenceVar(meta.influences[0])})`);
  $('section-name').textContent = meta.title;
  $('section-mood').textContent = meta.blurb || '';
  $('influence').textContent = meta.influences.join(' × ');
  $('chord').textContent = '—';
  $('barbeat').textContent = '—';
  $('layers').innerHTML = '';
  $('dur').textContent = fmt(meta.duration);
  $('st-render').textContent = `${(meta.renderMs / 1000).toFixed(1)}s`;

  const chips = [
    ['tempo', `${meta.bpm} BPM`],
    ['key', meta.key],
    ['length', fmt(meta.duration)],
    ['bars', meta.bars],
    ['format', `${meta.sampleRate / 1000} kHz · 16-bit WAV`],
    ['engine', 'Rust · zero deps'],
  ];
  $('chips').innerHTML = chips.map(([k, v]) => `<li>${k} <b>${v}</b></li>`).join('');

  $('section-list').innerHTML = meta.sections
    .map((s, i) => `
      <li>
        <button class="sec" data-i="${i}" style="--c: var(${sectionColorVar(s)})">
          <span class="row"><span class="num">${String(i + 1).padStart(2, '0')}</span><span>${fmt(s.start)} – ${fmt(s.end)}</span></span>
          <span class="name">${esc(s.name)}</span>
          <span class="inf">${esc(s.influence)}</span>
          <span class="desc">${esc(s.mood)}</span>
          <span class="bar"></span>
        </button>
      </li>`)
    .join('');
  document.querySelectorAll('#section-list .sec').forEach((btn) => {
    btn.addEventListener('click', () => seekTo(meta.sections[Number(btn.dataset.i)].start, true));
  });

  if ('mediaSession' in navigator) {
    navigator.mediaSession.metadata = new MediaMetadata({ title: meta.title, artist: meta.artist, album: album?.title ?? '' });
  }
}

function sectionIndexAt(t) {
  if (!meta) return -1;
  const i = meta.sections.findIndex((s) => t >= s.start && t < s.end);
  return i === -1 ? (t > 0 ? meta.sections.length - 1 : 0) : i;
}

function chordAt(t) {
  let name = meta.chords[0].name;
  for (const c of meta.chords) {
    if (c.time <= t) name = c.name;
    else break;
  }
  return name;
}

function updateNowPanel(t) {
  const i = sectionIndexAt(t);
  const s = meta.sections[i];
  const started = t > 0 || !audio.paused;

  if (i !== currentSection && started) {
    currentSection = i;
    document.documentElement.style.setProperty('--accent', `var(${sectionColorVar(s)})`);
    $('section-name').textContent = s.name;
    $('section-mood').textContent = s.mood;
    $('influence').textContent = s.influence;
    $('layers').innerHTML = s.layers.map((l) => `<li>${l}</li>`).join('');
    document.querySelectorAll('#section-list .sec').forEach((el, k) => el.classList.toggle('active', k === i));
  }

  const barLen = (240 / meta.bpm);
  const songEnd = meta.bars * barLen;
  if (started && t < songEnd) {
    const bar = Math.floor(t / barLen) + 1;
    const beat = Math.floor((t % barLen) / (60 / meta.bpm)) + 1;
    $('barbeat').textContent = `${bar} · ${beat}`;
    $('chord').textContent = chordAt(t);
  } else if (t >= songEnd) {
    $('barbeat').textContent = 'tail';
  }

  const active = document.querySelector(`.trk[data-i="${trackIndex}"]`);
  if (active) active.style.setProperty('--p', `${Math.min(1, t / (audio.duration || meta.duration)) * 100}%`);

  document.querySelectorAll('#section-list .sec').forEach((el, k) => {
    const sec = meta.sections[k];
    const p = Math.max(0, Math.min(1, (t - sec.start) / (sec.end - sec.start)));
    el.style.setProperty('--p', `${(t >= sec.start ? p : 0) * 100}%`);
  });
}

// ---------------------------------------------------------------- drawing

function bandEnergy(lo, hi) {
  if (!freqData || !ctx) return 0;
  const binHz = ctx.sampleRate / analyser.fftSize;
  const a = Math.max(1, Math.floor(lo / binHz));
  const b = Math.min(freqData.length - 1, Math.ceil(hi / binHz));
  let sum = 0;
  for (let i = a; i <= b; i++) sum += freqData[i];
  return sum / ((b - a + 1) * 255);
}

/** Log-spaced magnitude (0..1) at fraction x across 40 Hz – 16 kHz. */
function logMag(x) {
  if (!freqData || !ctx) return 0;
  const hz = 40 * Math.pow(16000 / 40, x);
  const bin = Math.min(freqData.length - 1, Math.round(hz / (ctx.sampleRate / analyser.fftSize)));
  return freqData[bin] / 255;
}

function drawViz(now) {
  const { w, h, dpr } = fitCanvas(vizCanvas);
  const g = vizCanvas.getContext('2d');
  const live = ctx && !audio.paused;
  const accent = sectionColor(meta?.sections[currentSection]);
  const [ar, ag, ab] = hexToRgb(accent || '#e9a55a');
  const idle = (k) => 0.18 + 0.12 * Math.sin(now / 900 + k);

  // Motion trails: fade previous frames toward transparent so the card shows through.
  g.globalCompositeOperation = 'destination-out';
  g.fillStyle = 'rgba(0, 0, 0, 0.28)';
  g.fillRect(0, 0, w, h);
  g.globalCompositeOperation = 'source-over';

  const cx = w / 2;
  const cy = h / 2;
  const R = Math.min(w, h) * 0.24;
  const bass = live ? bandEnergy(30, 130) : idle(0) * 0.6;
  const mids = live ? bandEnergy(300, 2500) : idle(1) * 0.5;
  const rot = reducedMotion ? 0 : now / 9000;

  // radial spectrum
  const N = 120;
  g.lineCap = 'round';
  g.lineWidth = Math.max(1.5, 2.2 * dpr);
  for (let i = 0; i < N; i++) {
    const x = i / N;
    const mirror = x < 0.5 ? x * 2 : (1 - x) * 2;
    const m = live ? logMag(0.02 + mirror * 0.9) : idle(i * 0.3) * 0.8;
    const a = rot + x * Math.PI * 2 - Math.PI / 2;
    const r0 = R * 1.18;
    const r1 = r0 + m * R * 0.95 + 2 * dpr;
    g.strokeStyle = `rgba(${ar}, ${ag}, ${ab}, ${0.25 + m * 0.75})`;
    g.beginPath();
    g.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0);
    g.lineTo(cx + Math.cos(a) * r1, cy + Math.sin(a) * r1);
    g.stroke();
  }

  // oscilloscope ring
  if (live) analyser.getFloatTimeDomainData(timeData);
  g.beginPath();
  const ringR = R * 1.92;
  const steps = 256;
  for (let i = 0; i <= steps; i++) {
    const v = live ? timeData[Math.floor((i % steps) / steps * timeData.length)] : Math.sin(i / 8 + now / 600) * 0.03;
    const a = -rot * 0.5 + (i / steps) * Math.PI * 2;
    const r = ringR + v * R * 0.5;
    const px = cx + Math.cos(a) * r;
    const py = cy + Math.sin(a) * r;
    if (i === 0) g.moveTo(px, py);
    else g.lineTo(px, py);
  }
  g.strokeStyle = `rgba(239, 229, 213, ${0.14 + mids * 0.4})`;
  g.lineWidth = 1.2 * dpr;
  g.stroke();

  // hexagon core (a nod to the BoC hexagon)
  const hexR = R * (0.62 + bass * 0.32);
  const grad = g.createRadialGradient(cx, cy, 0, cx, cy, hexR * 1.4);
  grad.addColorStop(0, `rgba(${ar}, ${ag}, ${ab}, ${0.18 + bass * 0.35})`);
  grad.addColorStop(1, 'rgba(18, 16, 14, 0)');
  g.fillStyle = grad;
  g.beginPath();
  g.arc(cx, cy, hexR * 1.4, 0, Math.PI * 2);
  g.fill();

  for (let ring = 0; ring < 3; ring++) {
    const rr = hexR * (1 - ring * 0.28);
    g.beginPath();
    for (let k = 0; k <= 6; k++) {
      const a = -rot * (ring % 2 ? -1.5 : 1) + (k / 6) * Math.PI * 2 + Math.PI / 6;
      const px = cx + Math.cos(a) * rr;
      const py = cy + Math.sin(a) * rr;
      if (k === 0) g.moveTo(px, py);
      else g.lineTo(px, py);
    }
    g.strokeStyle = ring === 0 ? `rgba(${ar}, ${ag}, ${ab}, 0.95)` : `rgba(239, 229, 213, ${0.35 - ring * 0.1})`;
    g.lineWidth = (ring === 0 ? 2.4 : 1.2) * dpr;
    g.shadowColor = `rgba(${ar}, ${ag}, ${ab}, 0.8)`;
    g.shadowBlur = ring === 0 ? 18 * dpr * (0.4 + bass) : 0;
    g.stroke();
  }
  g.shadowBlur = 0;

  // film dust
  if (!reducedMotion) {
    while (particles.length < 70) {
      particles.push({ x: Math.random(), y: Math.random(), v: 0.0002 + Math.random() * 0.0006, s: Math.random() * 1.6 + 0.4, p: Math.random() * 6 });
    }
    for (const p of particles) {
      p.y -= p.v * (1 + bass * 3);
      p.x += Math.sin(now / 2000 + p.p) * 0.0002;
      if (p.y < 0) { p.y = 1; p.x = Math.random(); }
      const a = 0.15 + 0.35 * Math.abs(Math.sin(now / 700 + p.p));
      g.fillStyle = `rgba(239, 229, 213, ${a})`;
      g.fillRect(p.x * w, p.y * h, p.s * dpr, p.s * dpr);
    }
  }
}

function drawWave() {
  const { w, h, dpr } = fitCanvas(waveCanvas);
  const g = waveCanvas.getContext('2d');
  g.clearRect(0, 0, w, h);
  if (!meta) return;

  const dur = audio.duration || meta.duration;
  const t = audio.currentTime;
  const played = t / dur;
  const band = 18 * dpr;
  const mid = band + (h - band) / 2;
  const amp = (h - band) / 2 - 4 * dpr;

  // section band
  g.font = `${10 * dpr}px "IBM Plex Mono", monospace`;
  g.textBaseline = 'middle';
  meta.sections.forEach((s, i) => {
    const x0 = (s.start / dur) * w;
    const x1 = (s.end / dur) * w;
    const col = sectionColor(s);
    g.fillStyle = col;
    g.globalAlpha = i === currentSection ? 0.9 : 0.35;
    g.fillRect(x0 + 1, 0, x1 - x0 - 2, 3 * dpr);
    g.globalAlpha = i === currentSection ? 1 : 0.55;
    const label = s.name;
    if (g.measureText(label).width < x1 - x0 - 8 * dpr) g.fillText(label, x0 + 4 * dpr, 11 * dpr);
  });
  g.globalAlpha = 1;

  // peaks
  const peaks = meta.peaks;
  const barW = w / peaks.length;
  for (let i = 0; i < peaks.length; i++) {
    const x = i * barW;
    const p = Math.max(0.01, peaks[i]) * amp;
    const isPlayed = i / peaks.length < played;
    const sIdx = sectionIndexAt((i / peaks.length) * dur);
    g.fillStyle = isPlayed ? sectionColor(meta.sections[sIdx]) : 'rgba(163, 151, 135, 0.32)';
    g.fillRect(x, mid - p, Math.max(1, barW * 0.7), p * 2);
  }

  // playhead
  const px = played * w;
  g.fillStyle = '#efe5d5';
  g.fillRect(px - dpr, band - 2 * dpr, 2 * dpr, h - band + 2 * dpr);

  // hover readout
  if (hoverX !== null) {
    const hx = hoverX * w;
    g.fillStyle = 'rgba(239, 229, 213, 0.35)';
    g.fillRect(hx, band, dpr, h - band);
    const label = fmt(hoverX * dur);
    g.font = `500 ${11 * dpr}px "IBM Plex Mono", monospace`;
    const tw = g.measureText(label).width + 10 * dpr;
    const lx = Math.min(w - tw, Math.max(0, hx - tw / 2));
    g.fillStyle = '#efe5d5';
    g.fillRect(lx, h - 18 * dpr, tw, 16 * dpr);
    g.fillStyle = '#12100e';
    g.fillText(label, lx + 5 * dpr, h - 10 * dpr);
  }

  waveCanvas.setAttribute('aria-valuenow', Math.round(t));
  waveCanvas.setAttribute('aria-valuetext', `${fmt(t)} of ${fmt(dur)}`);
}

function rmsDb(buf) {
  let s = 0;
  for (let i = 0; i < buf.length; i++) s += buf[i] * buf[i];
  const rms = Math.sqrt(s / buf.length);
  return 20 * Math.log10(rms + 1e-9);
}

function drawSpectrum() {
  const { w, h, dpr } = fitCanvas(specCanvas);
  const g = specCanvas.getContext('2d');
  g.clearRect(0, 0, w, h);
  const live = ctx && !audio.paused;
  if (ctx) analyser.getByteFrequencyData(freqData);

  const meterW = 46 * dpr;
  const specW = w - meterW - 14 * dpr;
  const bands = 56;
  const gap = 2 * dpr;
  const bw = (specW - gap * (bands - 1)) / bands;
  const labelH = 16 * dpr;
  const plotH = h - labelH;

  // gridlines
  g.strokeStyle = 'rgba(163, 151, 135, 0.12)';
  g.lineWidth = dpr;
  for (let k = 1; k < 4; k++) {
    const y = (plotH * k) / 4;
    g.beginPath();
    g.moveTo(0, y);
    g.lineTo(specW, y);
    g.stroke();
  }

  for (let i = 0; i < bands; i++) {
    const m = live ? logMag(i / (bands - 1)) : 0.02;
    const bh = Math.max(2 * dpr, m * plotH);
    const x = i * (bw + gap);
    const hue = i / bands;
    const col = hue < 0.33 ? COLORS.amber : hue < 0.66 ? COLORS.rose : COLORS.teal;
    g.fillStyle = col;
    g.globalAlpha = 0.35 + m * 0.65;
    g.fillRect(x, plotH - bh, bw, bh);
  }
  g.globalAlpha = 1;

  g.fillStyle = 'rgba(163, 151, 135, 0.7)';
  g.font = `${10 * dpr}px "IBM Plex Mono", monospace`;
  g.textBaseline = 'bottom';
  for (const hz of [100, 1000, 10000]) {
    const x = (Math.log(hz / 40) / Math.log(16000 / 40)) * specW;
    g.fillText(hz >= 1000 ? `${hz / 1000}k` : `${hz}`, x, h);
  }

  // L / R meters with peak hold
  const mx = w - meterW;
  const each = (meterW - 6 * dpr) / 2;
  [analyserL, analyserR].forEach((an, k) => {
    let level = 0;
    if (live) {
      const buf = k === 0 ? timeL : timeR;
      an.getFloatTimeDomainData(buf);
      level = Math.max(0, Math.min(1, (rmsDb(buf) + 48) / 48));
    }
    meterPeaks[k] = Math.max(level, meterPeaks[k] - 0.006);
    const x = mx + k * (each + 6 * dpr);
    g.fillStyle = 'rgba(163, 151, 135, 0.12)';
    g.fillRect(x, 0, each, plotH);
    const grad = g.createLinearGradient(0, plotH, 0, 0);
    grad.addColorStop(0, COLORS.teal);
    grad.addColorStop(0.7, COLORS.amber);
    grad.addColorStop(1, COLORS.rose);
    g.fillStyle = grad;
    g.fillRect(x, plotH * (1 - level), each, plotH * level);
    g.fillStyle = '#efe5d5';
    g.fillRect(x, plotH * (1 - meterPeaks[k]) - dpr, each, 2 * dpr);
    g.fillStyle = 'rgba(163, 151, 135, 0.7)';
    g.fillText(k === 0 ? 'L' : 'R', x + each / 2 - 3 * dpr, h);
  });
}

function frame(now) {
  if (meta) {
    const t = audio.currentTime;
    $('cur').textContent = fmt(t);
    updateNowPanel(t);
  }
  drawSpectrum();
  drawViz(now);
  drawWave();
  requestAnimationFrame(frame);
}

// ---------------------------------------------------------------- boot

async function init() {
  try {
    const res = await fetch('/api/album');
    if (!res.ok) throw new Error((await res.json()).error || res.statusText);
    album = await res.json();
    renderAlbum();
  } catch (err) {
    $('section-name').textContent = 'Album unavailable';
    $('section-mood').textContent = String(err.message || err);
  }
  // Deep links: /?track=sodium-rain&t=95 starts that track at 1:35; add &autoplay to start immediately.
  const params = new URLSearchParams(location.search);
  if (album?.tracks.length) {
    const i = Math.max(0, album.tracks.findIndex((t) => t.slug === params.get('track')));
    await loadTrack(i, { autoplay: params.has('autoplay'), startAt: Number(params.get('t')) || 0 });
  }
  pollStats();
  setInterval(pollStats, 5000);
  requestAnimationFrame(frame);
}

init();
