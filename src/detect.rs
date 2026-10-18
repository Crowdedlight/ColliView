//! Circle detection for the focuser drawtube rim (and other ring-shaped features).
//!
//! 1. Cast rays from the current centre and, on each ray, take the strongest radial
//!    brightness edge inside the allowed radius band (sub-pixel via parabola fit).
//! 2. Keep only edges of the dominant polarity (dark→bright or bright→dark).
//! 3. RANSAC a circle through the edge points, then refine with a Kåsa algebraic fit
//!    followed by a Gauss–Newton geometric fit on the inliers.
//! 4. Re-centre the rays on the result and repeat with a narrower band.

use egui::{ColorImage, Pos2, Vec2, pos2};

/// Single-channel image with values in 0..=255.
#[derive(Clone, Debug)]
pub struct GrayImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl GrayImage {
    pub fn from_color_image(img: &ColorImage) -> Self {
        let data =
            img.pixels.iter().map(|c| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32).collect();
        Self { width: img.size[0], height: img.size[1], data }
    }

    /// Separable Gaussian blur (σ in pixels), clamped at the borders.
    pub fn blurred(&self, sigma: f32) -> Self {
        if sigma <= 0.0 {
            return self.clone();
        }
        let radius = (sigma * 2.5).ceil() as usize;
        let r = radius as isize;
        let kernel: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
        let norm: f32 = kernel.iter().sum();
        let kernel: Vec<f32> = kernel.iter().map(|k| k / norm).collect();
        let (w, h) = (self.width, self.height);
        let clamped = |i: usize, t: usize, n: usize| (i as isize + t as isize - r).clamp(0, n as isize - 1) as usize;
        let mut tmp = vec![0.0; self.data.len()];
        for (row, dst) in self.data.chunks_exact(w).zip(tmp.chunks_exact_mut(w)) {
            for (x, d) in dst.iter_mut().enumerate() {
                let mut acc = 0.0;
                if x >= radius && x + radius < w {
                    for (k, v) in kernel.iter().zip(&row[x - radius..=x + radius]) {
                        acc += k * v;
                    }
                } else {
                    for (t, k) in kernel.iter().enumerate() {
                        acc += k * row[clamped(x, t, w)];
                    }
                }
                *d = acc;
            }
        }
        // Whole rows at a time, so the column pass stays cache-friendly. The taps are
        // summed in the same order as the row pass.
        let mut out = vec![0.0; self.data.len()];
        for (y, dst) in out.chunks_exact_mut(w).enumerate() {
            for (t, k) in kernel.iter().enumerate() {
                let yi = clamped(y, t, h);
                for (d, s) in dst.iter_mut().zip(&tmp[yi * w..(yi + 1) * w]) {
                    *d += k * s;
                }
            }
        }
        Self { width: self.width, height: self.height, data: out }
    }

