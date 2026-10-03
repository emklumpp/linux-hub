//! The photo, redrawn as ink.
//!
//! A stipple is a picture made only of dots: no lines, no fills, just density.
//! Dark parts of the photograph get many dots packed close, bright parts get
//! few, and the eye does the rest. We get that distribution from **error
//! diffusion** — walk the picture cell by cell, decide "dot or no dot", and push
//! the rounding error you just made onto the neighbours you haven't visited yet.
//! Each mistake is repaid immediately, so the dots come out evenly spread
//! instead of clumping the way independent random placement would.
//!
//! ```text
//!   photo ──► luminance grid ──► local contrast ──► tone ──► error diffusion
//!                                                              ──► dots ──► jitter
//! ```
//!
//! The dots are stored in **image space** (0..1, origin top-left) and know
//! nothing about the window. [`Wave`] then displaces them with the spectrogram
//! at draw time, which is what turns a still drawing into a moving one.

use crate::analysis::Spectrogram;
use nannou::image;

/// A single mark of ink.
pub struct Dot {
    /// Position in image space: 0..1 across, 0..1 down, origin top-left.
    pub u: f32,
    pub v: f32,
    /// How dark the photo is here (0..1). Fattens the dot, so tone comes from
    /// both how *many* dots there are and how *heavy* each one is.
    pub ink: f32,
    /// The photo's colour at this dot, for the coloured-ink look.
    pub color: [u8; 3],
    /// Per-dot rotation, so the marks don't line up into a visible lattice.
    pub rot: f32,
}

pub struct Stipple {
    pub dots: Vec<Dot>,
    /// Cell grid the dots were diffused over. `cols` sets how big a dot should
    /// be drawn — one cell wide is the natural upper bound.
    pub cols: usize,
    pub rows: usize,
    /// width / height of the source photo, so the drawing keeps its framing.
    pub aspect: f32,
}

/// Coverage ceiling — the largest fraction of cells that may take a dot. Without
/// it a low-key photo asks for a dot in nearly every cell, and since the dots
/// are drawn wider than the cells they sit in, the shadows flood to a solid
/// rectangle with no drawing left in them.
///
/// Approached asymptotically, never reached — see [`shoulder`].
const MAX_COVERAGE: f32 = 0.72;
/// Where the tone curve stops being linear and starts bending onto the ceiling.
/// Below this, coverage tracks darkness exactly; above it, the remaining
/// headroom is stretched to cover every darker tone there is.
const KNEE: f32 = 0.45;
/// Pushes light and dark apart before dithering. Photographs are mostly
/// midtones, and midtones stipple into an even grey mush.
const CONTRAST: f32 = 1.15;
/// > 1 holds the midtones back. Ink reads far darker than the luminance it
/// stands in for — overlapping marks only ever subtract light — so mapping
/// darkness to coverage 1:1 turns every mid-grey into a black hole.
const GAMMA: f32 = 1.25;
/// How hard to dodge and burn — see [`local_contrast`].
const LOCAL_CONTRAST: f32 = 0.7;
/// Ceiling on how much darkness one cell may be shifted by. Texture is
/// small-amplitude and passes through nearly untouched; the cliff at a skyline
/// is not, and without this it throws a halo — see [`local_contrast`].
const LOCAL_LIMIT: f32 = 0.08;
/// Cells of blur defining what counts as "local". Wide enough to sit well
/// outside the texture we want back (leaves, ripples), narrow enough to still
/// be inside the region that flooded.
const LOCAL_RADIUS: usize = 12;
/// Two box blurs approximate a Gaussian, as in [`Wave`].
const LOCAL_PASSES: usize = 2;

impl Stipple {
    /// Redraw `img` as roughly `cells` grid cells' worth of dots.
    ///
    /// The actual dot count lands at `cells × mean darkness` — a high-key photo
    /// yields far fewer marks than a low-key one, which is exactly the point.
    pub fn build(img: &image::RgbImage, cells: usize) -> Self {
        let (iw, ih) = img.dimensions();
        let aspect = iw as f32 / ih.max(1) as f32;

        // Split `cells` into a grid with the photo's proportions.
        let cols = ((cells as f32 * aspect).sqrt().round() as usize).max(2);
        let rows = (cells / cols).max(2);

        let (mut ink, colors) = downsample(img, cols, rows);
        local_contrast(&mut ink, cols, rows);
        for v in ink.iter_mut() {
            *v = tone(*v);
        }

        let dots = diffuse(&mut ink, &colors, cols, rows);

        Stipple {
            dots,
            cols,
            rows,
            aspect,
        }
    }
}

