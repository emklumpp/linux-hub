# sound-emulator

Record the sound of a place, redraw one of your Fuji photographs in ink, and let
the sound push the ink around.

## The idea

A **spectrogram is already a field laid over a picture**: slice the recording
into time frames, FFT each one, and you get *time × frequency → loudness*. Every
point of the photograph now has a loudness sitting underneath it. What you do
with that is the choice between the two views.

**Sketch** — the photo is redrawn as stipple: thousands of ink dots, dense where
it's dark, sparse where it's light, placed by error diffusion so they spread
evenly instead of clumping. Broad dark masses — a stone face in shadow, a
foreground of grass — are dodged and burned first, so they come out as texture
rather than as one flat slab of ink. Then every dot is shoved off its mark by the sound —
up where the field is louder than average, down where it's quieter, with a
sideways shear along the field's slope. The picture ripples like it's being seen
through moving water. One image deformed by sound, not two layers stacked.

```
photo ──► stipple ──┐
                    ├──► dots displaced by sound ──► screen
sound ──► spectrogram field
```

**Terrain** — the older reading: loudness as **elevation**, photograph as
**surface colour**, giving a 3D landscape you can orbit.

```
photo ──► colour ───┐
                    ├──► displaced grid ──► screen
sound ──► height
```

## Running

```bash
cargo run --release
cargo run --release -- --live   # open straight into live mode
```

Flags, handy for tuning the look without a keyboard (each pairs well with
`--capture <path>`, which saves one frame and quits):

| flag | effect |
|------|--------|
| `--view sketch\|surface\|points` | which view to open in |
| `--wave <n>`   | sound displacement, `0` for the undisturbed stipple |
| `--cells <n>`  | stipple grid cells — more cells, finer dots |
| `--colour`     | ink in the photo's own colours |

(The first build is slow — nannou compiles `wgpu` and friends.)

Drop a photo into `inputs/` (jpg/png/tiff — subfolders are searched too). The
first image found is used; if there's none, a placeholder gradient stands in so
the app still runs. `R` and `G` re-scan, so you can add a photo while it's open.

HEIF/HEIC straight off a phone won't load — the `image` crate can't decode it.
Convert first: `magick photo.heif -auto-orient photo.png`.

No microphone? Press `G` and the clip is synthesised in code instead.

## Controls

| key / action | effect |
|--------------|--------|
| `R`          | record a new ~6s clip from the mic |
| `G`          | generate a ~6s clip in code — no microphone needed |
| `L`          | reload `assets/recording.wav` |
| `space`      | live mode — loop the clip aloud, picture moves in step |
| `V`          | cycle sketch → surface → points |
| `+` / `-`    | more / less sound displacement (terrain height, in 3D views) |
| `[` / `]`    | coarser / finer stipple |
| `C`          | ink takes the photo's colour, or stays black |
| drag mouse   | orbit the camera (3D views) |
| `↑` / `↓`    | zoom in / out (3D views) |
| `S`          | save a screenshot to `assets/` |

Recordings are saved to `assets/recording.wav` so you can reload them later.

Start with `-` down to zero to see the plain stipple, then bring the sound back
in a step at a time — it's much easier to judge the displacement when you know
what the still drawing looks like.

## Layout

- `src/audio.rs` — mic capture (cpal) + WAV save/load (hound)
- `src/synth.rs` — sound generated in code, for when there's no mic to hand
- `src/analysis.rs` — recording → spectrogram (rustfft)
- `src/stipple.rs` — photo → ink dots, and the field that displaces them
- `src/terrain.rs` — spectrogram + photo → coloured grid, and the 3D projection
- `src/main.rs` — nannou app, input, rendering

## Roadmap

- Export the terrain as an `.obj` for use in Blender
- Live mode driven by the mic directly, rather than by a looping clip
- Export the sketch as SVG, so it can go to a pen plotter
