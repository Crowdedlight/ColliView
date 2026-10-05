//! PNG snapshots at sensor resolution, optionally with the overlay burnt in.

use std::path::Path;

use anyhow::Result;
use egui::{Color32, ColorImage, Pos2, Vec2, pos2};

use crate::overlay::{LayerKind, Overlay};
use crate::view::Orientation;

/// Compose the frame with the visible layers. Line widths are scaled from screen
/// pixels so that a 2 px line on a ~720 px tall view looks similar in the file.
pub fn compose(frame: &ColorImage, overlay: Option<&Overlay>, orientation: Orientation) -> ColorImage {
    let mut img = frame.clone();
    let [w, h] = img.size;
    if let Some(o) = overlay {
        let k = (h as f32 / 720.0).max(1.0);
        for layer in o.layers.iter().rev().filter(|l| l.visible) {
            let c = o.layer_center(layer);
            let half = 0.5 * layer.thickness * k;
            match layer.kind {
                LayerKind::Circle { radius } => {
                    stamp(&mut img, layer.color, |p| ((p - c).length() - radius).abs() - half, c, radius + half + 1.0)
                }
                LayerKind::Crosshair { angle_deg } => {
                    for extra in [0.0_f32, 90.0] {
                        let n = Vec2::angled((angle_deg + extra).to_radians()).rot90();
                        let reach = (w.max(h) * 2) as f32;
                        stamp(&mut img, layer.color, |p| (p - c).dot(n).abs() - half, c, reach);
                    }
                }
            }
        }
    }
    apply_orientation(&img, orientation)
}

/// Blend `color` wherever the signed distance `sd(p)` is ≤ 0, anti-aliased over 1 px.
fn stamp(img: &mut ColorImage, color: Color32, sd: impl Fn(Pos2) -> f32, c: Pos2, reach: f32) {
    let [w, h] = img.size;
    let x0 = ((c.x - reach).floor().max(0.0)) as usize;
    let y0 = ((c.y - reach).floor().max(0.0)) as usize;
    let x1 = ((c.x + reach).ceil().max(0.0) as usize).min(w);
    let y1 = ((c.y + reach).ceil().max(0.0) as usize).min(h);
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let alpha = a as f32 / 255.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let cov = (0.5 - sd(pos2(x as f32 + 0.5, y as f32 + 0.5))).clamp(0.0, 1.0) * alpha;
            if cov > 0.0 {
                let px = &mut img.pixels[y * w + x];
                let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * cov).round() as u8;
                *px = Color32::from_rgb(mix(px.r(), r), mix(px.g(), g), mix(px.b(), b));
            }
        }
    }
}

fn apply_orientation(img: &ColorImage, o: Orientation) -> ColorImage {
    if !o.flip_h && !o.flip_v {
        return img.clone();
    }
    let [w, h] = img.size;
    let pixels = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let sx = if o.flip_h { w - 1 - x } else { x };
            let sy = if o.flip_v { h - 1 - y } else { y };
            img.pixels[sy * w + sx]
        })
        .collect();
    ColorImage::new([w, h], pixels)
}

pub fn save_png(img: &ColorImage, path: &Path) -> Result<()> {
    let [w, h] = img.size;
    let rgb: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]).collect();
    image::save_buffer(path, &rgb, w as u32, h as u32, image::ExtendedColorType::Rgb8)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burns_in_circle_and_flips() {
        let frame = ColorImage::filled([200, 100], Color32::BLACK);
        let mut o = Overlay::newtonian_preset(200, 100);
        o.layers.iter_mut().for_each(|l| l.visible = false);
        o.layers[0].visible = true; // focuser ring, radius 42 around (100, 50)
        o.center = pos2(60.0, 50.0);
        let out = compose(&frame, Some(&o), Orientation::default());
        let at = |img: &ColorImage, x: usize, y: usize| img.pixels[y * 200 + x];
        assert_ne!(at(&out, 60 + 42, 50), Color32::BLACK);
        assert_eq!(at(&out, 60, 50), Color32::BLACK);
        let flipped = compose(&frame, Some(&o), Orientation { flip_h: true, flip_v: false });
        assert_ne!(at(&flipped, 199 - (60 + 42), 50), Color32::BLACK);
    }
}