/// Box-average the photo down onto the cell grid, returning per-cell darkness
/// (0..1) and per-cell colour. Averaging rather than point-sampling matters:
/// a single pixel plucked out of foliage is noise, and noise stipples into
/// static.
fn downsample(img: &image::RgbImage, cols: usize, rows: usize) -> (Vec<f32>, Vec<[u8; 3]>) {
    let (iw, ih) = img.dimensions();
    let mut dark = vec![0.0f32; cols * rows];
    let mut colors = vec![[0u8; 3]; cols * rows];

    for row in 0..rows {
        let y0 = (row as f32 / rows as f32 * ih as f32) as u32;
        let y1 = (((row + 1) as f32 / rows as f32 * ih as f32) as u32).clamp(y0 + 1, ih);
        for col in 0..cols {
            let x0 = (col as f32 / cols as f32 * iw as f32) as u32;
            let x1 = (((col + 1) as f32 / cols as f32 * iw as f32) as u32).clamp(x0 + 1, iw);

            let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
            let mut n = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = img.get_pixel(x, y);
                    r += p[0] as f32;
                    g += p[1] as f32;
                    b += p[2] as f32;
                    n += 1.0;
                }
            }
            let n = n.max(1.0);
            let (r, g, b) = (r / n, g / n, b / n);

            let i = row * cols + col;
            // Rec. 601 luma — green carries most of perceived brightness.
            let lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
            dark[i] = 1.0 - lum;
            colors[i] = [r as u8, g as u8, b as u8];
        }
    }

    (dark, colors)
}

/// Lift local detail back out of regions the photograph renders flat.
///
/// A landscape's foreground is often one broad dark mass: grass, water, shadow.
/// Every cell in it asks for near-maximum coverage, so the dots pack in at the
/// same density throughout and the texture that was actually there — the reason
/// to draw the foreground at all — never makes it onto the paper.
///
/// The fix is the darkroom one. Blur the darkness field to get the *large-scale*
/// tone, subtract it to get what each cell is doing relative to its
/// neighbourhood, and add that difference back amplified. Broad masses are left
/// where they are; fine variation within them is exaggerated until it survives
/// being rounded to dot or no-dot. This is dodging and burning, and an unsharp
/// mask, and local histogram equalisation — the same idea under three names.
///
/// Amplifying that difference raw is what gives unsharp masking its bad name.
/// At a hard edge — a skyline against bright cloud — the neighbourhood average
/// is nowhere near either side, so the cells just off the ridge get shoved a
/// long way and the sky beside the mountain loses its ink altogether, ringing
/// the peak in bare paper. So the difference is soft-clipped through `tanh`
/// before it is applied: texture, which is small, passes through very nearly
/// linearly, while an edge saturates at [`LOCAL_LIMIT`] however steep it is.
/// The foreground gets its detail back and the skyline keeps its halo off.
fn local_contrast(dark: &mut [f32], cols: usize, rows: usize) {
    let mut broad = dark.to_vec();
    for _ in 0..LOCAL_PASSES {
        blur(&mut broad, cols, rows, LOCAL_RADIUS);
    }
    for (d, b) in dark.iter_mut().zip(broad) {
        let detail = LOCAL_LIMIT * ((*d - b) / LOCAL_LIMIT).tanh();
        *d = (*d + LOCAL_CONTRAST * detail).clamp(0.0, 1.0);
    }
}

/// Contrast and gamma, then roll the top end onto the coverage ceiling.
fn tone(d: f32) -> f32 {
    let d = d.clamp(0.0, 1.0).powf(GAMMA);
    shoulder((d - 0.5) * CONTRAST + 0.5)
}

/// Linear up to [`KNEE`], then a `tanh` roll-off onto [`MAX_COVERAGE`].
///
/// A hard clamp is what floods the shadows: it maps every tone past the ceiling
/// to the *same* coverage, so a quarter of the frame comes out at one flat
/// density with its structure discarded before a dot is ever placed. `tanh`
/// leaves the curve monotonic — darker always means denser, however little —
/// so the ordering of tones survives all the way down. Its slope is exactly 1
/// at the knee, so the linear section joins it without a visible seam.
fn shoulder(t: f32) -> f32 {
    if t <= KNEE {
        return t.max(0.0);
    }
    let headroom = MAX_COVERAGE - KNEE;
    KNEE + headroom * ((t - KNEE) / headroom).tanh()
}

