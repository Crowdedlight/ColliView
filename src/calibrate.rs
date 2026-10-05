//! Rotation calibration: separate camera bias from real misalignment.
//!
//! Measure a feature's offset from the drawtube rim centre, rotate the adapter 180°
//! in the focuser, and measure again. Anything caused by the camera (off-centre
//! sensor, tilt, parallax at that feature's depth) stays fixed in the camera frame,
//! while a real misalignment of the telescope rotates with the scene:
//!
//! ```text
//! m0   = b + v
//! m180 = b − v      ⇒  b = (m0 + m180) / 2,   v = (m0 − m180) / 2
//! ```
//!
//! Writing `b` into the layer's offset makes its circle land where a perfectly
//! aligned telescope appears through this particular camera.

use egui::{Pos2, Vec2};

use crate::overlay::Measurement;

/// One measurement at a given adapter rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationSample {
    /// The reference (drawtube rim) fit.
    pub rim: Measurement,
    /// Centre of the feature being calibrated (e.g. the secondary outline).
    pub target: Pos2,
}

impl RotationSample {
    pub fn offset(&self) -> Vec2 {
        self.target - self.rim.center
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibrationResult {
    /// Camera-induced offset; becomes the layer offset.
    pub bias: Vec2,
    /// Real telescope misalignment at 0° (raw px).
    pub misalignment: Vec2,
    /// How far the rim centre moved between the two fits (px).
    pub rim_shift: f32,
    /// How much the rim radius changed between the two fits (px).
    pub rim_radius_change: f32,
    /// False if the adapter did not rotate about the focuser axis.
    pub coaxial: bool,
    /// Rim radius at 0°, for expressing results as a percentage.
    pub rim_radius: f32,
}

impl CalibrationResult {
    pub fn misalignment_percent(&self) -> f32 {
        100.0 * self.misalignment.length() / self.rim_radius.max(1e-3)
    }
}

/// Default coaxiality tolerance: 1.5 % of the rim radius.
pub const COAXIAL_TOLERANCE: f32 = 0.015;

pub fn solve(s0: &RotationSample, s180: &RotationSample, tolerance_frac: f32) -> CalibrationResult {
    let (m0, m180) = (s0.offset(), s180.offset());
    let rim_shift = (s180.rim.center - s0.rim.center).length();
    let rim_radius_change = (s180.rim.radius - s0.rim.radius).abs();
    // Allow at least a pixel so tiny rims aren't held to sub-pixel repeatability.
    let tol = (tolerance_frac * s0.rim.radius).max(1.0);
    CalibrationResult {
        bias: (m0 + m180) / 2.0,
        misalignment: (m0 - m180) / 2.0,
        rim_shift,
        rim_radius_change,
        coaxial: rim_shift <= tol && rim_radius_change <= tol,
        rim_radius: s0.rim.radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn sample(rim_c: Pos2, rim_r: f32, target: Pos2) -> RotationSample {
        RotationSample { rim: Measurement { center: rim_c, radius: rim_r }, target }
    }

    #[test]
    fn separates_bias_and_misalignment() {
        let rim = pos2(640.0, 480.0);
        let (b, v) = (vec2(3.0, -1.5), vec2(-4.0, 2.0));
        let r = solve(&sample(rim, 300.0, rim + b + v), &sample(rim, 300.0, rim + b - v), COAXIAL_TOLERANCE);
        assert!((r.bias - b).length() < 1e-5);
        assert!((r.misalignment - v).length() < 1e-5);
        assert!(r.coaxial);
    }

    #[test]
    fn flags_non_coaxial_rotation() {
        let r = solve(
            &sample(pos2(640.0, 480.0), 300.0, pos2(645.0, 480.0)),
            &sample(pos2(652.0, 480.0), 300.0, pos2(650.0, 480.0)),
            COAXIAL_TOLERANCE,
        );
        assert!(!r.coaxial);
        assert!((r.rim_shift - 12.0).abs() < 1e-5);
    }
}
