//! A simulated view down a Newtonian focuser, so the app (and the tests) can run
//! without a camera.
//!
//! The model matches what the rotation calibration assumes: the drawtube rim is on the
//! focuser axis, so it stays put when the adapter rotates; camera-induced offsets
//! (`parallax`) are fixed in the camera frame and grow with feature depth; real
//! misalignments rotate with the adapter.
//!
//! Instead of the mirrors it can show a sheet of paper taped inside the tube, for the
//! paper-target centring. The paper is fixed to the telescope at some depth, so its
//! image turns with the adapter about the axis as seen at that depth.

use std::time::{Duration, Instant};

use egui::{Pos2, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

use super::{Capture, PixelFormat, RgbFrame, VideoMode};
use crate::detect::Rng;
use crate::target;

/// What the simulated camera sees beyond the drawtube.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperKind {
    /// The secondary and the primary.
    #[default]
    Off,
    /// The printable centring target.
    Target,
    /// Plain squared paper: it repeats, so it can't be matched.
    Graph,
    /// Squared paper with a few bold pen marks.
    MarkedGraph,
}

impl PaperKind {
    pub const ALL: [Self; 4] = [Self::Off, Self::Target, Self::Graph, Self::MarkedGraph];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off (mirrors)",
            Self::Target => "Printed target",
            Self::Graph => "Plain squared paper",
            Self::MarkedGraph => "Squared paper + pen marks",
        }
    }
}

// `default` so that sessions saved before a field existed still load.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scene {
    /// Where the focuser axis appears relative to the image centre (px).
    pub axis_offset: [f32; 2],
    /// Camera-induced offset per unit depth (px). The secondary sits at depth 1.0,
    /// primary features at 1.6–1.8.
    pub parallax: [f32; 2],
    /// Real secondary misalignment (px). Rotates with the adapter.
    pub secondary_error: [f32; 2],
    /// Real primary misalignment (px). Rotates with the adapter.
    pub primary_error: [f32; 2],
    /// Adapter rotation in the focuser (degrees).
    pub rotation_deg: f32,
    /// Peak-to-peak sensor noise in grey levels.
    pub noise: f32,
    /// A sheet of paper inside the tube instead of the mirrors.
    pub paper: PaperKind,
    /// The paper's depth, in the units of `parallax` (the secondary sits at 1.0).
    pub paper_depth: f32,
    /// Where the paper sits: the page point on the axis, relative to the page centre (mm).
    pub paper_offset: [f32; 2],
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            axis_offset: [18.0, -11.0],
            parallax: [4.0, 2.5],
            secondary_error: [-6.0, 3.0],
            primary_error: [5.0, 4.0],
            rotation_deg: 0.0,
            noise: 6.0,
            paper: PaperKind::Off,
            paper_depth: 1.0,
            paper_offset: [6.0, -4.0],
        }
    }
}

/// Feature centres and radii of a rendered scene, in raw pixels.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub rim: (Pos2, f32),
    pub secondary: (Pos2, f32),
    pub primary: (Pos2, f32),
    pub secondary_reflection: (Pos2, f32),
    pub light_ring: (Pos2, f32),
    pub center_spot: (Pos2, f32),
    /// The axis at the paper's depth: the paper turns about it.
    pub paper: Pos2,
}

impl Scene {
    pub fn geometry(&self, width: u32, height: u32) -> Geometry {
        let r = 0.4 * width.min(height) as f32;
        let axis = pos2(width as f32 / 2.0, height as f32 / 2.0) + Vec2::from(self.axis_offset);
        let rot = |v: [f32; 2]| rotate(Vec2::from(v), self.rotation_deg);
        let parallax = Vec2::from(self.parallax);
        let sec = axis + parallax + rot(self.secondary_error);
        let prim = axis + parallax * 1.6 + rot(self.secondary_error) + rot(self.primary_error);
        let sec_refl = axis + parallax * 1.6 + rot(self.secondary_error) + rot(self.primary_error) * 2.0;
        Geometry {
            rim: (axis, r),
            secondary: (sec, 0.72 * r),
            primary: (prim, 0.6 * r),
            secondary_reflection: (sec_refl, 0.3 * r),
            light_ring: (sec_refl, 0.22 * r),
            center_spot: (axis + parallax * 1.8 + rot(self.secondary_error) + rot(self.primary_error), 0.035 * r),
            paper: axis + parallax * self.paper_depth,
        }
    }

