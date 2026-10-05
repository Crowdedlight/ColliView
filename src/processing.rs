//! Software image adjustments applied in the camera worker.
//!
//! Only pixel values change here. Geometry (flips/rotation) is a display-only
//! transform so overlay coordinates always stay in raw sensor space.

use egui::{Color32, ColorImage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Processing {
    /// Added after contrast, in -1..=1 of full scale.
    pub brightness: f32,
    /// Multiplier around mid-grey.
    pub contrast: f32,
    /// > 1 brightens shadows.
    pub gamma: f32,
    pub invert: bool,
    pub grayscale: bool,
    /// Running average over roughly this many frames (1 = off).
    pub average_frames: u32,
}

impl Default for Processing {
    fn default() -> Self {
        Self { brightness: 0.0, contrast: 1.0, gamma: 1.0, invert: false, grayscale: false, average_frames: 1 }
    }
}

impl Processing {
    pub fn is_identity(&self) -> bool {
        let d = Self::default();
        self.brightness == d.brightness
            && self.contrast == d.contrast
            && self.gamma == d.gamma
            && !self.invert
            && !self.grayscale
            && self.average_frames <= 1
    }

    fn lut(&self) -> [u8; 256] {
        let mut lut = [0u8; 256];
        for (i, out) in lut.iter_mut().enumerate() {
            let mut v = i as f32 / 255.0;
            v = (v - 0.5) * self.contrast + 0.5 + self.brightness;
            v = v.clamp(0.0, 1.0).powf(1.0 / self.gamma.max(0.05));
            if self.invert {
                v = 1.0 - v;
            }
            *out = (v * 255.0).round() as u8;
        }
        lut
    }
}

/// Holds the LUT and the running-average buffer between frames.
#[derive(Default)]
pub struct Processor {
    settings: Processing,
    lut: Option<[u8; 256]>,
    accum: Vec<f32>,
    accum_size: (usize, usize),
}

impl Processor {
    pub fn set(&mut self, settings: Processing) {
        if settings != self.settings {
            if settings.average_frames != self.settings.average_frames {
                self.accum.clear();
            }
            self.settings = settings;
            self.lut = None;
        }
    }

    /// RGB8 in, display-ready image out.
    pub fn process(&mut self, width: usize, height: usize, rgb: &[u8]) -> ColorImage {
        let s = self.settings;
        let averaged;
        let src: &[u8] = if s.average_frames > 1 {
            if self.accum_size != (width, height) || self.accum.len() != rgb.len() {
                self.accum = rgb.iter().map(|&v| v as f32).collect();
                self.accum_size = (width, height);
            } else {
                let alpha = 1.0 / s.average_frames as f32;
                for (a, &v) in self.accum.iter_mut().zip(rgb) {
                    *a += (v as f32 - *a) * alpha;
                }
            }
            averaged = self.accum.iter().map(|&v| v.round() as u8).collect::<Vec<u8>>();
            &averaged
        } else {
            rgb
        };
        if s.is_identity() || (s.average_frames > 1 && Processing { average_frames: 1, ..s }.is_identity()) {
            return ColorImage::from_rgb([width, height], src);
        }
        let lut = *self.lut.get_or_insert_with(|| s.lut());
        let pixels = src
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| {
                if s.grayscale {
                    let y = ((77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8) as usize;
                    let v = lut[y];
                    Color32::from_rgb(v, v, v)
                } else {
                    Color32::from_rgb(lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize])
                }
            })
            .collect();
        ColorImage::new([width, height], pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_passes_pixels_through() {
        let mut p = Processor::default();
        let rgb = [10, 20, 30, 200, 100, 0];
        let img = p.process(2, 1, &rgb);
        assert_eq!(img.pixels[0], Color32::from_rgb(10, 20, 30));
        assert_eq!(img.pixels[1], Color32::from_rgb(200, 100, 0));
    }

    #[test]
    fn invert_and_contrast() {
        let mut p = Processor::default();
        p.set(Processing { invert: true, ..Default::default() });
        assert_eq!(p.process(1, 1, &[0, 255, 128]).pixels[0], Color32::from_rgb(255, 0, 127));
        p.set(Processing { contrast: 2.0, ..Default::default() });
        let c = p.process(1, 1, &[32, 128, 224]).pixels[0];
        assert!(c.r() < 32 && c.b() > 224 && c.g().abs_diff(128) <= 1, "{c:?}");
    }

    #[test]
    fn averaging_converges() {
        let mut p = Processor::default();
        p.set(Processing { average_frames: 4, ..Default::default() });
        p.process(1, 1, &[0, 0, 0]);
        let mut last = Color32::BLACK;
        for _ in 0..40 {
            last = p.process(1, 1, &[200, 200, 200]).pixels[0];
        }
        assert_eq!(last, Color32::from_rgb(200, 200, 200));
    }
}
