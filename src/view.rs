//! Mapping between raw sensor pixels and screen points.
//!
//! All overlay geometry lives in raw sensor coordinates. The live texture and every
//! overlay go through the same [`ViewTransform`], so resizing the window, zooming,
//! panning or flipping the view can never move a circle relative to the image.

use egui::{Pos2, Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 32.0;

/// Display-only orientation. "Rotate 180°" is both flips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orientation {
    pub flip_h: bool,
    pub flip_v: bool,
}

/// Persistent view settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewState {
    /// 1.0 = whole image fits the view.
    pub zoom: f32,
    /// Raw image point shown at the centre of the view. `None` = image centre.
    pub pan: Option<Pos2>,
    pub orientation: Orientation,
    /// Use nearest-neighbour sampling when zoomed in, for pixel-exact fit checks.
    pub nearest_when_zoomed: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self { zoom: 1.0, pan: None, orientation: Orientation::default(), nearest_when_zoomed: true }
    }
}

impl ViewState {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = None;
    }

    /// Zoom by `factor`, keeping the image point under `anchor` (screen) fixed.
    pub fn zoom_at(&mut self, vt: &ViewTransform, anchor: Pos2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if new_zoom == self.zoom {
            return;
        }
        let under_cursor = vt.to_image(anchor);
        let new_scale = vt.fit_scale() * new_zoom;
        let oriented_pan = vt.orient(under_cursor) - (anchor - vt.rect.center()) / new_scale;
        self.zoom = new_zoom;
        self.pan = Some(vt.clamp_to_image(vt.orient(oriented_pan)));
    }

    /// Set an absolute zoom level centred on an image point.
    pub fn zoom_to(&mut self, zoom: f32, center: Pos2) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = Some(center);
    }

    /// Pan by a screen-space drag delta.
    pub fn pan_by(&mut self, vt: &ViewTransform, screen_delta: Vec2) {
        let oriented = vt.orient(vt.pan) - screen_delta / vt.scale();
        self.pan = Some(vt.clamp_to_image(vt.orient(oriented)));
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ViewTransform {
    pub rect: Rect,
    pub image_size: Vec2,
    pan: Pos2,
    scale: f32,
    orientation: Orientation,
}

impl ViewTransform {
    pub fn new(rect: Rect, image_size: Vec2, state: &ViewState) -> Self {
        let fit = fit_scale(rect, image_size);
        let pan = state.pan.unwrap_or((image_size / 2.0).to_pos2());
        Self {
            rect,
            image_size,
            pan,
            scale: fit * state.zoom.clamp(MIN_ZOOM, MAX_ZOOM),
            orientation: state.orientation,
        }
    }

    /// Screen pixels per sensor pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn fit_scale(&self) -> f32 {
        fit_scale(self.rect, self.image_size)
    }

    /// Applies the display flips. It is its own inverse.
    pub fn orient(self, p: Pos2) -> Pos2 {
        pos2(
            if self.orientation.flip_h { self.image_size.x - p.x } else { p.x },
            if self.orientation.flip_v { self.image_size.y - p.y } else { p.y },
        )
    }

    pub fn to_screen(self, p: Pos2) -> Pos2 {
        self.rect.center() + (self.orient(p) - self.orient(self.pan)) * self.scale
    }

    pub fn to_image(self, s: Pos2) -> Pos2 {
        let oriented = self.orient(self.pan) + (s - self.rect.center()) / self.scale;
        self.orient(oriented)
    }

    /// Where the whole image lands on screen, and the matching UV rectangle.
    /// The UV rect is "inverted" on flipped axes, which egui's mesh builder handles.
    pub fn image_quad(&self) -> (Rect, Rect) {
        let screen = Rect::from_two_pos(self.to_screen(Pos2::ZERO), self.to_screen(self.image_size.to_pos2()));
        let (u0, u1) = if self.orientation.flip_h { (1.0, 0.0) } else { (0.0, 1.0) };
        let (v0, v1) = if self.orientation.flip_v { (1.0, 0.0) } else { (0.0, 1.0) };
        (screen, Rect::from_min_max(pos2(u0, v0), pos2(u1, v1)))
    }

    pub fn clamp_to_image(&self, p: Pos2) -> Pos2 {
        pos2(p.x.clamp(0.0, self.image_size.x), p.y.clamp(0.0, self.image_size.y))
    }
}

