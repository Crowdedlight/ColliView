//! Image registration for the paper-target centring.
//!
//! Turning the adapter 180° turns the image of a sheet of paper fixed in the tube about
//! one point: where the focuser axis meets the paper. [`register`] finds the affine map
//! between two views of the paper, `x' = A x + t` in full-frame pixels, and
//! [`Registration::fixed_point`] solves for that point.
//!
//! Each view is cropped around the focuser ring (as fitted in the overlay, the same ring
//! for every view) and turned into a pyramid. Every level is
//! high-passed on its own, so bold pen marks and the target's large blocks still dominate
//! the coarse levels. It is normalised inside a disc well within the ring: the drawtube rim sits at
//! another depth and would pull the result towards its own centre. A brute-force ZNCC
//! search at the coarsest level finds the rough angle and shift, and notices when the
//! pattern repeats (plain squared paper). Gauss–Newton then refines all six parameters
//! from the coarsest level to the finest.

use egui::{Color32, ColorImage, Pos2, Vec2, pos2, vec2};

use crate::detect::GrayImage;

/// The mask is a disc of this fraction of the ring radius. The margin keeps the rim out
/// while the carrier screws move the camera in Live: the mask stays put, and the rim,
/// being nearer, moves further in the image than the paper.
pub const MASK_FRAC: f32 = 0.8;
/// The crop is a square of this fraction of the ring radius around the ring centre.
const CROP_FRAC: f32 = 1.08;
/// Band-pass σs, in pixels of each level. The small one removes detail that bilinear
/// interpolation can't follow, which would pull sub-pixel shifts towards whole pixels.
const SMOOTH_SIGMA: f32 = 1.0;
const HIGHPASS_SIGMA: f32 = 6.0;
/// The coarsest level's mask radius stays at least this many pixels.
const COARSE_RADIUS: f32 = 16.0;
const MAX_LEVELS: usize = 7;
/// A second match peak at least this good, relative to the best, means the pattern repeats.
const AMBIGUOUS_RATIO: f32 = 0.92;
/// Peaks closer than this (pixels of the coarsest level) are the same peak.
const PEAK_SEPARATION: f32 = 3.0;

/// Pyramid levels for a ring of this radius: the coarsest is the last one whose mask
/// radius is still at least `COARSE_RADIUS` pixels.
pub fn level_count(ring_radius: f32) -> usize {
    let r = MASK_FRAC * ring_radius;
    let mut n = 1;
    while n < MAX_LEVELS && r / (1u32 << n) as f32 >= COARSE_RADIUS {
        n += 1;
    }
    n
}

/// One pyramid level. Level coordinates are full-frame pixels / 2^level.
struct Level {
    /// Level coordinate of pixel (0, 0)'s top-left corner.
    off: Vec2,
    w: usize,
    h: usize,
    /// High-passed, zero mean and unit variance inside the mask.
    img: Vec<f32>,
    gx: Vec<f32>,
    gy: Vec<f32>,
}