    /// Render a noise-free RGB8 frame.
    pub fn render(&self, width: u32, height: u32) -> Vec<u8> {
        let g = self.geometry(width, height);
        if self.paper != PaperKind::Off {
            return self.render_paper(width, height, &g);
        }
        let (w, h) = (width as usize, height as usize);
        let rot = self.rotation_deg.to_radians();
        let vane_dirs = [rot + 0.25 * std::f32::consts::PI, rot + 0.75 * std::f32::consts::PI];
        let clip_angles = [90.0_f32, 210.0, 330.0].map(|a| (a + self.rotation_deg).to_radians());
        let mut out = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                // Drawtube wall, slightly lit towards the rim.
                let dr = (p - g.rim.0).length();
                let mut v = 14.0 + 10.0 * (1.0 - ((dr - g.rim.1) / 120.0).clamp(0.0, 1.0));
                v = blend(v, 60.0, disc(p, g.rim));
                v = blend(v, 140.0, disc(p, g.secondary));
                v = blend(v, 190.0, disc(p, g.primary));
                // Mirror clips: dark tabs over the primary's edge.
                for a in clip_angles {
                    let c = g.primary.0 + Vec2::angled(a) * g.primary.1;
                    let local = p - c;
                    let along = local.dot(Vec2::angled(a + std::f32::consts::FRAC_PI_2)).abs();
                    let radial = local.dot(Vec2::angled(a));
                    let cover = (0.06 * g.primary.1 - along + 0.5).clamp(0.0, 1.0)
                        * (radial + 0.12 * g.primary.1 + 0.5).clamp(0.0, 1.0)
                        * disc(p, (g.secondary.0, g.secondary.1));
                    v = blend(v, 45.0, cover);
                }
                // Spider vanes inside the primary reflection.
                for d in vane_dirs {
                    let dist = (p - g.secondary_reflection.0).dot(Vec2::angled(d + std::f32::consts::FRAC_PI_2)).abs();
                    v = blend(v, 30.0, (1.5 - dist + 0.5).clamp(0.0, 1.0) * disc(p, g.primary));
                }
                v = blend(v, 35.0, disc(p, g.secondary_reflection));
                v = blend(v, 225.0, ring(p, g.light_ring, 0.03 * g.rim.1));
                v = blend(v, 240.0, ring(p, g.center_spot, 2.0));
                let i = (y * w + x) * 3;
                out[i] = v as u8;
                out[i + 1] = v as u8;
                out[i + 2] = (v + 6.0).min(255.0) as u8;
            }
        }
        out
    }

    fn render_paper(&self, width: u32, height: u32, g: &Geometry) -> Vec<u8> {
        let (w, h) = (width as usize, height as usize);
        // About 80 mm of paper across the rim, as at the secondary's distance.
        let pixel_mm = 40.0 / g.rim.1;
        let (s, c) = (-self.rotation_deg).to_radians().sin_cos();
        let page = vec2(0.5 * target::PAGE_MM.0, 0.5 * target::PAGE_MM.1) + Vec2::from(self.paper_offset);
        let mut out = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let dr = (p - g.rim.0).length();
                let mut v = 14.0 + 10.0 * (1.0 - ((dr - g.rim.1) / 120.0).clamp(0.0, 1.0));
                let cover = disc(p, g.rim);
                if cover > 0.0 {
                    let d = (p - g.paper) * pixel_mm;
                    let q = page + vec2(c * d.x - s * d.y, s * d.x + c * d.y);
                    v = blend(v, 35.0 + 170.0 * paper_value(self.paper, q, pixel_mm), cover);
                }
                let i = (y * w + x) * 3;
                out[i] = v as u8;
                out[i + 1] = v as u8;
                out[i + 2] = (v + 6.0).min(255.0) as u8;
            }
        }
        out
    }
}

