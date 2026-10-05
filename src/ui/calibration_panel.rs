use egui::{Color32, DragValue, RichText, Ui};

use crate::app::{ColliViewApp, JobKind};

pub fn calibration_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(
        "If the camera isn't exactly on the focuser axis, features further down the tube look \
         shifted. Measuring at 0° and 180° adapter rotation separates that camera bias from real \
         misalignment.",
    );
    let o = &app.s.overlay;
    let candidates: Vec<_> =
        o.layers.iter().filter(|l| l.is_circle() && l.id != o.reference).map(|l| (l.id, l.name.clone())).collect();
    if app.calib.target.is_none_or(|t| !candidates.iter().any(|c| c.0 == t)) {
        app.calib.target = candidates.first().map(|c| c.0);
    }
    let target_name =
        app.calib.target.and_then(|t| candidates.iter().find(|c| c.0 == t)).map_or("—".into(), |c| c.1.clone());
    ui.horizontal(|ui| {
        ui.label("Layer");
        let mut changed = false;
        egui::ComboBox::from_id_salt("calib_target").selected_text(target_name).show_ui(ui, |ui| {
            for (id, name) in &candidates {
                changed |= ui.selectable_value(&mut app.calib.target, Some(*id), name).changed();
            }
        });
        if changed {
            app.calib.samples = [None, None];
            app.calib.result = None;
        }
    });
    ui.checkbox(&mut app.calib.manual, "Use current circle positions instead of auto-detect").on_hover_text(
        "Manual: before each capture, fit the focuser ring (centre) and the layer's circle \
         (Alt+drag its offset) onto the features yourself",
    );

    let busy = app.job.as_ref().is_some_and(|j| matches!(j.kind, JobKind::Calibration { .. }));
    ui.label("Capture at 0°, rotate the adapter 180° in the focuser, then capture again.");
    let steps = ["1. Capture at 0°", "2. Capture at 180°"];
    for (i, label) in steps.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let enabled = !busy
                && app.frame.is_some()
                && app.calib.target.is_some()
                && (i == 0 || app.calib.samples[0].is_some());
            if ui.add_enabled(enabled, egui::Button::new(*label)).clicked() {
                app.calibration_capture(i);
            }
            if let Some(s) = app.calib.samples[i] {
                let m = s.offset();
                ui.label(format!("✔ {:+.1}, {:+.1} px", m.x, m.y));
            }
        });
    }
    if busy {
        ui.spinner();
    }

    let Some(r) = app.calib.result else { return };
    ui.separator();
    if !r.coaxial {
        ui.colored_label(
            Color32::from_rgb(255, 110, 100),
            format!(
                "The focuser ring moved {:.1} px (radius changed {:.1} px) between the captures: the adapter \
                 didn't rotate about the focuser axis (e.g. a thumbscrew pushes it sideways). Re-seat it, \
                 tighten the same way both times, and capture again.",
                r.rim_shift, r.rim_radius_change
            ),
        );
    }
    ui.label(format!("Camera bias:  {:+.2}, {:+.2} px", r.bias.x, r.bias.y));
    ui.label(format!(
        "Real misalignment at 0°:  {:+.2}, {:+.2} px  ({:.2} % of ring)",
        r.misalignment.x,
        r.misalignment.y,
        r.misalignment_percent()
    ));
    ui.horizontal(|ui| {
        if ui
            .add_enabled(r.coaxial, egui::Button::new(RichText::new("Apply bias as layer offset").strong()))
            .on_hover_text("The circle moves to where a perfectly aligned feature appears through this camera")
            .clicked()
            && let Some(t) = app.calib.target
            && let Some(l) = app.s.overlay.layer_mut(t)
        {
            l.offset = r.bias;
            let name = l.name.clone();
            app.info(format!("Offset applied to '{name}'"));
        }
        if ui.button("Start over").clicked() {
            app.calib.samples = [None, None];
            app.calib.result = None;
        }
    });
}

/// Controls for the synthetic camera, to try the tools without hardware.
pub fn simulator_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    let mut s = app.s.scene;
    let vec = |ui: &mut Ui, label: &str, v: &mut [f32; 2]| {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.add(DragValue::new(&mut v[0]).speed(0.1).range(-60.0..=60.0).prefix("x "));
            ui.add(DragValue::new(&mut v[1]).speed(0.1).range(-60.0..=60.0).prefix("y "));
        });
    };
    vec(ui, "Axis offset", &mut s.axis_offset);
    vec(ui, "Camera parallax", &mut s.parallax);
    vec(ui, "Secondary error", &mut s.secondary_error);
    vec(ui, "Primary error", &mut s.primary_error);
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut s.rotation_deg, 0.0..=359.0).text("adapter rotation °"));
        if ui.button("+180°").clicked() {
            s.rotation_deg = (s.rotation_deg + 180.0) % 360.0;
        }
    });
    ui.add(egui::Slider::new(&mut s.noise, 0.0..=40.0).text("noise"));
    if ui.button("Perfectly collimated").clicked() {
        s.secondary_error = [0.0; 2];
        s.primary_error = [0.0; 2];
    }
    app.set_scene(s);
}