impl Level {
    fn new(raw: &GrayImage, off: Vec2, mask_c: Pos2, mask_r: f32) -> Self {
        let (w, h) = (raw.width, raw.height);
        let mut lvl = Self { off, w, h, img: Vec::new(), gx: Vec::new(), gy: Vec::new() };
        // The wide blur only averages inside the mask (normalised convolution). Otherwise
        // the rim's edge leaks into the coarse levels: it is symmetric about the rim
        // centre, so it would pull the coarse search there and hide a repeating pattern.
        let inside: Vec<f32> = (0..h)
            .flat_map(|j| (0..w).map(move |i| (i, j)))
            .map(|(i, j)| ((lvl.at(i, j) - mask_c).length() <= mask_r) as u8 as f32)
            .collect();
        let masked =
            GrayImage { width: w, height: h, data: raw.data.iter().zip(&inside).map(|(v, m)| v * m).collect() };
        let weight = GrayImage { width: w, height: h, data: inside }.blurred(HIGHPASS_SIGMA);
        let coarse = masked.blurred(HIGHPASS_SIGMA);
        let fine = raw.blurred(SMOOTH_SIGMA);
        let mut img: Vec<f32> = (0..w * h).map(|k| fine.data[k] - coarse.data[k] / weight.data[k].max(1e-3)).collect();
        let (mut n, mut s1, mut s2) = (0.0f64, 0.0f64, 0.0f64);
        for j in 0..h {
            for i in 0..w {
                if (lvl.at(i, j) - mask_c).length() <= mask_r {
                    let v = img[j * w + i] as f64;
                    n += 1.0;
                    s1 += v;
                    s2 += v * v;
                }
            }
        }
        let mean = s1 / n.max(1.0);
        let sd = (s2 / n.max(1.0) - mean * mean).max(0.0).sqrt().max(1e-3);
        for v in &mut img {
            *v = ((*v as f64 - mean) / sd) as f32;
        }
        let at = |x: usize, y: usize| img[y * w + x];
        let mut gx = vec![0.0; w * h];
        let mut gy = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                let (xa, xb) = (x.saturating_sub(1), (x + 1).min(w - 1));
                let (ya, yb) = (y.saturating_sub(1), (y + 1).min(h - 1));
                gx[y * w + x] = (at(xb, y) - at(xa, y)) / (xb - xa).max(1) as f32;
                gy[y * w + x] = (at(x, yb) - at(x, ya)) / (yb - ya).max(1) as f32;
            }
        }
        lvl.img = img;
        lvl.gx = gx;
        lvl.gy = gy;
        lvl
    }

    /// Level coordinate of pixel (i, j)'s centre.
    fn at(&self, i: usize, j: usize) -> Pos2 {
        pos2(self.off.x + i as f32 + 0.5, self.off.y + j as f32 + 0.5)
    }

    /// Bilinear weights at a level coordinate, or `None` outside the level.
    fn taps(&self, g: Pos2) -> Option<([usize; 4], f32, f32)> {
        let x = g.x - self.off.x - 0.5;
        let y = g.y - self.off.y - 0.5;
        // Written to be false for NaN as well.
        if !(x >= 0.0 && y >= 0.0 && x <= (self.w - 1) as f32 && y <= (self.h - 1) as f32) {
            return None;
        }
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let idx = [y0 * self.w + x0, y0 * self.w + x1, y1 * self.w + x0, y1 * self.w + x1];
        Some((idx, x - x0 as f32, y - y0 as f32))
    }

    fn value(&self, g: Pos2) -> Option<f32> {
        let (i, fx, fy) = self.taps(g)?;
        Some(bilinear(&self.img, i, fx, fy))
    }

    /// Value and gradient.
    fn sample(&self, g: Pos2) -> Option<[f32; 3]> {
        let (i, fx, fy) = self.taps(g)?;
        Some([bilinear(&self.img, i, fx, fy), bilinear(&self.gx, i, fx, fy), bilinear(&self.gy, i, fx, fy)])
    }
}

fn bilinear(d: &[f32], [a, b, c, e]: [usize; 4], fx: f32, fy: f32) -> f32 {
    let top = d[a] + (d[b] - d[a]) * fx;
    let bottom = d[c] + (d[e] - d[c]) * fx;
    top + (bottom - top) * fy
}

fn luma(c: Color32) -> f32 {
    0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
}

/// 2 × 2 box average.
fn downsample(img: &GrayImage) -> GrayImage {
    let (w, h) = (img.width / 2, img.height / 2);
    let mut data = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let at = |dx: usize, dy: usize| img.data[(2 * y + dy) * img.width + 2 * x + dx];
            data.push(0.25 * (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)));
        }
    }
    GrayImage { width: w, height: h, data }
}

/// One view of the paper, ready to register.
pub struct Prepared {
    /// The focuser ring the view was cropped and masked with, in full-frame pixels.
    pub ring_radius: f32,
    /// The mask disc, in full-frame pixels; centred on the ring.
    pub mask_center: Pos2,
    pub mask_radius: f32,
    finest: usize,
    /// Index = level; `None` below `finest`.
    levels: Vec<Option<Level>>,
}

