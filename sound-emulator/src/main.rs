//! sound-emulator — record the sound of a place, redraw your photograph in ink,
//! and let the sound push the ink around.
//!
//! Two ways to look at the same pair of inputs:
//!
//! ```text
//!   sketch    photo ──► stipple ──┐
//!                                 ├──► dots displaced by sound ──► screen
//!             sound ──► spectrogram field
//!
//!   terrain   photo ──► colour ───┐
//!                                 ├──► displaced 3D grid ──► screen
//!             sound ──► spectrogram height
//! ```
//!
//! Controls (also printed on screen):
//!   R            record a new clip from the mic
//!   G            generate a clip in code (no microphone needed)
//!   L            reload assets/recording.wav
//!   space        live mode — loop the clip aloud, move the picture with it
//!   V            cycle sketch → surface → points
//!   + / -        more / less sound displacement (or terrain height)
//!   [ / ]        coarser / finer stipple
//!   C            ink takes the photo's colour, or stays black
//!   drag         orbit the camera (terrain views)
//!   ↑ / ↓        zoom in / out (terrain views)
//!   S            save a screenshot to assets/
//!
//! Run with `--live` to open straight into live mode.

mod analysis;
mod audio;
mod playback;
mod stipple;
mod synth;
mod terrain;

use analysis::Spectrogram;
use audio::Recording;
use nannou::prelude::*;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use stipple::{Stipple, Wave};
use terrain::{Camera, Terrain};

// --- Tunables ---------------------------------------------------------------
const GRID_COLS: usize = 400; // time resolution
const GRID_ROWS: usize = 225; // frequency resolution (16:9, to match the frame)
// More rows need more bins to divide up, or the log-spaced bands at the bass end
// all collapse onto the same bin and stripe the terrain.
const FFT_SIZE: usize = 4096;
const RECORD_SECS: f32 = 6.0;
const SYNTH_RATE: u32 = 44_100; // sample rate for code-generated clips
const ASSETS: &str = "assets";
const INPUTS: &str = "inputs"; // drop your photograph in here

/// Grid cells the stipple is diffused over. Only the dark ones become dots, so
/// the mark count lands well under this — but every dot is four vertices of mesh
/// rebuilt each frame, so it is the number that governs the frame rate.
const STIPPLE_CELLS: usize = 70_000;
const STIPPLE_MIN: usize = 8_000;
const STIPPLE_MAX: usize = 240_000;

/// Ink on paper, for the sketch view.
const PAPER: (f32, f32, f32) = (0.93, 0.915, 0.885);
const INK: (f32, f32, f32) = (0.09, 0.08, 0.10);

/// What the window is showing.
#[derive(Clone, Copy, PartialEq)]
enum View {
    /// The photo as ink dots, pushed around by the sound.
    Sketch,
    /// The original 3D landscape, as a solid surface.
    Surface,
    /// The original 3D landscape, as a point cloud.
    Points,
}

impl View {
    fn next(self) -> View {
        match self {
            View::Sketch => View::Surface,
            View::Surface => View::Points,
            View::Points => View::Sketch,
        }
    }

    fn label(self) -> &'static str {
        match self {
            View::Sketch => "sketch",
            View::Surface => "surface",
            View::Points => "points",
        }
    }

    fn parse(s: &str) -> Option<View> {
        match s {
            "sketch" => Some(View::Sketch),
            "surface" => Some(View::Surface),
            "points" => Some(View::Points),
            _ => None,
        }
    }
}

fn main() {
    nannou::app(model).update(update).run();
}