fn fit_scale(rect: Rect, image_size: Vec2) -> f32 {
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return 1.0;
    }
    (rect.width() / image_size.x).min(rect.height() / image_size.y).max(1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Pos2, b: Pos2) -> bool {
        (a - b).length() < 1e-3
    }

    fn all_states() -> Vec<ViewState> {
        let mut out = Vec::new();
        for flip_h in [false, true] {
            for flip_v in [false, true] {
                for zoom in [1.0, 2.5, 8.0] {
                    for pan in [None, Some(pos2(100.0, 700.0))] {
                        out.push(ViewState {
                            zoom,
                            pan,
                            orientation: Orientation { flip_h, flip_v },
                            nearest_when_zoomed: false,
                        });
                    }
                }
            }
        }
        out
    }

    #[test]
    fn round_trip_all_orientations_and_zooms() {
        let rect = Rect::from_min_size(pos2(250.0, 30.0), Vec2::new(900.0, 640.0));
        for s in all_states() {
            let vt = ViewTransform::new(rect, Vec2::new(1280.0, 960.0), &s);
            for p in [pos2(0.0, 0.0), pos2(640.0, 480.0), pos2(1279.5, 3.25), pos2(17.0, 950.0)] {
                assert!(approx(vt.to_image(vt.to_screen(p)), p), "{s:?} {p:?}");
            }
        }
    }

    #[test]
    fn fit_letterboxes_and_centres_image() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 500.0));
        let vt = ViewTransform::new(rect, Vec2::new(1280.0, 960.0), &ViewState::default());
        let (screen, _) = vt.image_quad();
        assert!((screen.height() - 500.0).abs() < 1e-3);
        assert!(approx(screen.center(), rect.center()));
    }

    #[test]
    fn overlay_stays_locked_relative_to_image_when_resizing() {
        // A circle's screen position, expressed as a fraction of the image's screen
        // quad, must not depend on the window size.
        let img = Vec2::new(1280.0, 960.0);
        let p = pos2(700.0, 300.0);
        let mut fractions = Vec::new();
        for size in [Vec2::new(800.0, 600.0), Vec2::new(1920.0, 1000.0), Vec2::new(500.0, 900.0)] {
            let vt = ViewTransform::new(Rect::from_min_size(Pos2::ZERO, size), img, &ViewState::default());
            let (quad, _) = vt.image_quad();
            let s = vt.to_screen(p);
            fractions.push(((s.x - quad.min.x) / quad.width(), (s.y - quad.min.y) / quad.height()));
        }
        for f in &fractions[1..] {
            assert!((f.0 - fractions[0].0).abs() < 1e-5 && (f.1 - fractions[0].1).abs() < 1e-5);
        }
    }

    #[test]
    fn flip_moves_overlay_with_texture() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 960.0));
        let img = Vec2::new(1280.0, 960.0);
        let state = ViewState { orientation: Orientation { flip_h: true, flip_v: false }, ..Default::default() };
        let vt = ViewTransform::new(rect, img, &state);
        // Raw pixel (100, 50) is drawn mirrored horizontally.
        assert!(approx(vt.to_screen(pos2(100.0, 50.0)), pos2(1180.0, 50.0)));
        let (_, uv) = vt.image_quad();
        assert_eq!((uv.min.x, uv.max.x), (1.0, 0.0));
    }

    #[test]
    fn zoom_at_keeps_anchor_fixed() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), Vec2::new(800.0, 600.0));
        let img = Vec2::new(1280.0, 960.0);
        for s in all_states() {
            let mut state = s;
            let anchor = pos2(300.0, 250.0);
            let before = ViewTransform::new(rect, img, &state).to_image(anchor);
            let vt = ViewTransform::new(rect, img, &state);
            state.zoom_at(&vt, anchor, 1.5);
            let after_vt = ViewTransform::new(rect, img, &state);
            let after = after_vt.to_image(anchor);
            // The pan is clamped to the image, so only check when it stayed inside.
            let pan = state.pan.unwrap();
            if pan.x > 0.0 && pan.y > 0.0 && pan.x < img.x && pan.y < img.y {
                assert!(approx(before, after), "{s:?}: {before:?} vs {after:?}");
            }
        }
    }
}
