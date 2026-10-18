//! Overlay layers: concentric circles (and crosshairs) sharing one centre.
//!
//! Every layer is drawn at the shared centre, which the reference ring defines: the
//! point is to line the features up on one centre, so no layer has its own offset.
//!
//! Geometry is stored in raw sensor pixels; see `view.rs` for why.

pub mod draw;

use egui::{Color32, Pos2, Vec2, pos2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerId(pub u32);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayerKind {
    Circle {
        radius: f32,
    },
    /// Two perpendicular lines through the layer centre.
    Crosshair {
        angle_deg: f32,
    },
}

/// A circle fitted to an image feature, in raw sensor pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub center: Pos2,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub color: Color32,
    /// Line width in screen pixels, so lines stay crisp at any zoom.
    pub thickness: f32,
    pub kind: LayerKind,
    /// Last "Measure" result for this layer's feature (not persisted).
    #[serde(skip)]
    pub measured: Option<Measurement>,
}

impl Layer {
    pub fn radius(&self) -> Option<f32> {
        match self.kind {
            LayerKind::Circle { radius } => Some(radius),
            LayerKind::Crosshair { .. } => None,
        }
    }

    pub fn set_radius(&mut self, r: f32) {
        if let LayerKind::Circle { radius } = &mut self.kind {
            *radius = r.max(0.5);
        }
    }

    pub fn is_circle(&self) -> bool {
        matches!(self.kind, LayerKind::Circle { .. })
    }
}

/// What happened to the geometry when the capture resolution changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rescale {
    Unchanged,
    /// Same aspect ratio: everything was scaled by this factor.
    Scaled(f32),
    /// Aspect ratio changed; the camera may crop, so geometry was left alone.
    AspectChanged,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    /// Shared centre, defined by the reference (focuser drawtube) ring.
    pub center: Pos2,
    /// Resolution the geometry was defined at.
    pub ref_resolution: [u32; 2],
    /// Top of the list is drawn last (on top), like image-editor layers.
    pub layers: Vec<Layer>,
    pub selected: Option<LayerId>,
    /// The ring that defines the shared centre.
    pub reference: LayerId,
    pub show_center_marker: bool,
    next_id: u32,
}

impl Overlay {
    /// Default layer set for a Newtonian.
    pub fn newtonian_preset(width: u32, height: u32) -> Self {
        let r0 = 0.42 * width.min(height) as f32;
        let mut o = Self {
            center: pos2(width as f32 / 2.0, height as f32 / 2.0),
            ref_resolution: [width, height],
            layers: Vec::new(),
            selected: None,
            reference: LayerId(0),
            show_center_marker: true,
            next_id: 0,
        };
        let circle = |radius: f32| LayerKind::Circle { radius };
        let focuser = o.push("Focuser drawtube", Color32::from_rgb(255, 210, 0), circle(r0));
        o.push("Secondary outline", Color32::from_rgb(60, 220, 90), circle(0.72 * r0));
        o.push("Primary clips / reflection", Color32::from_rgb(240, 70, 60), circle(0.6 * r0));
        o.push("Light-ring reflection", Color32::from_rgb(230, 80, 230), circle(0.25 * r0));
        o.push("Primary centre spot", Color32::from_rgb(0, 220, 240), circle(0.05 * r0));
        let cross = o.push(
            "Crosshair",
            Color32::from_rgba_unmultiplied(255, 255, 255, 160),
            LayerKind::Crosshair { angle_deg: 0.0 },
        );
        o.layer_mut(cross).unwrap().visible = false;
        o.layer_mut(cross).unwrap().thickness = 1.0;
        o.reference = focuser;
        o.selected = Some(focuser);
        o
    }

    fn push(&mut self, name: &str, color: Color32, kind: LayerKind) -> LayerId {
        let id = LayerId(self.next_id);
        self.next_id += 1;
        self.layers.push(Layer {
            id,
            name: name.to_owned(),
            visible: true,
            color,
            thickness: 2.0,
            kind,
            measured: None,
        });
        id
    }

    pub fn add_circle(&mut self) -> LayerId {
        let r = self.reference_radius().unwrap_or(100.0) * 0.5;
        let n = self.layers.len() + 1;
        // Cycle through distinct hues so new layers are easy to tell apart.
        let hue = (n as f32 * 0.618_034).fract();
        let color = egui::ecolor::Hsva::new(hue, 0.8, 1.0, 1.0).into();
        let id = self.push(&format!("Circle {n}"), color, LayerKind::Circle { radius: r });
        // New layers go on top of the list.
        let l = self.layers.pop().unwrap();
        self.layers.insert(0, l);
        self.selected = Some(id);
        id
    }

    pub fn add_crosshair(&mut self) -> LayerId {
        let id = self.push("Crosshair", Color32::WHITE, LayerKind::Crosshair { angle_deg: 0.0 });
        let l = self.layers.pop().unwrap();
        self.layers.insert(0, l);
        self.selected = Some(id);
        id
    }

    pub fn duplicate(&mut self, id: LayerId) -> Option<LayerId> {
        let idx = self.index_of(id)?;
        let mut copy = self.layers[idx].clone();
        copy.id = LayerId(self.next_id);
        self.next_id += 1;
        copy.name = format!("{} copy", copy.name);
        copy.measured = None;
        let new_id = copy.id;
        self.layers.insert(idx, copy);
        self.selected = Some(new_id);
        Some(new_id)
    }

