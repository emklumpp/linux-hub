'use strict';

// Zero-dependency web server for the soundscape dashboard.
//   GET  /                     dashboard (static files in ./public)
//   GET  /api/album            album title + tracklist written by the Rust renderer
//   GET  /api/tracks/:slug     one track's metadata, sections, chords and waveform peaks
//   GET  /api/track            the first track's metadata (kept for old clients)
//   GET  /audio/:slug.wav      a rendered track (supports HTTP Range for seeking)
//   GET  /download/:slug       same file as an attachment
//   GET  /api/stats            listening stats for the dashboard
//   POST /api/events           player telemetry: { id, track, type: "play" | "heartbeat" | "pause" | "complete", seconds }
//   GET  /api/health           liveness probe

const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');

const PORT = Number(process.env.PORT) || 3000;
const HOST = process.env.HOST || '0.0.0.0';
const AUDIO_DIR = path.resolve(process.env.AUDIO_DIR || path.join(__dirname, '..', 'synth', 'out'));
const PUBLIC_DIR = path.join(__dirname, 'public');
const ALBUM_PATH = path.join(AUDIO_DIR, 'album.json');
const SLUG_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const LISTENER_TTL_MS = 25_000;

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
};

const stats = {
  startedAt: Date.now(),
  plays: 0,
  completions: 0,
  listenSeconds: 0,
  listeners: new Map(), // id -> last heartbeat timestamp
  trackPlays: new Map(), // slug -> plays
};

const jsonCache = new Map(); // path -> { mtimeMs, body }

/** Reads a JSON file written by the renderer, re-reading only when it changes on disk. */
function loadJson(file) {
  const st = fs.statSync(file);
  const hit = jsonCache.get(file);
  if (hit && hit.mtimeMs === st.mtimeMs) return hit.body;
  const body = fs.readFileSync(file, 'utf8');
  jsonCache.set(file, { mtimeMs: st.mtimeMs, body });
  return body;
}

function albumSlugs() {
  try {
    return JSON.parse(loadJson(ALBUM_PATH)).tracks.map((t) => t.slug);
  } catch {
    return [];
  }
}

/** Only slugs listed in album.json map to files, so request paths can never escape AUDIO_DIR. */
function trackFile(slug, ext) {
  if (!SLUG_RE.test(slug) || !albumSlugs().includes(slug)) return null;
  return path.join(AUDIO_DIR, `${slug}.${ext}`);
}

function sendJson(res, status, value) {
  const body = typeof value === 'string' ? value : JSON.stringify(value);
  res.writeHead(status, {
    'Content-Type': MIME['.json'],
    'Content-Length': Buffer.byteLength(body),
    'Cache-Control': 'no-store',
  });
  res.end(body);
}

function activeListeners() {
  const cutoff = Date.now() - LISTENER_TTL_MS;
  for (const [id, seen] of stats.listeners) {
    if (seen < cutoff) stats.listeners.delete(id);
  }
  return stats.listeners.size;
}

function serveAudio(req, res, slug, asDownload) {
  const file = trackFile(slug, 'wav');
  if (!file) return sendJson(res, 404, { error: `no track "${slug}" on the album` });
  fs.stat(file, (err, st) => {
    if (err) return sendJson(res, 404, { error: `${slug}.wav not rendered yet — run the Rust synth first` });

    const size = st.size;
    const headers = {
      'Content-Type': 'audio/wav',
      'Accept-Ranges': 'bytes',
      'Cache-Control': 'public, max-age=3600',
      'Last-Modified': st.mtime.toUTCString(),
    };
    if (asDownload) headers['Content-Disposition'] = `attachment; filename="${slug}.wav"`;

    let start = 0;
    let end = size - 1;
    let status = 200;
    const range = req.headers.range;
    if (range) {
      const m = /^bytes=(\d*)-(\d*)$/.exec(range.trim());
      if (!m || (m[1] === '' && m[2] === '')) {
        res.writeHead(416, { 'Content-Range': `bytes */${size}` });
        return res.end();
      }
      if (m[1] === '') {
        start = Math.max(0, size - Number(m[2]));
      } else {
        start = Number(m[1]);
        if (m[2] !== '') end = Math.min(Number(m[2]), size - 1);
      }
      if (start > end || start >= size) {
        res.writeHead(416, { 'Content-Range': `bytes */${size}` });
        return res.end();
      }
      status = 206;
      headers['Content-Range'] = `bytes ${start}-${end}/${size}`;
    }
    headers['Content-Length'] = end - start + 1;
    res.writeHead(status, headers);
    if (req.method === 'HEAD') return res.end();
    const stream = fs.createReadStream(file, { start, end });
    stream.on('error', () => res.destroy());
    res.on('close', () => stream.destroy());
    stream.pipe(res);
  });
}