struct Model {
    camera: Camera,
    terrain: Option<Terrain>,
    spectrogram: Option<Spectrogram>,
    /// The spectrogram blurred into a displacement field, built only for the
    /// sketch view. Cached because it changes exactly when the spectrogram
    /// does — see [`refresh_wave`].
    wave: Option<Wave>,
    height_scale: f32,
    view: View,
    /// The photo redrawn as dots. Built once per photo — the sound moves the
    /// dots but never re-places them, so this survives every frame.
    stipple: Option<Stipple>,
    stipple_cells: usize,
    /// How hard the sound pushes the ink, in sketch view.
    wave_strength: f32,
    /// Ink in the photo's own colours, rather than black.
    color_ink: bool,
    image_path: Option<PathBuf>,
    // Decoded once and held — live mode rebuilds the terrain every frame and
    // must not re-read the photo from disk each time.
    photo: Option<nannou::image::RgbImage>,
    // The clip currently driving the terrain, kept so live mode can loop it.
    recording: Option<Recording>,
    live: Option<Live>,
    status: String,
    // Receives the finished recording from the capture thread.
    recording_rx: Option<Receiver<anyhow::Result<Recording>>>,
    // Mouse-orbit bookkeeping.
    dragging: bool,
    last_mouse: Vec2,
    // `--capture <path>`: save one frame, then quit. Lets the render be checked
    // without a human at the keyboard.
    capture: Option<PathBuf>,
    captured: bool,
}

/// Live mode: the clip loops through the speakers while the terrain scrolls in
/// step with it — one new column of spectrogram per hop, oldest falling off the
/// far edge. Only the newly-arrived columns are analysed, which is what makes
/// this affordable at frame rate; re-analysing the whole clip would not be.
struct Live {
    playback: playback::Playback,
    analyzer: analysis::FrameAnalyzer,
    samples: Arc<Vec<f32>>,
    /// Ring of raw dB columns, oldest first.
    columns: VecDeque<Vec<f32>>,
    /// Clip-frame index of the next column still to analyse.
    next_start: u64,
    /// Clip frames between columns.
    hop: u64,
    /// Smoothed dB range. Normalising against each frame's own min/max makes the
    /// whole landscape jump about, so we ease the range toward the ring's.
    lo: f32,
    hi: f32,
}

impl Live {
    fn start(rec: &Recording) -> anyhow::Result<Live> {
        let samples = Arc::new(rec.samples.clone());
        let playback = playback::Playback::start(Arc::clone(&samples), rec.sample_rate)?;
        // One column per hop, so the visible window spans the whole clip.
        let hop = (rec.samples.len() / GRID_COLS).max(1) as u64;

        Ok(Live {
            playback,
            analyzer: analysis::FrameAnalyzer::new(GRID_ROWS, FFT_SIZE),
            samples,
            columns: VecDeque::with_capacity(GRID_COLS),
            next_start: 0,
            hop,
            lo: -6.0,
            hi: 0.0,
        })
    }

    /// Catch the ring up to the playhead, then hand back the field to draw.
    fn advance(&mut self) -> Spectrogram {
        let target = self.playback.frames_played();
        let usable = self
            .samples
            .len()
            .saturating_sub(self.analyzer.fft_size())
            .max(1) as u64;

        // If we've fallen more than a screenful behind — a stall, or the window
        // was hidden — skip ahead rather than grinding through every missed
        // column just to throw them all away.
        let screenful = self.hop * GRID_COLS as u64;
        if target > self.next_start + screenful {
            self.next_start = target.saturating_sub(screenful);
        }

        while self.next_start + self.hop <= target {
            let start = (self.next_start % usable) as usize;
            if let Some(column) = self.analyzer.column(&self.samples, start) {
                if self.columns.len() == GRID_COLS {
                    self.columns.pop_front();
                }
                self.columns.push_back(column);
            }
            self.next_start += self.hop;
        }

        self.field()
    }

