//! The two-depth centring jig (`part = "jig"` in the adapter model).
//!
//! The jig holds the adapter by its barrel, like the focuser, and has two holes on its
//! axis: a wide aperture just below the barrel and a small hole further down, lit from
//! behind. A hole at depth `z` shows at `A − f·e/z`, where `e` is the lens's offset from
//! the axis and `A` is where the axis itself appears. A tilted camera or a lens off-centre
//! over the sensor moves `A`, so both holes shift alike. A lens offset moves the near one
//! more than the far one:
//!
//! ```text
//! Δ = c_far − c_near = f·e·(1/z1 − 1/z2)
//! ```
//!
//! So the holes line up exactly when the lens sits on the axis, whatever the tilt. The
//! focal length `f` (px) follows from the aperture's size in the image, so the offset
//! comes out in mm. The zero is exact. The scale is approximate, since the distances are
//! measured from the lens front rather than its entrance pupil.

use egui::{Pos2, Vec2, vec2};
use serde::{Deserialize, Serialize};

use crate::detect::{DetectParams, Detection, GrayImage, Polarity, detect_circle};

/// The jig's holes, in mm. The distances are from the front of the lens.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JigGeometry {
    pub near_d: f32,
    pub near_dist: f32,
    pub far_d: f32,
    pub far_dist: f32,
}

impl Default for JigGeometry {
    /// The jig the adapter model prints with its defaults (`imx179_db05`); its console
    /// output lists the values for other settings.
    fn default() -> Self {
        Self { near_d: 16.0, near_dist: 44.4, far_d: 6.0, far_dist: 100.0 }
    }
}

impl JigGeometry {
    /// The far hole's radius over the near aperture's, in the image.
    pub fn far_radius_ratio(&self) -> f32 {
        (self.far_d / self.far_dist) / (self.near_d / self.near_dist).max(1e-6)
    }

    /// The focal length (px), for an aperture of `near_radius_px` in the image.
    pub fn focal_px(&self, near_radius_px: f32) -> f32 {
        near_radius_px * self.near_dist / (0.5 * self.near_d).max(1e-6)
    }

    /// Image shift between the two centres per mm of lens offset, for an aperture of
    /// `near_radius_px` in the image.
    pub fn px_per_mm(&self, near_radius_px: f32) -> f32 {
        self.focal_px(near_radius_px) * (1.0 / self.near_dist - 1.0 / self.far_dist)
    }

    /// The lens's offset from the jig's axis (mm), along the image axes.
    pub fn lens_offset_mm(&self, near: &Detection, far: &Detection) -> Vec2 {
        (far.center - near.center) / self.px_per_mm(near.radius).max(1e-6)
    }

    /// Where the jig's axis appears: the point both holes meet at once the lens is
    /// centred. Only camera tilt, or a lens off-centre over the sensor, moves it.
    pub fn axis_point(&self, near: &Detection, far: &Detection) -> Pos2 {
        // A = c_far + f·e/z2, and f·e/z2 = Δ · z1 / (z2 − z1).
        let k = self.near_dist / (self.far_dist - self.near_dist).max(1e-3);
        far.center + (far.center - near.center) * k
    }
}

/// Finds the jig's holes in one frame: the wide aperture, near `seed` (the last fit) or
/// else anywhere in the frame, then the backlit hole from the aperture's centre.
pub fn measure(gray: &GrayImage, seed: Option<&Detection>, g: &JigGeometry) -> Option<(Detection, Detection)> {
    let near = match seed {
        Some(s) => DetectParams::around(s.center, s.radius, 0.12),
        None => DetectParams::wide(vec2(gray.width as f32, gray.height as f32)),
    };
    // Only edges that are dark inside count: the baffle's edge, just outside the
    // aperture, is bright inside and would otherwise win.
    let near = detect_circle(gray, &near.with_polarity(Polarity::DarkInside))?;
    // Wide band: a lens offset moves the hole off the aperture's centre.
    let far = DetectParams::around(near.center, near.radius * g.far_radius_ratio(), 0.5)
        .with_polarity(Polarity::BrightInside);
    Some((near, detect_circle(gray, &far)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    fn det(center: Pos2, radius: f32) -> Detection {
        Detection { center, radius, inlier_ratio: 1.0, rms: 0.1 }
    }

    #[test]
    fn centred_lens_reads_zero_and_offsets_scale() {
        let g = JigGeometry::default();
        let near = det(pos2(640.0, 480.0), 560.0);
        let r_far = 560.0 * g.far_radius_ratio();
        assert!(g.lens_offset_mm(&near, &det(near.center, r_far)).length() < 1e-6);
        // 560 px for an 8 mm radius at 44.4 mm, so f ≈ 3108 px: 3108 · (1/44.4 − 1/100) ≈ 38.9 px/mm.
        assert!((g.focal_px(560.0) - 3108.0).abs() < 0.1);
        let k = g.px_per_mm(560.0);
        assert!((k - 38.9).abs() < 0.1, "{k}");
        let e = g.lens_offset_mm(&near, &det(near.center + vec2(2.0, -1.0) * k, r_far));
        assert!((e - vec2(2.0, -1.0)).length() < 1e-4, "{e:?}");
    }

    /// The holes show at A − f·e/z: the axis point stays on A as the lens moves, and the
    /// lens offset reads e wherever A is.
    #[test]
    fn axis_point_ignores_the_lens_offset() {
        let g = JigGeometry::default();
        let (r, axis) = (560.0, pos2(652.0, 471.0));
        let f = g.focal_px(r);
        for e in [vec2(0.0, 0.0), vec2(0.3, -0.2), vec2(-1.0, 0.5)] {
            let near = det(axis - e * f / g.near_dist, r);
            let far = det(axis - e * f / g.far_dist, r * g.far_radius_ratio());
            assert!((g.axis_point(&near, &far) - axis).length() < 1e-3, "{e:?}");
            assert!((g.lens_offset_mm(&near, &far) - e).length() < 1e-4, "{e:?}");
        }
    }
}