    /// The reference layer cannot be removed.
    pub fn remove(&mut self, id: LayerId) -> bool {
        if id == self.reference {
            return false;
        }
        let Some(idx) = self.index_of(id) else { return false };
        self.layers.remove(idx);
        if self.selected == Some(id) {
            self.selected = None;
        }
        true
    }

    /// Move a layer up (towards the top/front) or down in the list.
    pub fn move_layer(&mut self, id: LayerId, up: bool) {
        let Some(idx) = self.index_of(id) else { return };
        let target = if up { idx.checked_sub(1) } else { Some(idx + 1).filter(|&i| i < self.layers.len()) };
        if let Some(t) = target {
            self.layers.swap(idx, t);
        }
    }

    /// Make another circle the reference: the one Auto-detect fits. All circles share
    /// the centre, so nothing moves on screen.
    pub fn set_reference(&mut self, id: LayerId) {
        if self.layer(id).is_some_and(Layer::is_circle) {
            self.reference = id;
        }
    }

    pub fn index_of(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn selected_layer(&self) -> Option<&Layer> {
        self.selected.and_then(|id| self.layer(id))
    }

    pub fn reference_layer(&self) -> Option<&Layer> {
        self.layer(self.reference)
    }

    pub fn reference_radius(&self) -> Option<f32> {
        self.reference_layer().and_then(Layer::radius)
    }

    /// Set the shared centre and the reference ring radius (e.g. from auto-detect).
    pub fn fit_reference(&mut self, center: Pos2, radius: f32) {
        self.center = center;
        let reference = self.reference;
        if let Some(l) = self.layer_mut(reference) {
            l.set_radius(radius);
        }
    }

    pub fn adapt_to_resolution(&mut self, width: u32, height: u32) -> Rescale {
        let [ow, oh] = self.ref_resolution;
        if [ow, oh] == [width, height] || width == 0 || height == 0 {
            return Rescale::Unchanged;
        }
        self.ref_resolution = [width, height];
        if ow == 0 || oh == 0 {
            return Rescale::AspectChanged;
        }
        let old_aspect = ow as f32 / oh as f32;
        let new_aspect = width as f32 / height as f32;
        if (old_aspect - new_aspect).abs() > 0.01 {
            return Rescale::AspectChanged;
        }
        let k = width as f32 / ow as f32;
        self.center = (self.center.to_vec2() * k).to_pos2();
        for l in &mut self.layers {
            if let LayerKind::Circle { radius } = &mut l.kind {
                *radius *= k;
            }
            l.measured = None;
        }
        Rescale::Scaled(k)
    }

    /// Offset of a layer's measured feature from the shared centre, in raw px.
    pub fn measured_error(&self, layer: &Layer) -> Option<Vec2> {
        layer.measured.map(|m| m.center - self.center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_has_reference_at_the_image_centre() {
        let o = Overlay::newtonian_preset(1280, 960);
        assert_eq!(o.reference_layer().unwrap().name, "Focuser drawtube");
        assert_eq!(o.center, pos2(640.0, 480.0));
    }

    #[test]
    fn reference_cannot_be_removed() {
        let mut o = Overlay::newtonian_preset(640, 480);
        assert!(!o.remove(o.reference));
        let other = o.layers[1].id;
        assert!(o.remove(other));
    }

    #[test]
    fn same_aspect_rescales_uniformly() {
        let mut o = Overlay::newtonian_preset(640, 480);
        let sec = o.layers[1].id;
        let (r, rs) = (o.reference_radius().unwrap(), o.layer(sec).unwrap().radius().unwrap());
        assert_eq!(o.adapt_to_resolution(1280, 960), Rescale::Scaled(2.0));
        assert_eq!(o.center, pos2(640.0, 480.0));
        assert_eq!(o.reference_radius().unwrap(), 2.0 * r);
        assert_eq!(o.layer(sec).unwrap().radius().unwrap(), 2.0 * rs);
    }

    #[test]
    fn aspect_change_keeps_geometry() {
        let mut o = Overlay::newtonian_preset(640, 480);
        let before = o.clone();
        assert_eq!(o.adapt_to_resolution(1280, 720), Rescale::AspectChanged);
        assert_eq!(o.center, before.center);
        assert_eq!(o.layers, before.layers);
        assert_eq!(o.ref_resolution, [1280, 720]);
    }

    #[test]
    fn changing_reference_keeps_the_centre() {
        let mut o = Overlay::newtonian_preset(640, 480);
        let (sec, cross) = (o.layers[1].id, o.add_crosshair());
        let c = o.center;
        o.set_reference(sec);
        assert_eq!((o.reference, o.center), (sec, c));
        // Only a circle can be the reference.
        o.set_reference(cross);
        assert_eq!(o.reference, sec);
    }

    #[test]
    fn serde_round_trip() {
        let o = Overlay::newtonian_preset(1280, 960);
        let json = serde_json::to_string(&o).unwrap();
        let back: Overlay = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }

    /// Sessions and profiles saved when layers had their own offsets still load; the
    /// offsets are dropped, so every layer is back on the shared centre.
    #[test]
    fn old_layer_offsets_are_ignored() {
        let o = Overlay::newtonian_preset(1280, 960);
        let mut v = serde_json::to_value(&o).unwrap();
        for l in v["layers"].as_array_mut().unwrap() {
            l["offset"] = serde_json::json!({ "x": 3.5, "y": -2.0 });
        }
        let back: Overlay = serde_json::from_value(v).unwrap();
        assert_eq!(o, back);
    }
}