    /// Normalise the ring into a 0..1 height field.
    fn field(&mut self) -> Spectrogram {
        let (cols, rows) = (GRID_COLS, GRID_ROWS);
        let mut data = vec![0.0f32; cols * rows];

        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for column in &self.columns {
            for &v in column {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if lo.is_finite() && hi.is_finite() {
            self.lo += (lo - self.lo) * 0.08;
            self.hi += (hi - self.hi) * 0.08;
        }
        let range = (self.hi - self.lo).max(1e-6);

        // Right-align, so a partly-filled ring grows in from the near edge.
        let offset = cols - self.columns.len();
        for (i, column) in self.columns.iter().enumerate() {
            for (row, &db) in column.iter().enumerate() {
                data[row * cols + offset + i] = ((db - self.lo) / range).clamp(0.0, 1.0);
            }
        }

        Spectrogram { cols, rows, data }
    }
}

fn model(app: &App) -> Model {
    app.new_window()
        .size(1280, 720)
        .title("sound-emulator")
        .view(view)
        .key_pressed(key_pressed)
        .build()
        .unwrap();

    let mut model = Model {
        camera: Camera::default(),
        terrain: None,
        spectrogram: None,
        wave: None,
        height_scale: 6.0,
        view: flag_value("--view")
            .and_then(|v| View::parse(&v))
            .unwrap_or(View::Sketch),
        stipple: None,
        stipple_cells: flag_value("--cells")
            .and_then(|v| v.parse().ok())
            .unwrap_or(STIPPLE_CELLS)
            .clamp(STIPPLE_MIN, STIPPLE_MAX),
        // A fraction of the drawing's height. The spectrogram swings about
        // ±0.5 either side of its mean, so this is roughly the furthest a dot
        // travels — enough to ripple the picture, not enough to lose it.
        wave_strength: flag_value("--wave").and_then(|v| v.parse().ok()).unwrap_or(0.06),
        color_ink: std::env::args().any(|a| a == "--colour" || a == "--color"),
        image_path: None,
        photo: None,
        recording: None,
        live: None,
        status: "Press G to generate sound, then space for live mode.".to_string(),
        recording_rx: None,
        dragging: false,
        last_mouse: Vec2::ZERO,
        capture: flag_value("--capture").map(PathBuf::from),
        captured: false,
    };
    refresh_photo(&mut model);

    // If a clip already exists on disk, show it immediately.
    let wav = Path::new(ASSETS).join("recording.wav");
    if wav.exists() {
        match audio::load_wav(&wav) {
            Ok(rec) => build_from_recording(&mut model, &rec),
            Err(e) => model.status = format!("couldn't load {}: {e}", wav.display()),
        }
    }

    // `--live` opens straight into live mode, generating a clip if there isn't
    // one, so the landscape is already moving when the window appears.
    if std::env::args().any(|a| a == "--live") {
        toggle_live(&mut model);
    }

    model
}

fn update(app: &App, model: &mut Model, _update: Update) {
    // `--capture`: give the scene a moment to settle, save a frame, then quit.
    if let Some(path) = model.capture.clone() {
        if !model.captured && app.time > 2.5 {
            let _ = std::fs::create_dir_all(ASSETS);
            app.main_window().capture_frame(&path);
            model.captured = true;
        } else if model.captured && app.time > 4.5 {
            // capture_frame writes asynchronously, so leave it time to land.
            app.quit();
        }
    }

    // Live mode re-reads the field from the playhead every frame. The sketch
    // view displaces its dots straight from the spectrogram, so it only needs
    // the field — rebuilding 90,000 terrain vertices it would never draw is
    // the single most expensive thing we could do here.
    if let Some(spec) = model.live.as_mut().map(|live| live.advance()) {
        if model.view != View::Sketch {
            model.terrain = Some(Terrain::build(
                &spec,
                model.photo.as_ref(),
                model.height_scale,
            ));
        }
        model.spectrogram = Some(spec);
        refresh_wave(model);
    }

    // Poll the capture thread for a finished recording.
    if let Some(rx) = &model.recording_rx {
        match rx.try_recv() {
            Ok(Ok(rec)) => {
                let wav = Path::new(ASSETS).join("recording.wav");
                let _ = std::fs::create_dir_all(ASSETS);
                if let Err(e) = audio::save_wav(&rec, &wav) {
                    eprintln!("could not save wav: {e}");
                }
                build_from_recording(model, &rec);
                model.recording_rx = None;
            }
            Ok(Err(e)) => {
                model.status = format!("recording failed: {e}");
                model.recording_rx = None;
            }
            Err(TryRecvError::Empty) => {} // still recording
            Err(TryRecvError::Disconnected) => {
                model.status = "recording thread vanished".to_string();
                model.recording_rx = None;
            }
        }
    }

    // Left-drag orbits the camera.
    let pos = vec2(app.mouse.x, app.mouse.y);
    if app.mouse.buttons.left().is_down() {
        if model.dragging {
            let d = pos - model.last_mouse;
            model.camera.yaw += d.x * 0.008;
            model.camera.pitch = (model.camera.pitch + d.y * 0.008).clamp(-1.4, 1.4);
        }
        model.dragging = true;
    } else {
        model.dragging = false;
    }
    model.last_mouse = pos;
}

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::R => start_recording(model),
        Key::G => generate_sound(model),
        Key::L => {
            let wav = Path::new(ASSETS).join("recording.wav");
            match audio::load_wav(&wav) {
                Ok(rec) => build_from_recording(model, &rec),
                Err(e) => model.status = format!("load failed: {e}"),
            }
        }
        Key::Space => toggle_live(model),
        Key::V => {
            model.view = model.view.next();
            // Coming back to a terrain view mid-clip, whatever grid we have is
            // from before the switch — put it back in step with the sound.
            if model.view != View::Sketch {
                rebuild_terrain(model);
            }
            refresh_wave(model);
            model.status = format!("{} view", model.view.label());
        }
        Key::C => {
            model.color_ink = !model.color_ink;
            model.status = if model.color_ink {
                "ink takes the photo's colour".to_string()
            } else {
                "black ink".to_string()
            };
        }
        Key::LBracket => set_stipple_cells(model, model.stipple_cells * 3 / 4),
        Key::RBracket => set_stipple_cells(model, model.stipple_cells * 4 / 3),
        Key::Up => model.camera.zoom = (model.camera.zoom * 1.1).min(6.0),
        Key::Down => model.camera.zoom = (model.camera.zoom / 1.1).max(0.2),
        // `+`/`-` mean "more sound" in both views — it just reaches a different
        // dial depending on what is on screen.
        Key::Plus | Key::Equals => {
            if model.view == View::Sketch {
                model.wave_strength = (model.wave_strength + 0.01).min(0.6);
                model.status = format!("displacement {:.2}", model.wave_strength);
            } else {
                model.height_scale = (model.height_scale + 1.0).min(30.0);
                rebuild_terrain(model);
            }
        }
        Key::Minus => {
            if model.view == View::Sketch {
                model.wave_strength = (model.wave_strength - 0.01).max(0.0);
                model.status = format!("displacement {:.2}", model.wave_strength);
            } else {
                model.height_scale = (model.height_scale - 1.0).max(0.0);
                rebuild_terrain(model);
            }
        }
        Key::S => {
            let path = Path::new(ASSETS).join(output_name(model.view));
            let _ = std::fs::create_dir_all(ASSETS);
            app.main_window().capture_frame(&path);
            model.status = format!("saved {}", path.display());
        }
        _ => {}
    }
}