/// Floyd–Steinberg error diffusion, walked in **serpentine** order — left to
/// right along one row, right to left along the next. Always sweeping the same
/// direction drags the accumulated error into diagonal streaks; alternating
/// cancels them out.
///
/// Every cell is rounded to dot / no-dot, and the error that rounding introduced
/// is spread over the four not-yet-visited neighbours in the classic 7/3/5/1
/// proportions. Over a region the errors cancel, so local dot density tracks
/// local darkness closely.
fn diffuse(ink: &mut [f32], colors: &[[u8; 3]], cols: usize, rows: usize) -> Vec<Dot> {
    let mut dots = Vec::new();

    for row in 0..rows {
        let reverse = row % 2 == 1;
        for step in 0..cols {
            let col = if reverse { cols - 1 - step } else { step };
            let i = row * cols + col;

            let old = ink[i];
            let on = old > 0.5;
            let err = old - if on { 1.0 } else { 0.0 };

            // `ahead` is +1 on a forward row, -1 on a reversed one, so the
            // weights always land on cells we have yet to reach.
            let ahead: isize = if reverse { -1 } else { 1 };
            let mut spread = |dcol: isize, drow: usize, weight: f32| {
                let c = col as isize + dcol * ahead;
                let r = row + drow;
                if c >= 0 && (c as usize) < cols && r < rows {
                    ink[r * cols + c as usize] += err * weight;
                }
            };
            spread(1, 0, 7.0 / 16.0);
            spread(-1, 1, 3.0 / 16.0);
            spread(0, 1, 5.0 / 16.0);
            spread(1, 1, 1.0 / 16.0);

            if !on {
                continue;
            }

            // Jitter inside the cell so the marks read as hand-placed rather
            // than as a grid that happens to have gaps in it. Derived from the
            // cell's coordinates, so the same photo always stipples identically.
            let h = hash(col as u32, row as u32);
            let jx = unit(h) - 0.5;
            let jy = unit(h >> 11) - 0.5;
            let rot = unit(h >> 21) * std::f32::consts::TAU;

            dots.push(Dot {
                u: (col as f32 + 0.5 + jx * 0.8) / cols as f32,
                v: (row as f32 + 0.5 + jy * 0.8) / rows as f32,
                // `old` is this cell's darkness *after* inherited error, which
                // is the value the dot is actually standing in for.
                ink: old.clamp(0.0, 1.0),
                color: colors[i],
                rot,
            });
        }
    }

    dots
}

// --- Sound displacement -----------------------------------------------------

/// The spectrogram read as a force field laid over the picture.
///
/// Across the frame is time, up the frame is frequency, and the value is how
/// loud that band is at that moment — so every point of the photograph has a
/// loudness sitting underneath it. Dots are pushed **away from the average**:
/// where the sound is louder than the clip's norm the drawing bulges upward,
/// where it is quieter it sags. In live mode the field scrolls with the
/// playhead, so the bulge travels across the picture as a wave.
///
/// A second, smaller push runs *sideways*, along the field's horizontal slope.
/// Vertical displacement alone reads as banding; the shear is what makes it look
/// like the picture is being seen through moving water.
///
/// A `Wave` is a pure function of the spectrogram it was built from — building
/// one costs a blur over the whole grid, so the caller caches it and rebuilds
/// only when the spectrogram itself changes. How *hard* the field pushes is not
/// part of it: that is a scalar handed to [`push`](Wave::push) per call, so
/// turning the displacement up and down never touches the field.
pub struct Wave {
    /// A blurred copy of the spectrogram — see [`BLUR_RADIUS`].
    field: Vec<f32>,
    cols: usize,
    rows: usize,
    /// The field's own mean, so displacement is signed rather than all one way.
    mean: f32,
}

/// How far sideways to push, relative to the vertical push.
const SHEAR: f32 = 0.45;

/// Cells of blur applied to the spectrogram before it displaces anything.
///
/// A spectrogram is spiky — harmonics are narrow ridges one or two bins wide,
/// and transients are vertical walls. Displacing by it raw doesn't ripple the
/// picture, it *shears* it: wherever the field's slope is steep the dots either
/// side get pushed different distances and pile into a hard black seam, the way
/// light piles into a caustic. Blurring first keeps the loud/quiet structure but
/// turns those cliffs into swells, which is what actually reads as a wave.
const BLUR_RADIUS: usize = 5;
/// Two box blurs approximate a Gaussian closely enough, and each one is O(n).
const BLUR_PASSES: usize = 2;