impl Prepared {
    /// Crop `img` around the focuser ring and build `levels` levels; the ones below `finest`
    /// are skipped (Live doesn't need full resolution). `None` if the ring is too small
    /// or mostly outside the frame.
    pub fn new(img: &ColorImage, ring_center: Pos2, ring_radius: f32, levels: usize, finest: usize) -> Option<Self> {
        let [w, h] = img.size;
        let half = CROP_FRAC * ring_radius;
        let x0 = (ring_center.x - half).floor().clamp(0.0, w as f32) as usize;
        let y0 = (ring_center.y - half).floor().clamp(0.0, h as f32) as usize;
        let x1 = ((ring_center.x + half).ceil().clamp(0.0, w as f32) as usize).max(x0);
        let y1 = ((ring_center.y + half).ceil().clamp(0.0, h as f32) as usize).max(y0);
        let unit = 1usize << levels.saturating_sub(1);
        if levels == 0 || x1 - x0 < 8 * unit || y1 - y0 < 8 * unit {
            return None;
        }
        let data = (y0..y1).flat_map(|y| (x0..x1).map(move |x| luma(img.pixels[y * w + x]))).collect();
        let mut raw = GrayImage { width: x1 - x0, height: y1 - y0, data };
        let mask_radius = MASK_FRAC * ring_radius;
        let mut out = Vec::with_capacity(levels);
        for l in 0..levels {
            if l > 0 {
                raw = downsample(&raw);
            }
            if l < finest {
                out.push(None);
                continue;
            }
            let s = (1u32 << l) as f32;
            let off = vec2(x0 as f32, y0 as f32) / s;
            out.push(Some(Level::new(&raw, off, ring_center / s, mask_radius / s)));
        }
        Some(Self { ring_radius, mask_center: ring_center, mask_radius, finest: finest.min(levels - 1), levels: out })
    }

    pub fn levels(&self) -> usize {
        self.levels.len()
    }

    fn level(&self, l: usize) -> &Level {
        self.levels[l].as_ref().expect("pyramid level not prepared")
    }

    /// The mask disc at level `l`, in level coordinates.
    fn mask(&self, l: usize) -> (Pos2, f32) {
        let s = (1u32 << l) as f32;
        (self.mask_center / s, self.mask_radius / s)
    }
}

/// `x' = A x + t` from a reference view to a moving view, in full-frame pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Registration {
    /// A, row-major: [a11, a12, a21, a22].
    pub a: [f32; 4],
    pub t: Vec2,
    /// ZNCC of the two high-passed views after alignment, at the finest level used.
    pub score: f32,
}

impl Registration {
    pub const IDENTITY: Self = Self { a: [1.0, 0.0, 0.0, 1.0], t: Vec2::ZERO, score: 1.0 };

    pub fn map(&self, p: Pos2) -> Pos2 {
        let [a, b, c, d] = self.a;
        pos2(a * p.x + b * p.y + self.t.x, c * p.x + d * p.y + self.t.y)
    }

    /// The rotation part of A, in degrees within 0..360.
    pub fn rotation_deg(&self) -> f32 {
        let [a, b, c, d] = self.a;
        (c - b).atan2(a + d).to_degrees().rem_euclid(360.0)
    }