/// Kick off microphone capture on a worker thread so the UI keeps drawing.
fn start_recording(model: &mut Model) {
    if model.recording_rx.is_some() {
        return; // already recording
    }
    let (tx, rx) = std::sync::mpsc::channel();
    model.recording_rx = Some(rx);
    model.status = format!("● recording {RECORD_SECS:.0}s — make some noise…");
    std::thread::spawn(move || {
        let _ = tx.send(audio::record(RECORD_SECS));
    });
}

/// Build a landscape from sound synthesised in code — no microphone involved.
/// Saved to the same path as a recording so `L` reloads it like any other clip.
fn generate_sound(model: &mut Model) {
    let rec = synth::soundscape(SYNTH_RATE, RECORD_SECS);
    let wav = Path::new(ASSETS).join("recording.wav");
    let _ = std::fs::create_dir_all(ASSETS);
    if let Err(e) = audio::save_wav(&rec, &wav) {
        eprintln!("could not save wav: {e}");
    }
    build_from_recording(model, &rec);
    model.status = format!("generated · {}", model.status);
}

/// Analyse a recording and rebuild the terrain from it.
fn build_from_recording(model: &mut Model, rec: &Recording) {
    // Re-scan so a photo dropped into inputs/ after launch is picked up by R / L
    // without restarting the app.
    refresh_photo(model);

    let spec = analysis::compute_spectrogram(rec, GRID_COLS, GRID_ROWS, FFT_SIZE);
    model.terrain = Some(Terrain::build(
        &spec,
        model.photo.as_ref(),
        model.height_scale,
    ));
    model.spectrogram = Some(spec);
    refresh_wave(model);
    model.recording = Some(rec.clone());

    let photo = match (&model.image_path, &model.photo) {
        (Some(p), Some(_)) => p.display().to_string(),
        (Some(p), None) => format!("{} unreadable — gradient", p.display()),
        _ => "no photo (gradient)".to_string(),
    };
    model.status = format!(
        "{:.1}s of sound · {} · space for live, drag to orbit",
        rec.duration_secs(),
        photo
    );
}

