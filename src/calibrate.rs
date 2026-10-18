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
//! `b` is parallax between the rim's depth and the feature's: it grows with how far the
//! lens sits off the focuser axis, and a tilted camera alone doesn't cause it (tilt
//! shifts every depth alike, and the overlays follow the rim). So turning the adapter's
//! carrier screws until `b` ≈ 0 puts the lens on the axis, like a Cheshire's peephole.
//! For that, [`live_bias`] reads `b` live with the adapter left at 180°, where a
//! measurement is `m = b − v`: `b = m + v`.
//!
//! ## Paper target
//!
//! A sheet with squares taped inside the tube needs no circle fit at all. Turning the
//! adapter 180° turns the paper's image about one point: where the focuser axis meets the
//! paper ([`Registration::fixed_point`]). The paper is fixed to the telescope, so where it
//! sits doesn't matter. The bias is that point minus the sensor centre, so it is zero
//! when the lens is on the axis, *assuming* the camera looks straight down the focuser:
//! no tilt, and the lens centred over the sensor. The 3D-printed carrier is meant to give
//! that. The rotation calibration above can show whatever is left over.
//!
//! [`Registration::fixed_point`]: crate::register::Registration::fixed_point

use std::fmt;

use egui::{Pos2, Vec2};

use crate::overlay::Measurement;
use crate::register::{RegError, Registration};

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
    /// Camera-induced offset at the feature's depth (raw px).
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

/// A live bias below this fraction of the rim radius counts as centred.
pub const CENTRED_TOLERANCE: f32 = 0.0025;

/// The camera bias right now, from a live sample with the adapter still at 180° (where
/// m = b − v). Unlike `result.bias`, it follows the carrier screws as they turn.
pub fn live_bias(result: &CalibrationResult, live: &RotationSample) -> Vec2 {
    live.offset() + result.misalignment
}

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

/// Below this ZNCC the two views don't show the same paper.
pub const MIN_MATCH: f32 = 0.5;
/// How far from 180° the adapter may have been turned.
pub const MAX_TURN_ERROR_DEG: f32 = 30.0;

/// Paper-target result from the 0° and 180° captures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperResult {
    /// Where the focuser axis meets the paper, in both captures.
    pub c_paper: Pos2,
    /// Where it should be: the sensor centre.
    pub target: Pos2,
    /// `c_paper − target`.
    pub bias: Vec2,
    /// How far the adapter was turned, as measured from the paper.
    pub rotation_deg: f32,
    /// ZNCC of the aligned views.
    pub match_score: f32,
    /// The focuser ring's radius, for expressing the bias as a percentage.
    pub ring_radius: f32,
}

impl PaperResult {
    pub fn percent(&self, v: Vec2) -> f32 {
        100.0 * v.length() / self.ring_radius.max(1e-3)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaperError {
    Repeats,
    NoMatch,
    LowMatch(f32),
    Turn(f32),
}

impl From<RegError> for PaperError {
    fn from(e: RegError) -> Self {
        match e {
            RegError::Ambiguous { .. } => Self::Repeats,
            RegError::NoMatch => Self::NoMatch,
        }
    }
}

impl fmt::Display for PaperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repeats => write!(
                f,
                "The pattern repeats, so the match is ambiguous: draw a few bold, irregular marks on the \
                 paper, or print calibration/centring_target.pdf"
            ),
            Self::NoMatch | Self::LowMatch(_) => write!(
                f,
                "The two captures don't match: the paper moved or has too little texture. Tape it down, \
                 light it evenly and capture both again"
            ),
            Self::Turn(deg) => write!(f, "The adapter was turned {deg:.0}°: turn it closer to 180° and capture again"),
        }
    }
}

/// Solve the paper calibration from the 0° → 180° registration. `target` is the sensor
/// centre.
pub fn solve_paper(reg: &Registration, target: Pos2, ring_radius: f32) -> Result<PaperResult, PaperError> {
    let rotation_deg = reg.rotation_deg();
    if (rotation_deg - 180.0).abs() > MAX_TURN_ERROR_DEG {
        return Err(PaperError::Turn(rotation_deg));
    }
    if reg.score < MIN_MATCH {
        return Err(PaperError::LowMatch(reg.score));
    }
    let c_paper = reg.fixed_point().ok_or(PaperError::Turn(rotation_deg))?;
    Ok(PaperResult { c_paper, target, bias: c_paper - target, rotation_deg, match_score: reg.score, ring_radius })
}

/// Where the axis meets the paper in the live frame. `live` maps the 180° capture onto
/// it; `c_paper` is a point on the paper, so it moves with the paper's image when the
/// carrier screws move the camera.
pub fn paper_live_axis(result: &PaperResult, live: &Registration) -> Pos2 {
    live.map(result.c_paper)
}

/// The camera bias right now, with the adapter left at 180°.
pub fn paper_live_bias(result: &PaperResult, live: &Registration) -> Vec2 {
    paper_live_axis(result, live) - result.target
}

/// Rough label for a match score.
pub fn match_quality(score: f32) -> &'static str {
    match score {
        s if s >= 0.8 => "good",
        s if s >= 0.65 => "fair",
        _ => "poor",
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
    fn live_bias_tracks_camera_shift() {
        let rim = pos2(640.0, 480.0);
        let (b, v) = (vec2(3.0, -1.5), vec2(-4.0, 2.0));
        let r = solve(&sample(rim, 300.0, rim + b + v), &sample(rim, 300.0, rim + b - v), COAXIAL_TOLERANCE);
        // Still at 180°, the screws move the camera: the bias changes, v stays.
        let b2 = vec2(-0.5, 0.25);
        let rim2 = rim + vec2(2.0, 1.0);
        let live = live_bias(&r, &sample(rim2, 300.0, rim2 + b2 - v));
        assert!((live - b2).length() < 1e-5, "{live:?} vs {b2:?}");
    }

    /// A turn by `deg` about `c`, then the camera shifted by `shift`.
    fn turn(c: Pos2, deg: f32, shift: Vec2) -> Registration {
        let (s, k) = deg.to_radians().sin_cos();
        let a = [k, -s, s, k];
        let ac = vec2(k * c.x - s * c.y, s * c.x + k * c.y);
        Registration { a, t: c.to_vec2() - ac + shift, score: 0.9 }
    }

    #[test]
    fn paper_bias_is_the_fixed_point_minus_the_sensor_centre() {
        let (centre, c) = (pos2(640.0, 480.0), pos2(643.5, 478.0));
        let r = solve_paper(&turn(c, 178.0, Vec2::ZERO), centre, 300.0).unwrap();
        assert!((r.bias - vec2(3.5, -2.0)).length() < 1e-3, "{:?}", r.bias);
        assert!((r.rotation_deg - 178.0).abs() < 1e-3);
        // The screws move the camera: the paper's image shifts by 5 px.
        let live = turn(Pos2::ZERO, 0.0, vec2(5.0, 0.0));
        let b = paper_live_bias(&r, &live);
        assert!((b - vec2(8.5, -2.0)).length() < 1e-3, "{b:?}");
        let bad = solve_paper(&turn(c, 140.0, Vec2::ZERO), centre, 300.0);
        assert!(matches!(bad, Err(PaperError::Turn(_))));
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