    /// The point that maps onto itself: `(I − A)⁻¹ t`. `None` when the views are turned
    /// less than about 60° apart, where it is poorly defined.
    pub fn fixed_point(&self) -> Option<Pos2> {
        let [a, b, c, d] = self.a;
        let (m11, m12, m21, m22) = (1.0 - a, -b, -c, 1.0 - d);
        // det(I − R(θ)) = 2 − 2 cos θ, which is 1 at 60° and 4 at 180°.
        let det = m11 * m22 - m12 * m21;
        if det < 1.0 {
            return None;
        }
        let (tx, ty) = (self.t.x, self.t.y);
        Some(pos2((m22 * tx - m12 * ty) / det, (m11 * ty - m21 * tx) / det))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegError {
    /// Several placements match about equally well: the pattern repeats.
    Ambiguous { best: f32, second: f32 },
    /// No usable match: too little texture, or too little overlap.
    NoMatch,
}

/// Where the Gauss–Newton refinement starts.
#[derive(Clone, Debug)]
pub enum Start {
    /// Brute-force search over these rotations (degrees, about the reference's mask
    /// centre) and shifts of up to `range_px` (full-frame pixels) at the coarsest level.
    Search { angles_deg: Vec<f32>, range_px: f32 },
    /// From an earlier result, e.g. the previous live frame.
    From(Registration),
}

impl Start {
    /// For the 0° → 180° match, turned by hand: 150–210° in 2° steps, with the fixed
    /// point within a quarter of the ring radius of the ring centre.
    pub fn half_turn(ring_radius: f32) -> Self {
        Start::Search { angles_deg: (0..=30).map(|i| 150.0 + 2.0 * i as f32).collect(), range_px: 0.5 * ring_radius }
    }
}

/// Register `moving` against `reference`, refining down to level `finest`.
pub fn register(
    reference: &Prepared,
    moving: &Prepared,
    start: &Start,
    finest: usize,
) -> Result<Registration, RegError> {
    let coarsest = reference.levels().min(moving.levels()).saturating_sub(1);
    let finest = finest.max(reference.finest).max(moving.finest).min(coarsest);
    let init = match start {
        Start::From(r) => *r,
        Start::Search { angles_deg, range_px } => {
            let s = (1u32 << coarsest) as f32;
            let range = (range_px / s).ceil().max(1.0) as i32;
            let (angle, d) = coarse_search(reference, moving, coarsest, angles_deg, range)?;
            let (sn, cs) = angle.to_radians().sin_cos();
            let a = [cs, -sn, sn, cs];
            // W(x) = A (x − c) + c + s·d about the reference's mask centre c.
            let c = reference.mask_center;
            let ac = vec2(a[0] * c.x + a[1] * c.y, a[2] * c.x + a[3] * c.y);
            Registration { a, t: c.to_vec2() - ac + d * s, score: 0.0 }
        }
    };
    refine(reference, moving, init, coarsest, finest).ok_or(RegError::NoMatch)
}

/// Follow the paper from `reference` to `moving` (Live), starting at `seed`, the previous
/// result. If that match scores below `min_score`, e.g. after a quick turn of a screw,
/// search again for a shift of up to 30 % of the ring radius.
pub fn track(
    reference: &Prepared,
    moving: &Prepared,
    seed: Registration,
    finest: usize,
    min_score: f32,
) -> Result<Registration, RegError> {
    if let Ok(r) = register(reference, moving, &Start::From(seed), finest)
        && r.score >= min_score
    {
        return Ok(r);
    }
    let search = Start::Search { angles_deg: vec![0.0], range_px: 0.3 * reference.ring_radius };
    register(reference, moving, &search, finest)
}

/// Best (angle, shift in level pixels) at level `l` by brute-force ZNCC.
fn coarse_search(
    reference: &Prepared,
    moving: &Prepared,
    l: usize,
    angles_deg: &[f32],
    range: i32,
) -> Result<(f32, Vec2), RegError> {
    let (rl, ml) = (reference.level(l), moving.level(l));
    let (rc, rr) = reference.mask(l);
    let (mc, mr) = moving.mask(l);
    let mut px: Vec<(i32, i32, f32)> = Vec::new();
    for j in 0..ml.h {
        for i in 0..ml.w {
            if (ml.at(i, j) - mc).length() <= mr {
                px.push((i as i32, j as i32, ml.img[j * ml.w + i]));
            }
        }
    }
    if px.len() < 50 {
        return Err(RegError::NoMatch);
    }
    let imin = px.iter().map(|p| p.0).min().unwrap_or(0) - range;
    let jmin = px.iter().map(|p| p.1).min().unwrap_or(0) - range;
    let ew = (px.iter().map(|p| p.0).max().unwrap_or(0) + range - imin + 1) as usize;
    let eh = (px.iter().map(|p| p.1).max().unwrap_or(0) + range - jmin + 1) as usize;
    let n = (2 * range + 1) as usize;
    let mut scores = vec![f32::NEG_INFINITY; n * n];
    let mut best_angle = vec![0.0f32; n * n];
    // The reference turned by each angle about its mask centre, on the moving lattice.
    let mut turned = vec![f32::NAN; ew * eh];
    for &angle in angles_deg {
        let (sn, cs) = angle.to_radians().sin_cos();
        for jj in 0..eh {
            for ii in 0..ew {
                let y = pos2(ml.off.x + (ii as i32 + imin) as f32 + 0.5, ml.off.y + (jj as i32 + jmin) as f32 + 0.5);
                let v = y - rc;
                let q = rc + vec2(cs * v.x + sn * v.y, -sn * v.x + cs * v.y);
                turned[jj * ew + ii] = if (q - rc).length() <= rr { rl.value(q).unwrap_or(f32::NAN) } else { f32::NAN };
            }
        }
        for dy in -range..=range {
            for dx in -range..=range {
                let (mut cnt, mut sa, mut sb, mut saa, mut sbb, mut sab) = (0u32, 0.0f32, 0.0, 0.0, 0.0, 0.0);
                for &(i, j, a) in &px {
                    let b = turned[(j - dy - jmin) as usize * ew + (i - dx - imin) as usize];
                    if b.is_nan() {
                        continue;
                    }
                    cnt += 1;
                    sa += a;
                    sb += b;
                    saa += a * a;
                    sbb += b * b;
                    sab += a * b;
                }
                if (cnt as usize) < px.len() / 2 {
                    continue;
                }
                let c = cnt as f32;
                let var = (saa - sa * sa / c) * (sbb - sb * sb / c);
                if var <= 0.0 {
                    continue;
                }
                let z = (sab - sa * sb / c) / var.sqrt();
                let k = (dy + range) as usize * n + (dx + range) as usize;
                if z > scores[k] {
                    scores[k] = z;
                    best_angle[k] = angle;
                }
            }
        }
    }
    // Local maxima over a 5 × 5 neighbourhood, best first.
    let mut peaks: Vec<(f32, usize)> = Vec::new();
    for k in 0..n * n {
        let s = scores[k];
        if !s.is_finite() {
            continue;
        }
        let (kx, ky) = ((k % n) as i32, (k / n) as i32);
        let is_max = (-2..=2).all(|oy| {
            (-2..=2).all(|ox| {
                let (x, y) = (kx + ox, ky + oy);
                x < 0 || y < 0 || x >= n as i32 || y >= n as i32 || scores[y as usize * n + x as usize] <= s
            })
        });
        if is_max {
            peaks.push((s, k));
        }
    }
    peaks.sort_by(|a, b| b.0.total_cmp(&a.0));
    let Some(&(best, kb)) = peaks.first() else { return Err(RegError::NoMatch) };
    if best < 0.1 {
        return Err(RegError::NoMatch);
    }
    let shift = |k: usize| vec2((k % n) as f32 - range as f32, (k / n) as f32 - range as f32);
    let second = peaks.iter().skip(1).find(|p| (shift(p.1) - shift(kb)).length() >= PEAK_SEPARATION);
    if let Some(&(second, _)) = second
        && second >= AMBIGUOUS_RATIO * best
    {
        return Err(RegError::Ambiguous { best, second });
    }
    Ok((best_angle[kb], shift(kb)))
}

/// Gauss–Newton on the six affine parameters with Huber weights, from level `coarsest`
/// down to `finest`. `None` if the views stop overlapping or the system is singular.
fn refine(
    reference: &Prepared,
    moving: &Prepared,
    init: Registration,
    coarsest: usize,
    finest: usize,
) -> Option<Registration> {
    let mut a = init.a.map(f64::from);
    let mut t = [init.t.x as f64, init.t.y as f64];
    let mut score = 0.0;
    for l in (finest..=coarsest).rev() {
        let s = (1u32 << l) as f64;
        let (rl, ml) = (reference.level(l), moving.level(l));
        let (rc, rr) = reference.mask(l);
        let (mc, mr) = moving.mask(l);
        let (rcx, rcy) = (rc.x as f64, rc.y as f64);
        let mut refs: Vec<(f64, f64, f32)> = Vec::new();
        for j in 0..rl.h {
            for i in 0..rl.w {
                let g = rl.at(i, j);
                if (g - rc).length() <= rr {
                    refs.push(((g.x - rc.x) as f64, (g.y - rc.y) as f64, rl.img[j * rl.w + i]));
                }
            }
        }
        if refs.len() < 50 {
            return None;
        }
        // About the mask centre: W(g) = A (g − rc) + tau, with tau = A rc + t / s.
        let mut p = [a[0], a[1], a[2], a[3], a[0] * rcx + a[1] * rcy + t[0] / s, a[2] * rcx + a[3] * rcy + t[1] / s];
        let warp = |p: &[f64; 6], ux: f64, uy: f64| {
            pos2((p[0] * ux + p[1] * uy + p[4]) as f32, (p[2] * ux + p[3] * uy + p[5]) as f32)
        };
        let mut huber = f64::INFINITY;
        let mut abs_e: Vec<f32> = Vec::with_capacity(refs.len());
        for _ in 0..if l == finest { 15 } else { 25 } {
            let mut hm = [[0.0f64; 6]; 6];
            let mut bv = [0.0f64; 6];
            abs_e.clear();
            for &(ux, uy, rv) in &refs {
                let w = warp(&p, ux, uy);
                if (w - mc).length() > mr {
                    continue;
                }
                let Some([mv, gx, gy]) = ml.sample(w) else { continue };
                let e = (mv - rv) as f64;
                abs_e.push(e.abs() as f32);
                let wt = if e.abs() <= huber { 1.0 } else { huber / e.abs() };
                let (gx, gy) = (gx as f64, gy as f64);
                let jac = [gx * ux, gx * uy, gy * ux, gy * uy, gx, gy];
                for r in 0..6 {
                    bv[r] -= wt * jac[r] * e;
                    for c in r..6 {
                        hm[r][c] += wt * jac[r] * jac[c];
                    }
                }
            }
            if abs_e.len() * 10 < refs.len() * 3 {
                return None;
            }
            // Only the upper triangle was accumulated.
            let hm = std::array::from_fn(|r| std::array::from_fn(|c| hm[r.min(c)][r.max(c)]));
            let delta = solve(hm, bv)?;
            for (v, d) in p.iter_mut().zip(delta) {
                *v += d;
            }
            let mid = abs_e.len() / 2;
            let (_, median, _) = abs_e.select_nth_unstable_by(mid, f32::total_cmp);
            huber = (1.345 * 1.4826 * *median as f64).max(1e-3);
            let step = delta[4].hypot(delta[5]) + rr as f64 * delta[..4].iter().fold(0.0f64, |m, d| m.max(d.abs()));
            if step < 1e-3 {
                break;
            }
        }
        if l == finest {
            score = zncc(refs.iter().filter_map(|&(ux, uy, rv)| {
                let w = warp(&p, ux, uy);
                ((w - mc).length() <= mr).then(|| ml.value(w).map(|mv| (rv, mv))).flatten()
            }));
        }
        a = [p[0], p[1], p[2], p[3]];
        t = [s * (p[4] - (a[0] * rcx + a[1] * rcy)), s * (p[5] - (a[2] * rcx + a[3] * rcy))];
    }
    Some(Registration { a: a.map(|v| v as f32), t: vec2(t[0] as f32, t[1] as f32), score })
}

fn zncc(pairs: impl Iterator<Item = (f32, f32)>) -> f32 {
    let (mut n, mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0f64, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (a, b) in pairs {
        let (a, b) = (a as f64, b as f64);
        n += 1.0;
        sa += a;
        sb += b;
        saa += a * a;
        sbb += b * b;
        sab += a * b;
    }
    if n < 2.0 {
        return 0.0;
    }
    let var = (saa - sa * sa / n) * (sbb - sb * sb / n);
    if var <= 0.0 { 0.0 } else { ((sab - sa * sb / n) / var.sqrt()) as f32 }
}

/// Gaussian elimination with partial pivoting.
fn solve<const N: usize>(mut m: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|&x, &y| m[x][col].abs().total_cmp(&m[y][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        b.swap(col, pivot);
        let pivot_row = m[col];
        for row in col + 1..N {
            let f = m[row][col] / pivot_row[col];
            for (v, p) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *v -= f * p;
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let s: f64 = (row + 1..N).map(|k| m[row][k] * x[k]).sum();
        x[row] = (b[row] - s) / m[row][row];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 640;
    const H: usize = 480;
    const RIM_C: Pos2 = pos2(320.0, 240.0);
    const RIM_R: f32 = 200.0;

    /// A view of a texture (0..1) inside a dark rim. 4 × 4 jittered samples per pixel:
    /// a fixed sample grid would snap every edge to the same sub-pixel steps.
    fn view(tex: impl Fn(Pos2) -> f32) -> ColorImage {
        let mut rng = crate::detect::Rng::new(3);
        let mut pixels = Vec::with_capacity(W * H);
        for y in 0..H {
            for x in 0..W {
                let mut v = 0.0;
                for k in 0..16 {
                    let jx = (k % 4) as f32 + 0.5 + 0.5 * rng.signed();
                    let jy = (k / 4) as f32 + 0.5 + 0.5 * rng.signed();
                    let p = pos2(x as f32 + jx / 4.0, y as f32 + jy / 4.0);
                    v += if (p - RIM_C).length() <= RIM_R { 40.0 + 160.0 * tex(p) } else { 20.0 };
                }
                pixels.push(Color32::from_gray((v / 16.0).round() as u8));
            }
        }
        ColorImage::new([W, H], pixels)
    }

    /// The printable target at 5 px/mm, turned by `deg` about `c`.
    fn target_view(c: Pos2, deg: f32) -> ColorImage {
        let (sn, cs) = (-deg).to_radians().sin_cos();
        view(|p| {
            let v = p - c;
            let q = vec2(cs * v.x - sn * v.y, sn * v.x + cs * v.y) / 5.0;
            crate::target::value(105.0 + q.x, 148.5 + q.y)
        })
    }

    fn prepare(img: &ColorImage) -> Prepared {
        Prepared::new(img, RIM_C, RIM_R, level_count(RIM_R), 0).unwrap()
    }

    #[test]
    fn finds_the_fixed_point_of_a_hand_turned_half_turn() {
        let c = pos2(327.3, 235.6);
        let r0 = prepare(&target_view(c, 0.0));
        for deg in [177.0, 183.5] {
            let reg = register(&r0, &prepare(&target_view(c, deg)), &Start::half_turn(RIM_R), 0).unwrap();
            let fp = reg.fixed_point().unwrap();
            assert!((fp - c).length() < 0.1, "{deg}°: {:.4} {:.4} vs {c:?}", fp.x, fp.y);
            assert!((reg.rotation_deg() - deg).abs() < 0.05, "{} vs {deg}", reg.rotation_deg());
            assert!(reg.score > 0.9, "score {}", reg.score);
        }
    }

    #[test]
    fn refuses_a_repeating_pattern() {
        let checker = |p: Pos2| ((p.x / 30.0).floor() + (p.y / 30.0).floor()).rem_euclid(2.0);
        let c = pos2(318.0, 244.0);
        let turned = |deg: f32| {
            let (sn, cs) = (-deg).to_radians().sin_cos();
            view(move |p| {
                let v = p - c;
                checker(c + vec2(cs * v.x - sn * v.y, sn * v.x + cs * v.y))
            })
        };
        let r = register(&prepare(&turned(0.0)), &prepare(&turned(180.0)), &Start::half_turn(RIM_R), 0);
        assert!(matches!(r, Err(RegError::Ambiguous { .. })), "{r:?}");
    }

    #[test]
    fn tracks_a_small_shift_from_the_identity() {
        let c = pos2(320.0, 240.0);
        let shift = vec2(3.4, -2.1);
        let r0 = prepare(&target_view(c, 0.0));
        let r1 = prepare(&target_view(c + shift, 0.0));
        let reg = register(&r0, &r1, &Start::From(Registration::IDENTITY), 0).unwrap();
        // Checked inside the mask: `t` alone is the map at the image corner, far outside it.
        for p in [c, c + vec2(150.0, 0.0), c + vec2(0.0, -150.0)] {
            let e = reg.map(p) - (p + shift);
            assert!(e.length() < 0.05, "at {p:?}: off by {:.4} {:.4}, a {:?}", e.x, e.y, reg.a);
        }
    }
}