/// Rebuild the displacement field from whatever spectrogram we now hold.
///
/// [`Wave`] is a pure function of the spectrogram, and building one blurs the
/// whole 400×225 grid — so it is cached here and rebuilt exactly when the
/// spectrogram changes, instead of once per frame the way the drawing code used
/// to do it. On a still picture that is every frame's worth of work saved; in
/// live mode the spectrogram really does change each frame, so the cost is the
/// same as before, just paid in `update` rather than in `view`.
///
/// Only the sketch view reads the field, so the terrain views never pay for it
/// at all — which is why switching views has to come back through here.
fn refresh_wave(model: &mut Model) {
    model.wave = match (&model.spectrogram, model.view) {
        (Some(spec), View::Sketch) => Some(Wave::new(spec)),
        _ => None,
    };
}

/// Re-displace the existing spectrogram after a height-scale change.
fn rebuild_terrain(model: &mut Model) {
    if let Some(spec) = &model.spectrogram {
        model.terrain = Some(Terrain::build(
            spec,
            model.photo.as_ref(),
            model.height_scale,
        ));
    }
}

/// Re-scan for a photo, decoding it only when the choice actually changes.
fn refresh_photo(model: &mut Model) {
    let path = find_photo();
    if path == model.image_path {
        return;
    }
    model.photo = path.as_deref().and_then(terrain::load_photo);
    model.image_path = path;
    rebuild_stipple(model);
}

/// Re-stipple the photo. Slow enough to notice — it walks every pixel of a
/// full-size Fuji frame — so it happens only when the photo or the density
/// changes, never per frame.
fn rebuild_stipple(model: &mut Model) {
    model.stipple = model
        .photo
        .as_ref()
        .map(|img| Stipple::build(img, model.stipple_cells));
}

fn set_stipple_cells(model: &mut Model, cells: usize) {
    model.stipple_cells = cells.clamp(STIPPLE_MIN, STIPPLE_MAX);
    rebuild_stipple(model);
    model.status = match &model.stipple {
        Some(s) => format!(
            "{} cells · {} dots",
            model.stipple_cells,
            s.dots.len()
        ),
        None => "no photo to stipple".to_string(),
    };
}

/// Toggle live mode: loop the current clip aloud and scroll the terrain with it.
fn toggle_live(model: &mut Model) {
    if model.live.take().is_some() {
        model.status = "live mode off".to_string();
        return;
    }

    // Nothing to play yet? Generate something, so space always does something.
    if model.recording.is_none() {
        generate_sound(model);
    }
    let Some(rec) = model.recording.clone() else {
        return;
    };

    match Live::start(&rec) {
        Ok(live) => {
            model.live = Some(live);
            model.status = "▶ live — clip looping, terrain scrolling with it".to_string();
        }
        Err(e) => model.status = format!("live mode failed: {e}"),
    }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    let win = app.window_rect();
    let sketch = model.view == View::Sketch;

    // Paper for the drawing, night for the landscape.
    draw.background().color(if sketch {
        rgb(PAPER.0, PAPER.1, PAPER.2)
    } else {
        rgb(0.04, 0.04, 0.06)
    });

    if sketch {
        draw_sketch(&draw, model, win);
    } else if let Some(terrain) = &model.terrain {
        draw_terrain(&draw, &model.camera, terrain, model.view == View::Points);
    }

    // HUD — dark type on paper, light type on night.
    let (dim, bright) = if sketch {
        (rgb(0.45, 0.43, 0.40), rgb(0.20, 0.19, 0.22))
    } else {
        (rgb(0.5, 0.55, 0.6), rgb(0.85, 0.87, 0.9))
    };
    draw.text(&format!("{:.0} fps", app.fps()))
        .xy(vec2(win.right() - 60.0, win.top() - 20.0))
        .font_size(13)
        .color(dim);
    draw.text(&model.status)
        .xy(vec2(0.0, win.bottom() + 24.0))
        .font_size(15)
        .color(bright)
        .w(win.w() - 40.0);

    draw.to_frame(app, &frame).unwrap();
}

