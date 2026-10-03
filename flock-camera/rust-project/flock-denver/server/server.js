// Serves the map UI and the camera dataset written by the Rust fetcher.
// POST /api/refresh shells out to `cargo run` so you can re-pull from the UI.

import express from "express";
import { readFile, stat } from "node:fs/promises";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
// Containers mount a writable volume elsewhere; local dev keeps ../data.
const DATA_FILE = process.env.DATA_FILE || path.join(ROOT, "data", "cameras.json");
const FETCHER_DIR = path.join(ROOT, "fetcher");
const PORT = process.env.PORT || 3000;

const app = express();
app.use(express.static(path.join(__dirname, "public")));

app.get("/api/cameras", async (_req, res) => {
  try {
    const raw = await readFile(DATA_FILE, "utf8");
    res.type("json").send(raw);
  } catch (err) {
    if (err.code === "ENOENT") {
      return res.status(404).json({
        error: "No data yet. Run `npm run refresh` (needs Rust/cargo) or POST /api/refresh.",
      });
    }
    res.status(500).json({ error: err.message });
  }
});

app.get("/api/status", async (_req, res) => {
  try {
    const s = await stat(DATA_FILE);
    res.json({ hasData: true, modified: s.mtime });
  } catch {
    res.json({ hasData: false });
  }
});

// Named mirrors only. The chosen endpoint is handed to a subprocess that makes
// outbound requests, so accepting an arbitrary caller-supplied URL here would be
// a server-side request forgery hole.
const MIRRORS = {
  main: "https://overpass-api.de/api/interpreter",
  kumi: "https://overpass.kumi.systems/api/interpreter",
  coffee: "https://overpass.private.coffee/api/interpreter",
  osmjp: "https://overpass.osm.jp/api/interpreter",
};

// Containers ship the compiled binary and have no cargo; local dev falls back to
// `cargo run` so the repo still works with nothing but a checkout.
const FETCHER_BIN = process.env.FETCHER_BIN || null;

app.get("/api/mirrors", (_req, res) => {
  res.json({ default: "main", mirrors: Object.keys(MIRRORS) });
});

let refreshing = false;
app.post("/api/refresh", (req, res) => {
  if (refreshing) return res.status(409).json({ error: "Refresh already running" });

  const mirror = req.query.mirror || "main";
  // hasOwn, not `in` — otherwise ?mirror=constructor resolves up the prototype.
  if (!Object.hasOwn(MIRRORS, mirror)) {
    return res.status(400).json({
      error: `Unknown mirror "${mirror}". Valid: ${Object.keys(MIRRORS).join(", ")}`,
    });
  }

  const args = ["--out", DATA_FILE, "--endpoint", MIRRORS[mirror]];
  if (req.query.flockOnly === "1") args.push("--flock-only");

  const [cmd, argv, opts] = FETCHER_BIN
    ? [FETCHER_BIN, args, {}]
    : ["cargo", ["run", "--release", "--", ...args], { cwd: FETCHER_DIR }];

  refreshing = true;
  // Generous: the fetcher now retries with backoff before giving up.
  execFile(cmd, argv, { ...opts, timeout: 10 * 60_000, maxBuffer: 1 << 20 }, (err, _stdout, stderr) => {
    refreshing = false;
    const log = (stderr || "").trim();
    if (err) return res.status(500).json({ error: err.message, mirror, stderr: log.slice(-1200) });
    res.json({ ok: true, mirror, log: log.split("\n").slice(-4) });
  });
});

app.listen(PORT, () => {
  console.log(`flock-denver map: http://localhost:${PORT}`);
});