/// Reflectance (0 black … 1 white) at page point `q` (mm), averaged over a pixel `b` mm
/// wide. The average is computed, not sampled, so edges keep their sub-pixel positions.
fn paper_value(kind: PaperKind, q: Vec2, b: f32) -> f32 {
    match kind {
        PaperKind::Off => 1.0,
        PaperKind::Target => {
            let bc = b / target::CELL_MM;
            let u = (q.x - target::ORIGIN_MM.0) / target::CELL_MM;
            let v = (q.y - target::ORIGIN_MM.1) / target::CELL_MM;
            let mut white = 0.0;
            for (col, wx) in box_weights(u, bc) {
                for (row, wy) in box_weights(v, bc) {
                    if !target::is_black(col, row) {
                        white += wx * wy;
                    }
                }
            }
            white
        }
        PaperKind::Graph => graph_value(q, b),
        PaperKind::MarkedGraph => graph_value(q, b) * (1.0 - marks(q, b)),
    }
}

/// How a box `b` cells wide centred at `u` splits between the cell its left edge is in
/// and the next one.
fn box_weights(u: f32, b: f32) -> [(i32, f32); 2] {
    let i = (u - 0.5 * b).floor();
    let next = ((u + 0.5 * b - (i + 1.0)) / b).clamp(0.0, 1.0);
    [(i as i32, 1.0 - next), (i as i32 + 1, next)]
}

/// Squared paper: 0.3 mm grey lines every 5 mm.
fn graph_value(q: Vec2, b: f32) -> f32 {
    const PITCH: f32 = 5.0;
    const LINE: f32 = 0.3;
    let cover = |x: f32| {
        let d = (x - PITCH * (x / PITCH).round()).abs();
        let overlap = (d + 0.5 * b).min(0.5 * LINE) - (d - 0.5 * b).max(-0.5 * LINE);
        (overlap / b).clamp(0.0, 1.0)
    };
    let ink = 1.0 - (1.0 - cover(q.x)) * (1.0 - cover(q.y));
    1.0 - 0.55 * ink
}

/// Bold felt-pen strokes near the page centre (mm): start, end, width.
const MARKS: [([f32; 2], [f32; 2], f32); 8] = [
    ([82.0, 122.0], [97.0, 130.0], 3.0),
    ([121.0, 116.0], [118.0, 138.0], 3.5),
    ([99.0, 150.0], [128.0, 157.0], 2.5),
    ([74.0, 158.0], [80.0, 181.0], 3.0),
    ([110.0, 176.0], [133.0, 168.0], 4.0),
    ([93.0, 139.0], [94.0, 140.0], 5.0),
    ([126.0, 141.0], [139.0, 151.0], 2.5),
    ([86.0, 103.0], [104.0, 108.0], 3.0),
];

/// Ink coverage of the pen marks.
fn marks(q: Vec2, b: f32) -> f32 {
    MARKS
        .iter()
        .map(|&(a, e, width)| {
            let (a, e) = (Vec2::from(a), Vec2::from(e));
            let t = ((q - a).dot(e - a) / (e - a).length_sq().max(1e-6)).clamp(0.0, 1.0);
            let dist = (q - (a + (e - a) * t)).length();
            ((0.5 * width - dist) / b + 0.5).clamp(0.0, 1.0)
        })
        .fold(0.0, f32::max)
}

fn rotate(v: Vec2, deg: f32) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    vec2(c * v.x - s * v.y, s * v.x + c * v.y)
}

