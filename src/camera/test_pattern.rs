//! A simulated view down a Newtonian focuser, so the app (and the tests) can run
//! without a camera.
//!
//! The model matches what the rotation calibration assumes: the drawtube rim is on the
//! focuser axis, so it stays put when the adapter rotates; camera-induced offsets
//! (`parallax`) are fixed in the camera frame and grow with feature depth; real
//! misalignments rotate with the adapter.

use std::time::{Duration, Instant};

use egui::{Pos2, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

use super::{Capture, PixelFormat, RgbFrame, VideoMode};
use crate::detect::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
    use crate::calibrate::{COAXIAL_TOLERANCE, RotationSample, solve};
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
}
