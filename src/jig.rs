//! The two-depth centring jig (`part = "jig"` in the adapter model).
//!
//! The jig holds the adapter by its barrel, like the focuser, and has two holes on its
//! axis: a wide aperture just below the barrel and a small hole further down, lit from
//! behind. Seen through the camera, their centres coincide exactly when the lens sits on
//! the barrel's axis. A tilted camera or an off-centre sensor shifts both alike. A lens
//! offset `e` moves the near one more than the far one:
//!
//! ```text
//! Δ = c_far − c_near = f·e·(1/z1 − 1/z2)
//! ```
//!
//! The focal length `f` (px) follows from the aperture's size in the image, so the offset
//! comes out in mm. The zero is exact. The scale is approximate, since the distances are
//! measured from the lens front rather than its entrance pupil.

use egui::Vec2;
use serde::{Deserialize, Serialize};

use crate::detect::Detection;

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

    /// Image shift between the two centres per mm of lens offset, for an aperture of
    /// `near_radius_px` in the image.
    pub fn px_per_mm(&self, near_radius_px: f32) -> f32 {
        // f = r1_px · z1 / r1, and f · (1/z1 − 1/z2) = (r1_px / r1) · (1 − z1/z2).
        near_radius_px / (0.5 * self.near_d) * (1.0 - self.near_dist / self.far_dist)
    }

    /// The lens's offset from the jig's axis (mm), along the image axes.
    pub fn lens_offset_mm(&self, near: &Detection, far: &Detection) -> Vec2 {
        (far.center - near.center) / self.px_per_mm(near.radius).max(1e-6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, pos2, vec2};

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
        let k = g.px_per_mm(560.0);
        assert!((k - 38.9).abs() < 0.1, "{k}");
        let e = g.lens_offset_mm(&near, &det(near.center + vec2(2.0, -1.0) * k, r_far));
        assert!((e - vec2(2.0, -1.0)).length() < 1e-4, "{e:?}");
    }
}
