# music-soundscape

**Night Bus Polaroids**: a six-track, ~20-minute album synthesized from scratch in Rust, played on a Node.js dashboard, shipped in Docker.

> Cassette nostalgia goes out after dark. Boards of Canada, Four Tet and Ben Böhmer meet Fred again..'s vocal-chop techno and Burial's rain-soaked 2-step and dubstep.

| # | Track | Influences | BPM · Key | Length |
|---|---|---|---|---|
| 1 | Northern Tape Memory | Boards of Canada × Four Tet × Ben Böhmer | 120 · F minor | 3:19 |
| 2 | Night Bus Hymn | Burial × Boards of Canada | 138 · D minor | 3:20 |
| 3 | Text Me When You're Home | Fred again.. × Four Tet | 132 · A♭ major | 3:14 |
| 4 | Sodium Rain | Burial × Ben Böhmer | 140 · G minor | 3:18 |
| 5 | Strobe Through Fog | Fred again.. × Ben Böhmer × Four Tet | 130 · A minor | 3:32 |
| 6 | Last Train, Polaroid Light | Burial × Fred again.. × Boards of Canada | 126 · F minor | 2:54 |

The closer returns to the opener's key and chords, quotes its melody and ends on the same tape stop.

## Run it (Docker)

```bash
docker compose up -d --build
# then open http://localhost:3000   (different port: PORT=8080 docker compose up -d --build)
```

The image build compiles the synth and renders the whole album (~11 s, three tracks in parallel, ~1 GB RAM). It then copies only the WAVs and metadata into a small `node:22-alpine` runtime. The port is bound to `127.0.0.1` only.

## Run it without Docker

```bash
cd synth && cargo run --release     # writes synth/out/<slug>.wav + <slug>.json per track, and album.json
cd ../server && npm start           # http://localhost:3000 (reads ../synth/out)
```

While iterating on one track: `SOUNDSCAPE_ONLY=sodium-rain cargo run --release`. That re-renders only that track, and `album.json` keeps the other entries. `SOUNDSCAPE_JOBS=n` sets how many tracks render at once.

## How the album is made (`synth/`)

Zero dependencies: every sample comes from the DSP code in `src/`. Each RNG is seeded, so every render is identical.

| File | |
|---|---|
| `dsp.rs` | buffers, band-limited oscillators, SVF/one-pole filters, ping-pong delay, Freeverb-style reverb, lo-fi, sidechain, tape stop |
| `instruments.rs` | the first track's voices: tape pads, Karplus-Strong harp, kalimba, bass, lead, drums, risers, vinyl |
| `voices.rs` | the after-dark voices (below) plus bus filters, saturation and fades |
| `track.rs` | the shared `Track` / `Section` types, mixer, master glue and JSON metadata |
| `tracks/*.rs` | one module per track: harmony, arrangement, automation and mix |
| `main.rs` | renders the album in parallel and writes `album.json` |

| Influence | What it contributes | Where |
|---|---|---|
| Boards of Canada | Detuned saw pads on wow & flutter, bit-crush, vinyl, music box, tape stop | `pad_note`, `lofi`, `vinyl`, `tape_stop` |
| Four Tet | Swung shakers, wooden clicks, kalimba and harp | `kalimba`, `pluck`, `rim`, `shaker` |
| Ben Böhmer | Melancholic leads, rolling arps and basslines, sidechain pump, big reverb | `lead_note`, `bass_note`, `duck_envelope`, `reverb` |
| Fred again.. | Formant-synthesised voice, sung or chopped into riffs and stutters; felt piano; pumping supersaws; techno kick and rumble | `vox` / `sing`, `piano`, `supersaw`, `stab`, `techno_kick` |
| Burial | Skippy 2-step and half-time dubstep with clanking snares, pitched-up "ghost" vocals that sink, rain and city rumble, reese and tempo-synced wobble bass, gliding sub | `clank`, `GHOST`, `rain`, `reese`, `sub` |

There are no samples. The vocals are a glottal sawtooth plus breath noise through three moving formant filters that glide between vowels. Scaling the formants gives Burial's sped-up "chipmunk" sample sound.

## Dashboard (`server/`)

Plain `node:http`, no npm dependencies.

- Album tracklist with per-track play counts, prev/next, and auto-advance to the next track
- Radial spectrum + oscilloscope visualizer, color-coded by the current section's influence
- Clickable / draggable waveform timeline with section bands
- Live "now playing": section, chord, bar · beat, active layers
- 56-band log spectrum and L/R level meters with peak hold
- Listening stats: listeners now, plays, total listen time, full listens, uptime
- Keyboard: `space` play/pause · `←/→` seek · `↑/↓` volume · `N`/`P` next/prev track · `L` loop track · `1–8` jump to section
- Deep links: `/?track=sodium-rain&t=95&autoplay`

| Endpoint | |
|---|---|
| `GET /api/album` | album title and tracklist |
| `GET /api/tracks/:slug` | a track's metadata, sections, chord timeline and waveform peaks |
| `GET /audio/:slug.wav` | a track (HTTP Range supported, so seeking works) |
| `GET /download/:slug` | a track as an attachment |
| `GET /api/track` | the first track's metadata (for older clients) |
| `GET /api/stats` | listening stats, including plays per track (in memory, reset on restart) |
| `POST /api/events` | player telemetry |
| `GET /api/health` | health check (used by the Docker `HEALTHCHECK`) |