fn blend(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Anti-aliased coverage of a filled disc.
fn disc(p: Pos2, (c, r): (Pos2, f32)) -> f32 {
    (r - (p - c).length() + 0.5).clamp(0.0, 1.0)
}

/// Anti-aliased coverage of a ring of the given width centred on radius `r`.
fn ring(p: Pos2, (c, r): (Pos2, f32), width: f32) -> f32 {
    let d = ((p - c).length() - r).abs();
    (width / 2.0 - d + 0.5).clamp(0.0, 1.0)
}

const MODES: [(u32, u32); 3] = [(1280, 960), (640, 480), (1920, 1080)];

pub struct TestPatternCapture {
    scene: Scene,
    mode: VideoMode,
    base: Vec<u8>,
    rng: Rng,
    next_due: Instant,
}

impl TestPatternCapture {
    pub fn new(scene: Scene, requested: Option<VideoMode>) -> Self {
        let modes = Self::all_modes();
        let mode = requested.filter(|m| modes.contains(m)).unwrap_or(modes[0]);
        let base = scene.render(mode.width, mode.height);
        Self { scene, mode, base, rng: Rng::new(1), next_due: Instant::now() }
    }

    fn all_modes() -> Vec<VideoMode> {
        MODES
            .iter()
            .map(|&(width, height)| VideoMode { width, height, fps: 30, format: PixelFormat::Synthetic })
            .collect()
    }
}

impl Capture for TestPatternCapture {
    fn mode(&self) -> VideoMode {
        self.mode
    }

    fn modes(&self) -> Vec<VideoMode> {
        Self::all_modes()
    }

    fn next_frame(&mut self) -> anyhow::Result<RgbFrame> {
        // Pace like a real 30 fps camera.
        let now = Instant::now();
        if self.next_due > now {
            std::thread::sleep(self.next_due - now);
        }
        self.next_due = Instant::now().max(self.next_due) + Duration::from_secs_f32(1.0 / self.mode.fps as f32);
        let mut data = self.base.clone();
        if self.scene.noise > 0.0 {
            let amp = self.scene.noise / 2.0;
            for px in data.as_chunks_mut::<3>().0 {
                let n = amp * self.rng.signed();
                for c in px {
                    *c = (*c as f32 + n).clamp(0.0, 255.0) as u8;
                }
            }
        }
        Ok(RgbFrame { width: self.mode.width as usize, height: self.mode.height as usize, data })
    }

    fn set_scene(&mut self, scene: &Scene) {
        if *scene != self.scene {
            self.scene = *scene;
            self.base = scene.render(self.mode.width, self.mode.height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibrate::{COAXIAL_TOLERANCE, RotationSample, live_bias, solve};
    use crate::detect::{DetectParams, GrayImage, detect_circle};
    use crate::overlay::Measurement;

    const W: u32 = 960;
    const H: u32 = 720;

    fn gray(scene: &Scene) -> GrayImage {
        let rgb = scene.render(W, H);
        let mut rng = Rng::new(99);
        let data = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32 + scene.noise / 2.0 * rng.signed())
            .collect();
        GrayImage { width: W as usize, height: H as usize, data }
    }

    fn measure(img: &GrayImage, start: Pos2, radius: f32, tol: f32) -> Measurement {
        let d = detect_circle(img, &DetectParams::around(start, radius, tol)).expect("detection failed");
        Measurement { center: d.center, radius: d.radius }
    }

    #[test]
    fn detects_rim_in_simulated_view() {
        let scene = Scene::default();
        let g = scene.geometry(W, H);
        let img = gray(&scene);
        // Start from the image centre with the default preset radius guess.
        let m = measure(&img, pos2(W as f32 / 2.0, H as f32 / 2.0), 0.42 * H as f32, 0.25);
        assert!((m.center - g.rim.0).length() < 0.5, "{m:?} vs {:?}", g.rim);
        assert!((m.radius - g.rim.1).abs() < 0.5);
    }

    #[test]
    fn rotation_calibration_recovers_bias_and_misalignment() {
        let mut scene = Scene { noise: 4.0, ..Default::default() };
        let mut samples = Vec::new();
        for rot in [0.0, 180.0] {
            scene.rotation_deg = rot;
            let g = scene.geometry(W, H);
            let img = gray(&scene);
            let rim = measure(&img, pos2(W as f32 / 2.0, H as f32 / 2.0), 0.42 * H as f32, 0.25);
            // "Measure" the secondary, searching around the rim centre like the UI does.
            let sec = measure(&img, rim.center, g.secondary.1, 0.12);
            samples.push(RotationSample { rim, target: sec.center });
        }
        let r = solve(&samples[0], &samples[1], COAXIAL_TOLERANCE);
        let expected_bias = Vec2::from(scene.parallax);
        let expected_v = Vec2::from(scene.secondary_error);
        assert!(r.coaxial);
        assert!((r.bias - expected_bias).length() < 0.5, "bias {:?} vs {expected_bias:?}", r.bias);
        assert!((r.misalignment - expected_v).length() < 0.5, "v {:?} vs {expected_v:?}", r.misalignment);
    }

    /// Measure the rim, then the secondary from the rim's result, like the app does.
    fn sample_at(scene: &Scene) -> RotationSample {
        let g = scene.geometry(W, H);
        let img = gray(scene);
        let rim = measure(&img, pos2(W as f32 / 2.0, H as f32 / 2.0), 0.42 * H as f32, 0.25);
        let sec = measure(&img, rim.center, g.secondary.1, 0.12);
        RotationSample { rim, target: sec.center }
    }

    #[test]
    fn live_bias_follows_the_carrier_screws() {
        let mut scene = Scene { noise: 4.0, ..Default::default() };
        let s0 = sample_at(&scene);
        scene.rotation_deg = 180.0;
        let r = solve(&s0, &sample_at(&scene), COAXIAL_TOLERANCE);
        assert!(r.coaxial);
        // Still at 180°, the screws move the camera towards the axis: less parallax, and
        // the whole image shifts a little.
        for (parallax, axis_offset) in [([1.5, -0.5], [14.0, -9.0]), ([0.0, 0.0], [12.0, -8.0])] {
            scene.parallax = parallax;
            scene.axis_offset = axis_offset;
            let live = live_bias(&r, &sample_at(&scene));
            let expected = Vec2::from(parallax);
            assert!((live - expected).length() < 0.5, "live bias {live:?} vs {expected:?}");
        }
    }

    // ------------------------------------------------------------ paper target

    use std::sync::atomic::{AtomicU64, Ordering};

    use PaperKind::{Graph, MarkedGraph, Target};
    use egui::{Color32, ColorImage};

    use crate::calibrate::{PaperError, PaperResult, paper_live_bias, solve_paper};
    use crate::register::{Prepared, Registration, Start, level_count, register, track};

    /// A noisy frame; every call gets fresh noise, like a camera.
    fn color(scene: &Scene) -> ColorImage {
        static SEED: AtomicU64 = AtomicU64::new(1);
        let mut rng = Rng::new(SEED.fetch_add(1, Ordering::Relaxed));
        let amp = scene.noise / 2.0;
        let px = scene
            .render(W, H)
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| {
                let n = amp * rng.signed();
                let f = |c: u8| (c as f32 + n).clamp(0.0, 255.0) as u8;
                Color32::from_rgb(f(p[0]), f(p[1]), f(p[2]))
            })
            .collect();
        ColorImage::new([W as usize, H as usize], px)
    }

    fn sensor_centre() -> Pos2 {
        pos2(W as f32 / 2.0, H as f32 / 2.0)
    }

    /// The focuser ring as a rough hand or auto fit: a few px off, a little too large.
    fn rough_ring(scene: &Scene) -> (Pos2, f32) {
        let g = scene.geometry(W, H);
        (g.rim.0 + vec2(5.0, -4.0), 1.03 * g.rim.1)
    }

    /// Capture at 0° and at `deg`, leaving the scene at `deg`.
    fn paper_calibration(scene: &mut Scene, deg: f32) -> Result<(PaperResult, Prepared), PaperError> {
        let (c, r) = rough_ring(scene);
        let levels = level_count(r);
        scene.rotation_deg = 0.0;
        let v0 = Prepared::new(&color(scene), c, r, levels, 0).unwrap();
        scene.rotation_deg = deg;
        let v1 = Prepared::new(&color(scene), c, r, levels, 0).unwrap();
        let reg = register(&v0, &v1, &Start::half_turn(r), 0)?;
        Ok((solve_paper(&reg, sensor_centre(), r)?, v1))
    }

    /// Auto-detect (D) with the paper already in the tube, from the preset ring.
    #[test]
    fn auto_detect_finds_the_rim_with_the_paper_in() {
        for paper in [Target, MarkedGraph, Graph] {
            let scene = Scene { paper, noise: 4.0, ..Default::default() };
            let g = scene.geometry(W, H);
            let img = GrayImage::from_color_image(&color(&scene));
            let m = measure(&img, pos2(W as f32 / 2.0, H as f32 / 2.0), 0.42 * H as f32, 0.25);
            assert!((m.center - g.rim.0).length() < 0.5, "{paper:?}: {m:?} vs {:?}", g.rim);
            assert!((m.radius - g.rim.1).abs() < 0.5, "{paper:?}: {m:?}");
        }
    }

    #[test]
    fn paper_calibration_recovers_the_bias() {
        // At exactly 180° every pixel lands at the same sub-pixel offset, so put the
        // fixed point off the pixel grid as well.
        let off_grid = [18.3, -11.15];
        for (paper, deg, axis_offset) in [
            (Target, 177.0, Scene::default().axis_offset),
            (Target, 183.0, off_grid),
            (Target, 180.0, off_grid),
            (MarkedGraph, 180.0, off_grid),
        ] {
            let mut scene = Scene { paper, noise: 4.0, axis_offset, ..Default::default() };
            let (r, _) = paper_calibration(&mut scene, deg).unwrap_or_else(|e| panic!("{paper:?} at {deg}°: {e}"));
            // Where the axis meets the paper, relative to the sensor centre.
            let expected = scene.geometry(W, H).paper - sensor_centre();
            assert!((r.bias - expected).length() < 0.3, "{paper:?} at {deg}°: bias {:?} vs {expected:?}", r.bias);
            assert!((r.rotation_deg - deg).abs() < 0.2, "{} vs {deg}", r.rotation_deg);
        }
    }

    /// A frame with lens vignetting fixed to the camera (40 % darker at the ring's edge,
    /// centred on the sensor, not on the paper's axis point) and slightly out of focus.
    fn vignetted(scene: &Scene) -> ColorImage {
        let g = scene.geometry(W, H);
        let img = GrayImage::from_color_image(&color(scene));
        let data = img
            .data
            .iter()
            .enumerate()
            .map(|(k, v)| {
                let p = pos2((k % W as usize) as f32 + 0.5, (k / W as usize) as f32 + 0.5);
                v * (1.0 - 0.4 * ((p - sensor_centre()).length() / g.rim.1).powi(2))
            })
            .collect();
        let soft = GrayImage { data, ..img }.blurred(1.5);
        let px = soft.data.iter().map(|v| Color32::from_gray(v.round().clamp(0.0, 255.0) as u8)).collect();
        ColorImage::new([W as usize, H as usize], px)
    }

    #[test]
    fn paper_calibration_survives_vignetting_and_soft_focus() {
        for paper in [Target, MarkedGraph] {
            let mut scene = Scene { paper, noise: 4.0, axis_offset: [18.3, -11.15], ..Default::default() };
            let (c, r) = rough_ring(&scene);
            let levels = level_count(r);
            let v0 = Prepared::new(&vignetted(&scene), c, r, levels, 0).unwrap();
            scene.rotation_deg = 178.5;
            let v1 = Prepared::new(&vignetted(&scene), c, r, levels, 0).unwrap();
            let reg = register(&v0, &v1, &Start::half_turn(r), 0).unwrap();
            let res = solve_paper(&reg, sensor_centre(), r).unwrap();
            let expected = scene.geometry(W, H).paper - sensor_centre();
            assert!((res.bias - expected).length() < 0.3, "{paper:?}: bias {:?} vs {expected:?}", res.bias);
        }
    }

    /// Live, the mask stays on the ring of the captures while the screws move the camera,
    /// down to centred, including a quick turn of about 15 % of the ring radius at once.
    #[test]
    fn paper_live_bias_follows_the_carrier_screws() {
        let mut scene = Scene { paper: Target, noise: 4.0, ..Default::default() };
        let (r, reference) = paper_calibration(&mut scene, 180.0).unwrap();
        let min_score = 0.9 * r.match_score;
        let mut seed = Registration::IDENTITY;
        for (parallax, axis_offset) in
            [([1.5, -0.5], [14.0, -9.0]), ([-12.0, 30.0], [-20.0, 25.0]), ([0.5, 0.2], [-0.5, -0.2])]
        {
            scene.parallax = parallax;
            scene.axis_offset = axis_offset;
            let live =
                Prepared::new(&color(&scene), reference.mask_center, reference.ring_radius, reference.levels(), 1)
                    .unwrap();
            seed = track(&reference, &live, seed, 1, min_score).unwrap();
            let b = paper_live_bias(&r, &seed);
            let expected = scene.geometry(W, H).paper - sensor_centre();
            assert!((b - expected).length() < 0.3, "live bias {b:?} vs {expected:?} (score {})", seed.score);
        }
    }

    #[test]
    fn plain_squared_paper_is_refused() {
        let mut scene = Scene { paper: Graph, noise: 4.0, ..Default::default() };
        let r = paper_calibration(&mut scene, 180.0).map(|(r, _)| r);
        assert!(matches!(r, Err(PaperError::Repeats)), "{r:?}");
    }

    #[test]
    fn scenes_saved_before_the_paper_fields_still_load() {
        let old = r#"{"axis_offset":[1.0,2.0],"parallax":[0.5,0.0],"secondary_error":[0.0,0.0],
                      "primary_error":[0.0,0.0],"rotation_deg":90.0,"noise":3.0}"#;
        let s: Scene = serde_json::from_str(old).unwrap();
        assert_eq!((s.axis_offset, s.rotation_deg, s.paper), ([1.0, 2.0], 90.0, PaperKind::Off));
    }

    /// Timing at the IMX179's 8 MP: `cargo test --release paper_timing -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn paper_timing_at_8_mp() {
        use std::time::Instant;
        let (w, h) = (3264u32, 2448u32);
        let mut scene = Scene { paper: Target, noise: 4.0, ..Default::default() };
        let frame = |scene: &Scene| ColorImage::from_rgb([w as usize, h as usize], &scene.render(w, h));
        let g = scene.geometry(w, h);
        let levels = level_count(g.rim.1);
        let f0 = frame(&scene);
        scene.rotation_deg = 180.0;
        let f1 = frame(&scene);
        scene.axis_offset = [10.0, -5.0];
        let f2 = frame(&scene);
        let t = Instant::now();
        let v0 = Prepared::new(&f0, g.rim.0, g.rim.1, levels, 0).unwrap();
        let v1 = Prepared::new(&f1, g.rim.0, g.rim.1, levels, 0).unwrap();
        let prep = t.elapsed() / 2;
        let t = Instant::now();
        let reg = register(&v0, &v1, &Start::half_turn(g.rim.1), 0).unwrap();
        let capture = t.elapsed();
        let t = Instant::now();
        let live = Prepared::new(&f2, g.rim.0, g.rim.1, levels, 1).unwrap();
        let live_reg = track(&v1, &live, Registration::IDENTITY, 1, 0.9 * reg.score).unwrap();
        let live_time = t.elapsed();
        eprintln!(
            "ring r={:.0} px, {levels} levels: prepare {prep:?}, 0°→180° match {capture:?} (score {:.3}), \
             live update {live_time:?} (score {:.3})",
            g.rim.1, reg.score, live_reg.score
        );
    }
}