/// The sketch view: the stipple, with every dot shoved off its mark by the
/// sound.
///
/// The dots are one mesh, not one `draw.ellipse()` each — tens of thousands of
/// individual shapes a frame will not draw in any useful time. At this size a
/// mark is two or three pixels across, so each one is a little quad, spun to its
/// own angle so the grid it came from never shows through.
fn draw_sketch(draw: &Draw, model: &Model, win: Rect) {
    let Some(stipple) = &model.stipple else {
        draw.text("drop a photo into inputs/ — jpg, png or tiff")
            .xy(vec2(0.0, 0.0))
            .font_size(18)
            .color(rgb(0.35, 0.33, 0.31));
        return;
    };

    // Fit the photo's own frame inside the window, leaving room for the HUD.
    let margin = 46.0;
    let avail_w = (win.w() - margin * 2.0).max(1.0);
    let avail_h = (win.h() - margin * 2.0 - 24.0).max(1.0);
    let (w, h) = if avail_w / avail_h > stipple.aspect {
        (avail_h * stipple.aspect, avail_h)
    } else {
        (avail_w, avail_w / stipple.aspect)
    };

    // One dot is at most one cell across; a little under keeps the paper showing
    // through in the shadows instead of flooding to solid black. The grid is
    // only approximately the photo's proportions — integer rows and columns
    // rarely divide evenly — so take the tighter of the two.
    let cell = (w / stipple.cols as f32).min(h / stipple.rows as f32);
    // `r` is a half-width, so a mark spans `2r`. Keeping that under one cell
    // means even a fully-covered region still shows paper between the marks.
    let base_r = cell * 0.42;

    let wave = model.wave.as_ref();

    let mut points: Vec<(Point3, Srgb)> = Vec::with_capacity(stipple.dots.len() * 4);
    let mut indices: Vec<usize> = Vec::with_capacity(stipple.dots.len() * 6);

    for dot in &stipple.dots {
        let push = wave.map(|w| w.push(dot, model.wave_strength));
        let (dx, dy, swell) = match &push {
            Some(p) => (p.dx, p.dy, p.swell.max(0.15)),
            None => (0.0, 0.0, 1.0),
        };

        // Image space (origin top-left, y down) into window space (origin
        // centre, y up). Displacement is in image-space units, so it scales
        // with the drawing rather than drifting as the window resizes.
        let x = (dot.u - 0.5 + dx) * w;
        let y = (0.5 - dot.v + dy) * h;

        // Heavier ink where the photo is dark, heavier again where it is loud.
        let r = base_r * (0.55 + 0.6 * dot.ink) * swell;

        let color = if model.color_ink {
            // Knocked back toward the paper — full-strength photo colour reads
            // as a mosaic rather than as ink.
            let c = |v: u8| (v as f32 / 255.0) * 0.78;
            srgb(c(dot.color[0]), c(dot.color[1]), c(dot.color[2]))
        } else {
            srgb(INK.0, INK.1, INK.2)
        };

        let (s, c) = dot.rot.sin_cos();
        let base = points.len();
        for (ox, oy) in [(-r, -r), (r, -r), (r, r), (-r, r)] {
            points.push((pt3(x + ox * c - oy * s, y + ox * s + oy * c, 0.0), color));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    draw.mesh().indexed_colored(points, indices);
}

fn draw_terrain(draw: &Draw, cam: &Camera, terrain: &Terrain, points_only: bool) {
    // Project every vertex once.
    let projected: Vec<terrain::Projected> =
        terrain.verts.iter().map(|v| cam.project(v.pos)).collect();

    if points_only {
        draw_points(draw, cam, terrain, &projected);
        return;
    }

    // Solid surface. We project by hand and hand nannou flat 2D triangles, so
    // submission order — not a depth buffer — is what resolves overlap: emit the
    // far quads first and let the near ones paint over them.
    let (cols, rows) = (terrain.cols, terrain.rows);
    let mut quads: Vec<(f32, usize)> = Vec::with_capacity(cols.saturating_sub(1) * rows.saturating_sub(1));
    for row in 0..rows.saturating_sub(1) {
        for col in 0..cols.saturating_sub(1) {
            let i = row * cols + col;
            // Sum rather than mean — we only need the ordering, not the value.
            let depth = projected[i].depth
                + projected[i + 1].depth
                + projected[i + cols].depth
                + projected[i + cols + 1].depth;
            quads.push((depth, i));
        }
    }
    quads.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));

    let mut indices = Vec::with_capacity(quads.len() * 6);
    for &(_, i) in &quads {
        indices.extend_from_slice(&[i, i + 1, i + cols, i + 1, i + cols + 1, i + cols]);
    }

    let points = terrain.verts.iter().zip(&projected).map(|(v, p)| {
        (
            pt3(p.screen.x, p.screen.y, 0.0),
            srgb(
                v.color.red as f32 / 255.0,
                v.color.green as f32 / 255.0,
                v.color.blue as f32 / 255.0,
            ),
        )
    });

    draw.mesh().indexed_colored(points, indices);
}