    /// Bilinear sample at a pixel-centre coordinate system where pixel (i, j) covers
    /// [i, i+1) × [j, j+1). Returns `None` outside the image.
    pub fn sample(&self, p: Pos2) -> Option<f32> {
        let x = p.x - 0.5;
        let y = p.y - 0.5;
        if x < 0.0 || y < 0.0 || x > (self.width - 1) as f32 || y > (self.height - 1) as f32 {
            return None;
        }
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |xx: usize, yy: usize| self.data[yy * self.width + xx];
        let top = at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx;
        let bottom = at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx;
        Some(top * (1.0 - fy) + bottom * fy)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DetectParams {
    /// Starting centre (raw px).
    pub center: Pos2,
    pub radius_min: f32,
    pub radius_max: f32,
    pub rays: usize,
    /// Pre-blur σ in pixels; suppresses sensor noise and fine texture.
    pub blur_sigma: f32,
}

impl DetectParams {
    /// Search around an expected radius with a relative tolerance (e.g. 0.25 = ±25%).
    pub fn around(center: Pos2, radius: f32, tolerance: f32) -> Self {
        Self {
            center,
            radius_min: (radius * (1.0 - tolerance)).max(2.0),
            radius_max: radius * (1.0 + tolerance),
            rays: 360,
            blur_sigma: 1.2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    pub center: Pos2,
    pub radius: f32,
    /// Fraction of rays whose edge agreed with the final circle.
    pub inlier_ratio: f32,
    /// RMS distance of the inlier edges from the circle (px).
    pub rms: f32,
}

impl Detection {
    /// Rough quality label for the UI.
    pub fn quality(&self) -> &'static str {
        match (self.inlier_ratio, self.rms) {
            (r, e) if r > 0.7 && e < 1.0 => "good",
            (r, e) if r > 0.45 && e < 2.0 => "fair",
            _ => "poor",
        }
    }
}

pub fn detect_circle(img: &GrayImage, params: &DetectParams) -> Option<Detection> {
    let img = img.blurred(params.blur_sigma);
    let mut center = params.center;
    let (mut rmin, mut rmax) = (params.radius_min, params.radius_max);
    let mut best: Option<Detection> = None;
    for pass in 0..3 {
        let edges = radial_edges(&img, center, rmin, rmax, params.rays);
        if edges.len() < 8 {
            break;
        }
        let tol = if pass == 0 { 2.0 } else { 1.2 };
        let (fit, inliers) = robust_circle_fit(&edges, tol, (params.radius_min * 0.8, params.radius_max * 1.2))?;
        let rms = rms_residual(&inliers, &fit);
        let det =
            Detection { center: fit.0, radius: fit.1, inlier_ratio: inliers.len() as f32 / params.rays as f32, rms };
        best = Some(det);
        // Narrow the band around the current fit and re-centre the rays: rays that
        // cross the edge perpendicularly give the most accurate edge positions.
        center = det.center;
        let band = (det.radius * 0.06).max(4.0);
        rmin = (det.radius - band).max(2.0);
        rmax = det.radius + band;
    }
    best
}

/// One edge point per ray: the strongest radial gradient of the dominant polarity.
fn radial_edges(img: &GrayImage, center: Pos2, rmin: f32, rmax: f32, rays: usize) -> Vec<Pos2> {
    const STEP: f32 = 0.5;
    let n = (((rmax - rmin) / STEP).ceil() as usize).max(3) + 1;
    // Strongest positive and negative edge per ray: (radius, strength).
    let mut pos_peaks = Vec::with_capacity(rays);
    let mut neg_peaks = Vec::with_capacity(rays);
    let mut profile = vec![0.0f32; n];
    for k in 0..rays {
        let dir = Vec2::angled(k as f32 / rays as f32 * std::f32::consts::TAU);
        let normal = dir.rot90();
        let mut ok = true;
        for (i, v) in profile.iter_mut().enumerate() {
            let p = center + dir * (rmin + i as f32 * STEP);
            // Average across the ray to suppress noise along the edge.
            match (img.sample(p - normal), img.sample(p), img.sample(p + normal)) {
                (Some(a), Some(b), Some(c)) => *v = 0.25 * a + 0.5 * b + 0.25 * c,
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            pos_peaks.push(None);
            neg_peaks.push(None);
            continue;
        }
        // Central difference over ±1 px.
        let grad: Vec<f32> = (0..n)
            .map(|i| {
                let a = profile[i.saturating_sub(2)];
                let b = profile[(i + 2).min(n - 1)];
                b - a
            })
            .collect();
        pos_peaks.push(peak(&grad, 1.0).map(|(i, s)| (rmin + i * STEP, s)));
        neg_peaks.push(peak(&grad, -1.0).map(|(i, s)| (rmin + i * STEP, s)));
    }
    let strength = |v: &[Option<(f32, f32)>]| v.iter().flatten().map(|p| p.1).sum::<f32>();
    let chosen = if strength(&pos_peaks) >= strength(&neg_peaks) { pos_peaks } else { neg_peaks };
    // Reject very weak edges (flat regions) relative to the median strength.
    let mut strengths: Vec<f32> = chosen.iter().flatten().map(|p| p.1).collect();
    if strengths.is_empty() {
        return Vec::new();
    }
    strengths.sort_by(f32::total_cmp);
    let median = strengths[strengths.len() / 2];
    chosen
        .iter()
        .enumerate()
        .filter_map(|(k, p)| {
            let (r, s) = (*p)?;
            (s >= 0.25 * median && s > 1.0)
                .then(|| center + Vec2::angled(k as f32 / rays as f32 * std::f32::consts::TAU) * r)
        })
        .collect()
}

/// Index (sub-sample, via parabola through the peak) and strength of the largest
/// `sign * grad` value.
fn peak(grad: &[f32], sign: f32) -> Option<(f32, f32)> {
    let (i, &g) = grad.iter().enumerate().max_by(|a, b| (sign * a.1).total_cmp(&(sign * b.1)))?;
    let g = sign * g;
    if g <= 0.0 {
        return None;
    }
    let mut pos = i as f32;
    if i > 0 && i + 1 < grad.len() {
        let (a, b, c) = (sign * grad[i - 1], g, sign * grad[i + 1]);
        let denom = a - 2.0 * b + c;
        if denom.abs() > 1e-6 {
            pos += (0.5 * (a - c) / denom).clamp(-0.5, 0.5);
        }
    }
    Some((pos, g))
}

type Circle = (Pos2, f32);

/// RANSAC + least-squares refinement. Returns the circle and its inliers.
pub fn robust_circle_fit(points: &[Pos2], tol: f32, radius_range: (f32, f32)) -> Option<(Circle, Vec<Pos2>)> {
    if points.len() < 3 {
        return None;
    }
    let mut rng = Rng::new(0x5eed_c011_1a7e);
    let mut best: Option<(Circle, usize)> = None;
    let iterations = 400;
    for _ in 0..iterations {
        let a = points[rng.below(points.len())];
        let b = points[rng.below(points.len())];
        let c = points[rng.below(points.len())];
        let Some(circle) = circumcircle(a, b, c) else { continue };
        if circle.1 < radius_range.0 || circle.1 > radius_range.1 {
            continue;
        }
        let count = points.iter().filter(|p| ((**p - circle.0).length() - circle.1).abs() < tol).count();
        if best.is_none_or(|(_, n)| count > n) {
            best = Some((circle, count));
        }
    }
    let (mut circle, _) = best?;
    let mut inliers = Vec::new();
    for _ in 0..3 {
        inliers = points.iter().copied().filter(|p| ((*p - circle.0).length() - circle.1).abs() < tol).collect();
        if inliers.len() < 3 {
            return None;
        }
        circle = geometric_fit(&inliers, kasa_fit(&inliers)?);
    }
    Some((circle, inliers))
}

fn circumcircle(a: Pos2, b: Pos2, c: Pos2) -> Option<Circle> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-3 {
        return None;
    }
    let (a2, b2, c2) = (a.to_vec2().length_sq(), b.to_vec2().length_sq(), c.to_vec2().length_sq());
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = pos2(ux, uy);
    Some((center, (a - center).length()))
}

/// Algebraic least-squares circle (Kåsa), computed on centred data for stability.
pub fn kasa_fit(points: &[Pos2]) -> Option<Circle> {
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.x as f64).sum::<f64>() / n;
    let my = points.iter().map(|p| p.y as f64).sum::<f64>() / n;
    // Solve [Sxx Sxy Sx; Sxy Syy Sy; Sx Sy n] [D E F]ᵀ = -[Sxz Syz Sz], z = x²+y².
    let mut m = [[0.0f64; 4]; 3];
    for p in points {
        let (x, y) = (p.x as f64 - mx, p.y as f64 - my);
        let z = x * x + y * y;
        let row = [x, y, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += row[i] * row[j];
            }
            m[i][3] -= row[i] * z;
        }
    }
    let [d, e, f] = solve3(m)?;
    let cx = -d / 2.0;
    let cy = -e / 2.0;
    let r2 = cx * cx + cy * cy - f;
    (r2 > 0.0).then(|| (pos2((cx + mx) as f32, (cy + my) as f32), r2.sqrt() as f32))
}

/// Gauss–Newton minimisation of the geometric distance Σ(|p − c| − r)².
pub fn geometric_fit(points: &[Pos2], init: Circle) -> Circle {
    let (mut a, mut b, mut r) = (init.0.x as f64, init.0.y as f64, init.1 as f64);
    for _ in 0..20 {
        let mut m = [[0.0f64; 4]; 3];
        for p in points {
            let (dx, dy) = (p.x as f64 - a, p.y as f64 - b);
            let d = (dx * dx + dy * dy).sqrt().max(1e-9);
            let res = d - r;
            let j = [-dx / d, -dy / d, -1.0];
            for i in 0..3 {
                for k in 0..3 {
                    m[i][k] += j[i] * j[k];
                }
                m[i][3] -= j[i] * res;
            }
        }
        let Some([da, db, dr]) = solve3(m) else { break };
        a += da;
        b += db;
        r += dr;
        if da.abs() + db.abs() + dr.abs() < 1e-6 {
            break;
        }
    }
    (pos2(a as f32, b as f32), r as f32)
}

fn rms_residual(points: &[Pos2], c: &Circle) -> f32 {
    let s: f32 = points.iter().map(|p| ((*p - c.0).length() - c.1).powi(2)).sum();
    (s / points.len().max(1) as f32).sqrt()
}

/// Gaussian elimination with partial pivoting on an augmented 3×4 matrix.
fn solve3(mut m: [[f64; 4]; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let pivot = (col..3).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        for row in 0..3 {
            if row != col {
                let pivot_row = m[col];
                let f = m[row][col] / pivot_row[col];
                for (v, p) in m[row].iter_mut().zip(pivot_row).skip(col) {
                    *v -= f * p;
                }
            }
        }
    }
    Some([m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]])
}

