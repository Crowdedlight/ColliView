//! A simulated view down a Newtonian focuser, so the app (and the tests) can run
//! without a camera.
//!
//! The model: the drawtube rim is on the
//! focuser axis, so it stays put when the adapter rotates; camera-induced offsets
//! (`parallax`) are fixed in the camera frame and grow with feature depth; real
//! misalignments rotate with the adapter.
//!
//! It can also show the view into the centring jig instead (see `crate::jig`).

use std::time::{Duration, Instant};

use egui::{Pos2, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

use super::{Capture, PixelFormat, RgbFrame, VideoMode};
use crate::detect::Rng;
use crate::jig::JigGeometry;

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
    /// Show the view into the centring jig instead of the telescope.
    pub jig: bool,
    /// The lens's offset from the jig's axis (mm).
    pub jig_lens_offset_mm: [f32; 2],
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
            jig: false,
            jig_lens_offset_mm: [0.15, -0.1],
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
        }
    }

    /// Render a noise-free RGB8 frame.
    pub fn render(&self, width: u32, height: u32) -> Vec<u8> {
        let g = self.geometry(width, height);
        if self.jig {
            return self.render_jig(width, height);
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

    /// Where the jig's holes appear: the near aperture (centre, radius) and the far hole.
    /// The jig's axis shows `axis_offset` from the image centre (a camera tilt, or the lens
    /// off-centre over the sensor), and a hole at depth z shows at axis − f·e/z for a lens
    /// offset e: the near one moves more.
    pub fn jig_geometry(&self, width: u32, height: u32) -> ((Pos2, f32), (Pos2, f32)) {
        let jig = JigGeometry::default();
        let r = 0.3 * width.min(height) as f32;
        let axis = pos2(width as f32 / 2.0, height as f32 / 2.0) + Vec2::from(self.axis_offset);
        let fe = Vec2::from(self.jig_lens_offset_mm) * jig.focal_px(r);
        ((axis - fe / jig.near_dist, r), (axis - fe / jig.far_dist, r * jig.far_radius_ratio()))
    }

    fn render_jig(&self, width: u32, height: u32) -> Vec<u8> {
        let (near, far) = self.jig_geometry(width, height);
        // The baffle limits the view, centred on the sensor: to about 14.9° off the axis with
        // the adapter's defaults, against the aperture's atan(r1 / z1).
        let jig = JigGeometry::default();
        let baffle = near.1 * 14.9_f32.to_radians().tan() / (0.5 * jig.near_d / jig.near_dist);
        let view = (pos2(width as f32 / 2.0, height as f32 / 2.0), baffle);
        let (w, h) = (width as usize, height as usize);
        let mut out = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                // The funnel's face around the aperture, lit by the light ring, then the
                // dark far plate with the hole lit from behind.
                let mut v = blend(150.0, 28.0, disc(p, near));
                v = blend(v, 235.0, disc(p, far));
                v = blend(12.0, v, disc(p, view));
                let i = (y * w + x) * 3;
                out[i] = v as u8;
                out[i + 1] = v as u8;
                out[i + 2] = (v + 6.0).min(255.0) as u8;
            }
        }
        out
    }
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
    use crate::detect::{DetectParams, Detection, GrayImage, Polarity, detect_circle};
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

    /// The lens offset reads the same with the axis off the sensor centre, and the axis
    /// point reads that offset.
    #[test]
    fn jig_reads_the_lens_offset() {
        let jig = JigGeometry::default();
        let centre = pos2(W as f32 / 2.0, H as f32 / 2.0);
        for e in [[0.2, -0.1], [0.0, 0.0], [-0.6, 0.4]] {
            let scene = Scene { jig: true, jig_lens_offset_mm: e, axis_offset: [14.0, -9.0], ..Default::default() };
            let ((c, r), _) = scene.jig_geometry(W, H);
            // Out of focus: the aperture is nearer than the lens's closest focus.
            let img = gray(&scene).blurred(2.0);
            // As the live readout does: from scratch, then from a slightly stale last fit.
            let stale = Detection { center: c + vec2(3.0, -2.0), radius: 1.03 * r, inlier_ratio: 1.0, rms: 0.1 };
            for seed in [None, Some(&stale)] {
                let (near, far) = crate::jig::measure(&img, seed, &jig).expect("holes not found");
                let got = jig.lens_offset_mm(&near, &far);
                let tol = if e == [0.0, 0.0] { 0.01 } else { 0.02 };
                assert!((got - Vec2::from(e)).length() < tol, "{got:?} vs {e:?}");
                let axis = jig.axis_point(&near, &far) - centre;
                assert!((axis - vec2(14.0, -9.0)).length() < 0.1, "{axis:?} for {e:?}");
            }
        }
    }

    /// The jig's first search covers most of the frame, so it also spans the baffle's
    /// edge just outside the aperture. That edge is stronger but bright inside, so only
    /// the aperture's dark-inside edge may count.
    #[test]
    fn wide_jig_search_skips_the_baffle_edge() {
        let scene = Scene { jig: true, axis_offset: [14.0, -9.0], ..Default::default() };
        let ((c, r), _) = scene.jig_geometry(W, H);
        let img = gray(&scene).blurred(3.0);
        let wide = DetectParams::wide(vec2(W as f32, H as f32));
        let d = detect_circle(&img, &wide.with_polarity(Polarity::DarkInside)).unwrap();
        assert!((d.center - c).length() < 0.5 && (d.radius - r).abs() < 1.0, "{d:?} vs {c:?} r {r}");
    }

    #[test]
    fn scenes_saved_with_the_paper_target_still_load() {
        let old = r#"{"axis_offset":[1.0,2.0],"parallax":[0.5,0.0],"secondary_error":[0.0,0.0],
                      "primary_error":[0.0,0.0],"rotation_deg":90.0,"noise":3.0,
                      "paper":"Target","paper_depth":1.0,"paper_offset":[6.0,-4.0]}"#;
        let s: Scene = serde_json::from_str(old).unwrap();
        assert_eq!((s.axis_offset, s.rotation_deg, s.jig), ([1.0, 2.0], 90.0, false));
    }
}