/// The original point-cloud look, kept on `W`.
///
/// Decimated to roughly the density it had before the grid grew dense enough to
/// read as a photograph — 90,000 individual ellipses a frame would not draw in
/// any useful time.
fn draw_points(draw: &Draw, cam: &Camera, terrain: &Terrain, projected: &[terrain::Projected]) {
    const TARGET_POINTS: f32 = 12_600.0;
    let stride = (((terrain.cols * terrain.rows) as f32 / TARGET_POINTS).sqrt() as usize).max(1);

    let mut order: Vec<usize> = (0..terrain.rows)
        .step_by(stride)
        .flat_map(|row| (0..terrain.cols).step_by(stride).map(move |col| row * terrain.cols + col))
        .collect();
    order.sort_unstable_by(|&a, &b| projected[b].depth.total_cmp(&projected[a].depth));

    for &i in &order {
        let p = &projected[i];
        let c = terrain.verts[i].color;
        // Points shrink with distance for depth cueing.
        let r = (900.0 / p.depth).clamp(1.5, 7.0) * cam.zoom.sqrt() * stride as f32 * 0.5;
        draw.ellipse()
            .xy(p.screen)
            .radius(r)
            .color(rgb(c.red, c.green, c.blue));
    }
}

/// Value following `flag` on the command line, if present.
fn flag_value(flag: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(a) = args.next() {
        if a == flag {
            return args.next();
        }
    }
    None
}

/// Look for a photo in `inputs/` first, falling back to `assets/`.
fn find_photo() -> Option<PathBuf> {
    [INPUTS, ASSETS]
        .iter()
        .find_map(|dir| first_image(Path::new(dir)))
}

/// The alphabetically-first image anywhere under `dir`, searched recursively so
/// you can keep `inputs/` organised into subfolders.
fn first_image(dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    collect_images(dir, &mut candidates);
    candidates.sort();
    candidates.into_iter().next()
}

fn collect_images(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        if path.is_dir() {
            collect_images(&path, out);
        } else if is_loadable_image(&path) {
            out.push(path);
        }
    }
}

/// Where `S` saves a screenshot, per view. Separate names so saving a sketch
/// never overwrites a landscape you meant to keep.
fn output_name(view: View) -> &'static str {
    match view {
        View::Sketch => "sketch.png",
        View::Surface | View::Points => "landscape.png",
    }
}

/// Formats nannou's `image` crate can decode. HEIF/HEIC are deliberately absent —
/// it can't read them, so convert those to PNG before dropping them in.
fn is_loadable_image(p: &Path) -> bool {
    let ext_ok = matches!(
        p.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("tif") | Some("tiff")
    );
    // Our own screenshots live in assets/ — don't drape the app over itself, or
    // stipple a picture that is already made of dots.
    let own_output = p
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| [View::Sketch, View::Surface].iter().any(|&v| output_name(v) == n));
    ext_ok && !own_output
}
