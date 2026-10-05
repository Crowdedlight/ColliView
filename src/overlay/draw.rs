use std::f32::consts::TAU;

use egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2, vec2};

use super::{LayerKind, Measurement, Overlay};
use crate::view::ViewTransform;

/// Paint all visible layers. The top of the list is drawn last.
pub fn paint_overlay(painter: &Painter, vt: &ViewTransform, overlay: &Overlay) {
    let diag = vt.image_size.length();
    for layer in overlay.layers.iter().rev().filter(|l| l.visible) {
        let c = vt.to_screen(overlay.layer_center(layer));
        let stroke = Stroke::new(layer.thickness, layer.color);
        let selected = overlay.selected == Some(layer.id);
        match layer.kind {
            LayerKind::Circle { radius } => {
                let r = radius * vt.scale();
                if selected {
                    let halo = Color32::from_rgba_unmultiplied(255, 255, 255, 40);
                    painter.circle_stroke(c, r, Stroke::new(layer.thickness + 4.0, halo));
                }
                painter.circle_stroke(c, r, stroke);
            }
            LayerKind::Crosshair { angle_deg } => {
                let half = diag * vt.scale();
                // Angles are given in raw-image space; map the direction through the
                // view so flips mirror the crosshair together with the image.
                for extra in [0.0_f32, 90.0] {
                    let a = (angle_deg + extra).to_radians();
                    let p = overlay.layer_center(layer);
                    let dir_screen = (vt.to_screen(p + Vec2::angled(a)) - c).normalized();
                    painter.line_segment([c - dir_screen * half, c + dir_screen * half], stroke);
                }
            }
        }
    }
    if overlay.show_center_marker {
        paint_center_marker(painter, vt.to_screen(overlay.center));
    }
}

/// A small high-contrast cross marking the shared centre (also the drag handle).
pub fn paint_center_marker(painter: &Painter, c: Pos2) {
    let s = 7.0;
    for (w, col) in [(3.0, Color32::BLACK), (1.0, Color32::WHITE)] {
        let st = Stroke::new(w, col);
        painter.line_segment([c - vec2(s, 0.0), c + vec2(s, 0.0)], st);
        painter.line_segment([c - vec2(0.0, s), c + vec2(0.0, s)], st);
    }
}

/// Dashed circle used for auto-detect previews and measurements.
pub fn paint_dashed_circle(painter: &Painter, vt: &ViewTransform, m: &Measurement, color: Color32) {
    let c = vt.to_screen(m.center);
    let r = m.radius * vt.scale();
    let n = ((r * TAU / 4.0) as usize).clamp(32, 2048);
    let mut pts: Vec<Pos2> = (0..=n).map(|i| c + Vec2::angled(i as f32 / n as f32 * TAU) * r).collect();
    pts.dedup();
    let shadow = Shape::dashed_line(&pts, Stroke::new(3.0, Color32::from_black_alpha(160)), 8.0, 6.0);
    let dash = Shape::dashed_line(&pts, Stroke::new(1.5, color), 8.0, 6.0);
    painter.extend(shadow);
    painter.extend(dash);
    paint_center_marker(painter, c);
}