function serveStatic(req, res, pathname) {
  let rel;
  try {
    rel = decodeURIComponent(pathname === '/' ? '/index.html' : pathname);
  } catch {
    return sendJson(res, 400, { error: 'bad path' });
  }
  const file = path.normalize(path.join(PUBLIC_DIR, rel));
  if (!file.startsWith(PUBLIC_DIR + path.sep)) return sendJson(res, 403, { error: 'forbidden' });

  fs.stat(file, (err, st) => {
    if (err || !st.isFile()) return sendJson(res, 404, { error: 'not found' });
    res.writeHead(200, {
      'Content-Type': MIME[path.extname(file)] || 'application/octet-stream',
      'Content-Length': st.size,
      'Cache-Control': 'no-cache',
    });
    if (req.method === 'HEAD') return res.end();
    fs.createReadStream(file).pipe(res);
  });
}

function readBody(req, limit = 2048) {
  return new Promise((resolve, reject) => {
    let data = '';
    req.setEncoding('utf8');
    req.on('data', (chunk) => {
      data += chunk;
      if (data.length > limit) {
        reject(new Error('body too large'));
        req.destroy();
      }
    });
    req.on('end', () => resolve(data));
    req.on('error', reject);
  });
}

async function handleEvent(req, res) {
  let event;
  try {
    event = JSON.parse(await readBody(req));
  } catch {
    return sendJson(res, 400, { error: 'invalid JSON' });
  }
  const id = typeof event.id === 'string' ? event.id.slice(0, 64) : null;
  const track = typeof event.track === 'string' && albumSlugs().includes(event.track) ? event.track : null;
  const seconds = Number(event.seconds);
  if (Number.isFinite(seconds) && seconds > 0 && seconds < 120) stats.listenSeconds += seconds;

  switch (event.type) {
    case 'play':
      stats.plays += 1;
      if (track) stats.trackPlays.set(track, (stats.trackPlays.get(track) || 0) + 1);
      if (id) stats.listeners.set(id, Date.now());
      break;
    case 'heartbeat':
      if (id) stats.listeners.set(id, Date.now());
      break;
    case 'pause':
      if (id) stats.listeners.delete(id);
      break;
    case 'complete':
      stats.completions += 1;
      if (id) stats.listeners.delete(id);
      break;
    default:
      return sendJson(res, 400, { error: 'unknown event type' });
  }
  sendJson(res, 200, { ok: true });
}

const server = http.createServer((req, res) => {
  const { pathname } = new URL(req.url, 'http://localhost');
  const isRead = req.method === 'GET' || req.method === 'HEAD';

  if (pathname === '/api/health' && isRead) {
    const slugs = albumSlugs();
    const ready = slugs.filter((slug) => fs.existsSync(path.join(AUDIO_DIR, `${slug}.wav`))).length;
    return sendJson(res, 200, { ok: true, songReady: slugs.length > 0 && ready === slugs.length, tracks: ready });
  }
  if (pathname === '/api/album' && isRead) {
    try {
      return sendJson(res, 200, loadJson(ALBUM_PATH));
    } catch {
      return sendJson(res, 404, { error: 'album.json not found — run the Rust synth first' });
    }
  }
  const trackMatch = /^\/api\/tracks\/([^/]+)$/.exec(pathname);
  if ((trackMatch || pathname === '/api/track') && isRead) {
    const file = trackFile(trackMatch ? trackMatch[1] : albumSlugs()[0] || '', 'json');
    try {
      if (!file) throw new Error('unknown track');
      return sendJson(res, 200, loadJson(file));
    } catch {
      return sendJson(res, 404, { error: 'track metadata not found — run the Rust synth first' });
    }
  }
  if (pathname === '/api/stats' && isRead) {
    return sendJson(res, 200, {
      plays: stats.plays,
      completions: stats.completions,
      listenSeconds: Math.round(stats.listenSeconds),
      listening: activeListeners(),
      uptimeSeconds: Math.round((Date.now() - stats.startedAt) / 1000),
      trackPlays: Object.fromEntries(stats.trackPlays),
    });
  }
  if (pathname === '/api/events' && req.method === 'POST') {
    return void handleEvent(req, res);
  }
  const audioMatch = /^\/audio\/([^/]+)\.wav$/.exec(pathname);
  if (audioMatch && isRead) return serveAudio(req, res, audioMatch[1], false);
  const downloadMatch = /^\/download\/([^/]+)$/.exec(pathname);
  if (downloadMatch && isRead) return serveAudio(req, res, downloadMatch[1], true);
  if (isRead) return serveStatic(req, res, pathname);

  sendJson(res, 405, { error: 'method not allowed' });
});

server.listen(PORT, HOST, () => {
  const n = albumSlugs().length;
  const ready = n ? `${n} track${n === 1 ? '' : 's'} ready` : 'WARNING: album.json missing';
  console.log(`♪ soundscape dashboard on http://localhost:${PORT}  (audio: ${AUDIO_DIR} — ${ready})`);
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    server.close(() => process.exit(0));
    setTimeout(() => process.exit(0), 2000).unref();
  });
}