/// Small deterministic xorshift RNG (RANSAC sampling, synthetic noise).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Uniform in [-1, 1).
    pub fn signed(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anti-aliased rings: a bright disc of radius `r` plus distractor rings.
    fn synthetic(w: usize, h: usize, c: Pos2, r: f32, noise: f32) -> GrayImage {
        let mut rng = Rng::new(42);
        let mut data = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                let d = (pos2(x as f32 + 0.5, y as f32 + 0.5) - c).length();
                let cover = |rad: f32| (rad - d + 0.5).clamp(0.0, 1.0);
                let mut v = 30.0 + 120.0 * cover(r);
                // Distractors: a dark inner disc and a faint outer ring.
                v -= 60.0 * cover(0.55 * r);
                v += 25.0 * (cover(1.45 * r) - cover(1.4 * r));
                data[y * w + x] = v + noise * rng.signed();
            }
        }
        GrayImage { width: w, height: h, data }
    }

    #[test]
    fn fits_exact_points() {
        let c = pos2(321.7, 210.2);
        let pts: Vec<Pos2> = (0..50).map(|i| c + Vec2::angled(i as f32 * 0.3) * 123.4).collect();
        let k = kasa_fit(&pts).unwrap();
        let g = geometric_fit(&pts, k);
        assert!((g.0 - c).length() < 1e-2 && (g.1 - 123.4).abs() < 1e-2, "{g:?}");
    }

    #[test]
    fn ransac_ignores_outliers() {
        let c = pos2(400.0, 300.0);
        let mut rng = Rng::new(7);
        let mut pts: Vec<Pos2> =
            (0..200).map(|i| c + Vec2::angled(i as f32 * 0.0314) * (150.0 + 0.3 * rng.signed())).collect();
        for _ in 0..60 {
            pts.push(pos2(400.0 + 300.0 * rng.signed(), 300.0 + 300.0 * rng.signed()));
        }
        let ((fc, fr), _) = robust_circle_fit(&pts, 1.5, (50.0, 400.0)).unwrap();
        assert!((fc - c).length() < 0.1 && (fr - 150.0).abs() < 0.1, "{fc:?} {fr}");
    }

    #[test]
    fn detects_ring_with_subpixel_accuracy() {
        let truth = pos2(512.3, 380.7);
        let img = synthetic(1024, 768, truth, 220.0, 6.0);
        // Start 15 px off with a radius guess that is 10% wrong.
        let params = DetectParams::around(truth + Vec2::new(15.0, -9.0), 200.0, 0.25);
        let det = detect_circle(&img, &params).unwrap();
        assert!((det.center - truth).length() < 0.3, "{det:?}");
        assert!((det.radius - 220.0).abs() < 0.3, "{det:?}");
        assert_eq!(det.quality(), "good");
    }

    #[test]
    fn partial_ring_still_detected() {
        // Rim partially outside the frame.
        let truth = pos2(300.0, 200.0);
        let img = synthetic(640, 360, truth, 230.0, 3.0);
        let det = detect_circle(&img, &DetectParams::around(pos2(320.0, 180.0), 230.0, 0.2)).unwrap();
        assert!((det.center - truth).length() < 0.5, "{det:?}");
    }
}