impl Wave {
    pub fn new(spec: &Spectrogram) -> Self {
        let (cols, rows) = (spec.cols, spec.rows);
        let mut field = spec.data.clone();
        for _ in 0..BLUR_PASSES {
            blur(&mut field, cols, rows, BLUR_RADIUS);
        }

        let mean = if field.is_empty() {
            0.0
        } else {
            field.iter().sum::<f32>() / field.len() as f32
        };

        Wave {
            field,
            cols,
            rows,
            mean,
        }
    }

    /// Loudness under image-space point `(u, v)`, bilinearly interpolated. The
    /// spectrogram grid is coarser than the dot grid, so nearest-neighbour would
    /// snap whole blocks of dots to the same offset and quantise the wave into
    /// visible steps.
    fn at(&self, u: f32, v: f32) -> f32 {
        let (cols, rows) = (self.cols, self.rows);
        if cols < 2 || rows < 2 {
            return self.mean;
        }
        // v runs down the picture but rows run up in frequency: the top of the
        // frame is the treble, matching how the terrain is laid out.
        let x = u.clamp(0.0, 1.0) * (cols - 1) as f32;
        let y = (1.0 - v.clamp(0.0, 1.0)) * (rows - 1) as f32;

        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(cols - 1), (y0 + 1).min(rows - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);

        let cell = |c: usize, r: usize| self.field[r * cols + c];
        let top = cell(x0, y0) * (1.0 - fx) + cell(x1, y0) * fx;
        let bot = cell(x0, y1) * (1.0 - fx) + cell(x1, y1) * fx;
        top * (1.0 - fy) + bot * fy
    }

    /// Where a dot ends up, and how much the sound is asking it to swell.
    ///
    /// Offsets come back in **image-space units** — the caller scales them by
    /// the on-screen size of the drawing, so the wave keeps its proportions
    /// whatever the window size.
    pub fn push(&self, dot: &Dot, strength: f32) -> Push {
        let e = self.at(dot.u, dot.v);
        let signed = e - self.mean;

        // Horizontal slope of the field, by finite difference. One spectrogram
        // column either side — narrower and we'd be differencing interpolation
        // noise rather than the field.
        let step = 1.0 / self.cols.max(2) as f32;
        let slope = self.at(dot.u + step, dot.v) - self.at(dot.u - step, dot.v);

        Push {
            // Screen y grows upward, so a positive (louder) value lifts.
            dy: signed * strength,
            dx: -slope * strength * SHEAR,
            // Ink pools where it's loud, thins where it's quiet.
            swell: 1.0 + signed * 1.1,
        }
    }
}

pub struct Push {
    pub dx: f32,
    pub dy: f32,
    /// Multiplier on the dot's radius.
    pub swell: f32,
}

/// Separable box blur over a `cols × rows` field, in place.
///
/// Both passes use a running sum, so the cost is one add and one subtract per
/// cell regardless of `radius` — live mode runs this every frame. Edges clamp to
/// the border rather than wrapping: the left edge of the field is the start of
/// the clip, and it should not bleed into the end.
fn blur(field: &mut [f32], cols: usize, rows: usize, radius: usize) {
    if cols < 2 || rows < 2 || radius == 0 {
        return;
    }
    let width = (2 * radius + 1) as f32;
    let mut line: Vec<f32> = Vec::with_capacity(cols.max(rows));

    // Horizontal.
    for row in 0..rows {
        let base = row * cols;
        line.clear();
        line.extend_from_slice(&field[base..base + cols]);
        let get = |i: isize| line[i.clamp(0, cols as isize - 1) as usize];

        let mut sum: f32 = (-(radius as isize)..=radius as isize).map(get).sum();
        for col in 0..cols {
            field[base + col] = sum / width;
            sum += get(col as isize + radius as isize + 1) - get(col as isize - radius as isize);
        }
    }

    // Vertical.
    for col in 0..cols {
        line.clear();
        line.extend((0..rows).map(|r| field[r * cols + col]));
        let get = |i: isize| line[i.clamp(0, rows as isize - 1) as usize];

        let mut sum: f32 = (-(radius as isize)..=radius as isize).map(get).sum();
        for row in 0..rows {
            field[row * cols + col] = sum / width;
            sum += get(row as isize + radius as isize + 1) - get(row as isize - radius as isize);
        }
    }
}

/// A small integer hash — deterministic jitter without pulling in a RNG, and
/// stable across runs so a photo always stipples the same way.
fn hash(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h
}

/// Top 24 bits of a hash as 0.0..1.0.
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}
