//! The bridge between the photo and the sound: a grid of coloured, displaced
//! points that we render with a hand-rolled 3D → 2D projection.
//!
//! Each grid vertex gets:
//!   * its **colour** from the corresponding pixel of your Fuji photo, and
//!   * its **height** from the spectrogram magnitude at that spot.
//!
//! So the picture is the skin of the landscape and the sound is its relief.

use crate::analysis::Spectrogram;
use nannou::image;
use nannou::prelude::*;
use std::path::Path;

/// World size of the grid, in the units `Camera::project` scales up by. Fixed so
/// the framing holds steady as the grid resolution changes.
const REF_COLS: f32 = 150.0;
const REF_ROWS: f32 = 84.0;

pub struct Vertex {
    /// Position in grid space, centred on the origin. x = time, z = frequency,
    /// y = height (up). Units are roughly "one grid cell = 1.0".
    pub pos: Vec3,
    pub color: Rgb<u8>,
}

pub struct Terrain {
    pub cols: usize,
    pub rows: usize,
    pub verts: Vec<Vertex>,
}

impl Terrain {
    /// Combine a spectrogram (height) with an image (colour) into a grid.
    ///
    /// If `image_path` can't be loaded we fall back to a synthetic gradient so
    /// the app always shows *something* — swap in a real photo whenever ready.
    pub fn build(spec: &Spectrogram, img: Option<&image::RgbImage>, height_scale: f32) -> Self {
        let cols = spec.cols;
        let rows = spec.rows;

        let mut verts = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let base = match img {
                    Some(img) => sample_image(img, col, row, cols, rows),
                    None => gradient(col, row, cols, rows),
                };

                // Elevation shading: darken valleys, brighten peaks. Driven by
                // the normalised (0..1) spectrogram value, not the scaled height,
                // so the look holds steady as you exaggerate relief with +/-.
                let elev = spec.at(col, row);
                let color = shade(base, elev);

                let h = elev * height_scale;
                // Centre the grid on the origin so it orbits about its middle,
                // and always lay it out at the same world size whatever the
                // resolution — raising `cols`/`rows` should sharpen the picture,
                // not change how much of it is on screen.
                let x = (col as f32 / (cols.max(2) - 1) as f32 - 0.5) * REF_COLS;
                let z = (row as f32 / (rows.max(2) - 1) as f32 - 0.5) * REF_ROWS;
                verts.push(Vertex {
                    pos: vec3(x, h, z),
                    color,
                });
            }
        }

        Terrain { cols, rows, verts }
    }

}

/// Decode a photo once, up front.
///
/// Live mode rebuilds the terrain every frame, so the image must be decoded and
/// held rather than re-read from disk each time. Returns `None` (and says why)
/// on anything `image` can't read — HEIF/HEIC off a phone, most often — and the
/// caller falls back to the placeholder gradient.
pub fn load_photo(path: &Path) -> Option<image::RgbImage> {
    match image::open(path) {
        Ok(i) => Some(i.to_rgb8()),
        Err(e) => {
            eprintln!(
                "could not load image ({}): {e} — using gradient",
                path.display()
            );
            None
        }
    }
}

/// Multiply a colour by a brightness derived from elevation (0..1). Valleys land
/// around 0.5× (in shadow), peaks around 1.35× (catching the light) — a linear
/// ramp that makes the relief read strongly without washing colours out.
fn shade(c: Rgb<u8>, elev: f32) -> Rgb<u8> {
    let factor = 0.5 + 0.85 * elev.clamp(0.0, 1.0);
    let ch = |v: u8| (v as f32 * factor).clamp(0.0, 255.0) as u8;
    rgb(ch(c.red), ch(c.green), ch(c.blue))
}

/// Nearest-pixel sample of the image mapped onto the grid. Rows are flipped so
/// the top of the photo lands at the far (high-frequency) edge of the terrain.
fn sample_image(
    img: &image::RgbImage,
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
) -> Rgb<u8> {
    let (iw, ih) = img.dimensions();
    let u = col as f32 / (cols.max(2) - 1) as f32;
    let v = 1.0 - row as f32 / (rows.max(2) - 1) as f32;
    let px = ((u * (iw - 1) as f32) as u32).min(iw - 1);
    let py = ((v * (ih - 1) as f32) as u32).min(ih - 1);
    let p = img.get_pixel(px, py);
    rgb(p[0], p[1], p[2])
}

/// A placeholder colour ramp used when no photo is supplied.
fn gradient(col: usize, row: usize, cols: usize, rows: usize) -> Rgb<u8> {
    let u = col as f32 / cols.max(1) as f32;
    let v = row as f32 / rows.max(1) as f32;
    rgb(
        (40.0 + 180.0 * u) as u8,
        (60.0 + 120.0 * v) as u8,
        (140.0 + 100.0 * (1.0 - u)) as u8,
    )
}

/// Result of projecting a world-space point to the screen.
pub struct Projected {
    pub screen: Vec2,
    /// Camera-space depth — larger is farther. Used to sort back-to-front and to
    /// scale point size for a sense of distance.
    pub depth: f32,
}

/// Orbit camera state. Yaw spins around the vertical axis, pitch tilts up/down,
/// zoom scales the whole thing, and `focal`/`cam_dist` set the perspective.
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            yaw: 0.6,
            pitch: 0.5,
            // Sized so the whole landscape lands inside a 1280-wide window
            // rather than opening part-way inside it.
            zoom: 0.24,
        }
    }
}

impl Camera {
    /// Project a grid-space point to the screen with a simple perspective.
    /// All the "3D" lives here — the rest of the app draws in plain 2D.
    pub fn project(&self, p: Vec3) -> Projected {
        // Scale grid units up to pixels first.
        let cell = 22.0;
        let p = p * cell;

        // Rotate around Y (yaw).
        let (sy, cy) = self.yaw.sin_cos();
        let x1 = p.x * cy + p.z * sy;
        let z1 = -p.x * sy + p.z * cy;
        let y1 = p.y;

        // Rotate around X (pitch).
        let (sp, cp) = self.pitch.sin_cos();
        let y2 = y1 * cp - z1 * sp;
        let z2 = y1 * sp + z1 * cp;

        // Perspective divide. `cam_dist` has to clear the grid's own depth
        // extent (±~1700 units once yaw and pitch are applied) or the near edge
        // crosses the camera plane, gets clamped, and smears off screen. `focal`
        // scales with it to hold the field of view steady.
        let cam_dist = 2800.0;
        let focal = 2200.0;
        let depth = z2 + cam_dist;
        let f = focal / depth.max(1.0) * self.zoom;

        Projected {
            screen: vec2(x1 * f, y2 * f),
            depth,
        }
    }
}
