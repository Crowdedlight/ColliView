use egui::{Color32, DragValue, RichText, Ui, vec2};

use crate::app::{ColliViewApp, JobKind};
use crate::overlay::LayerKind;

pub fn layers_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    centre_section(app, ui);
    ui.separator();
    detect_section(app, ui);
    ui.separator();
    layer_list(app, ui);
    ui.separator();
    selected_layer(app, ui);
}

fn centre_section(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(RichText::new("Shared centre").strong());
    let c = &mut app.s.overlay.center;
    ui.horizontal(|ui| {
        ui.label("x");
        ui.add(DragValue::new(&mut c.x).speed(0.1).fixed_decimals(1).suffix(" px"));
        ui.label("y");
        ui.add(DragValue::new(&mut c.y).speed(0.1).fixed_decimals(1).suffix(" px"));
    });
    ui.horizontal(|ui| {
        if ui.button("To image centre").clicked()
            && let Some(ic) = app.image_center()
        {
            app.s.overlay.center = ic;
        }
        ui.checkbox(&mut app.s.overlay.show_center_marker, "Show centre cross");
    });
    ui.small("Arrows nudge 1 px · Shift 10 px · Ctrl 0.1 px");
}

fn detect_section(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(RichText::new("Auto-detect focuser ring").strong());
    ui.horizontal(|ui| {
        let busy = app.job.as_ref().is_some_and(|j| j.kind == JobKind::Reference);
        if ui
            .add_enabled(!busy && app.frame.is_some(), egui::Button::new("Auto-detect (D)"))
            .on_hover_text("Fits the reference ring and shared centre to the drawtube edge")
            .clicked()
        {
            app.detect_reference();
        }
        if busy {
            ui.spinner();
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.s.detect_wide, "Wide search")
            .on_hover_text("Search 8–55 % of the frame from the image centre instead of near the current ring");
        ui.add_enabled_ui(!app.s.detect_wide, |ui| {
            let mut pct = app.s.detect_tolerance * 100.0;
            if ui.add(DragValue::new(&mut pct).range(5.0..=60.0).speed(0.5).prefix("radius ±").suffix(" %")).changed()
            {
                app.s.detect_tolerance = pct / 100.0;
            }
        });
    });
    if let Some(d) = app.preview {
        ui.horizontal(|ui| {
            let color = match d.quality() {
                "good" => Color32::from_rgb(120, 220, 120),
                "fair" => Color32::from_rgb(240, 200, 80),
                _ => Color32::from_rgb(255, 110, 100),
            };
            ui.colored_label(color, format!("{} fit", d.quality()));
            ui.label(format!("r {:.1} px · rms {:.2} px · {:.0} % edges", d.radius, d.rms, d.inlier_ratio * 100.0));
        });
        ui.horizontal(|ui| {
            if ui.button("Accept").clicked() {
                app.accept_preview();
            }
            if ui.button("Reject").clicked() {
                app.preview = None;
            }
        });
    }
}

fn layer_list(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(RichText::new("Layers").strong());
    let o = &mut app.s.overlay;
    let reference = o.reference;
    let mut select = None;
    for layer in &mut o.layers {
        ui.horizontal(|ui| {
            ui.checkbox(&mut layer.visible, "").on_hover_text("Show / hide");
            let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, layer.color);
            let mut label = layer.name.clone();
            if layer.id == reference {
                label.push_str("  ★");
            }
            if matches!(layer.kind, LayerKind::Crosshair { .. }) {
                label.push_str("  ✚");
            }
            if layer.offset != egui::Vec2::ZERO {
                label.push_str("  ↗");
            }
            let selected = o.selected == Some(layer.id);
            if ui.selectable_label(selected, label).clicked() {
                select = Some(layer.id);
            }
        });
    }
    if let Some(id) = select {
        o.selected = Some(id);
    }
    ui.small("★ reference (defines the centre) · ↗ has a calibration offset");
    ui.horizontal_wrapped(|ui| {
        if ui.button("+ Circle").clicked() {
            o.add_circle();
        }
        if ui.button("+ Crosshair").clicked() {
            o.add_crosshair();
        }
        let sel = o.selected;
        ui.add_enabled_ui(sel.is_some(), |ui| {
            if ui.button("Duplicate").clicked() {
                o.duplicate(sel.unwrap());
            }
            if ui.add_enabled(sel != Some(reference), egui::Button::new("Delete")).clicked() {
                o.remove(sel.unwrap());
            }
            if ui.button("Up").on_hover_text("Move up (drawn on top)").clicked() {
                o.move_layer(sel.unwrap(), true);
            }
            if ui.button("Down").on_hover_text("Move down").clicked() {
                o.move_layer(sel.unwrap(), false);
            }
        });
    });
}

fn selected_layer(app: &mut ColliViewApp, ui: &mut Ui) {
    let Some(id) = app.s.overlay.selected else {
        ui.label("Select a layer to edit it.");
        return;
    };
    let reference = app.s.overlay.reference;
    let ref_radius = app.s.overlay.reference_radius();
    let measured_error = app.s.overlay.layer(id).and_then(|l| app.s.overlay.measured_error(l));
    let mut measure = false;
    let mut make_reference = false;
    {
        let Some(layer) = app.s.overlay.layer_mut(id) else { return };
        ui.label(RichText::new("Selected layer").strong());
        ui.text_edit_singleline(&mut layer.name);
        ui.horizontal(|ui| {
            ui.label("Colour");
            ui.color_edit_button_srgba(&mut layer.color);
            ui.label("Width");
            ui.add(DragValue::new(&mut layer.thickness).range(0.5..=12.0).speed(0.05).suffix(" px"));
        });
        match &mut layer.kind {
            LayerKind::Circle { radius } => {
                ui.horizontal(|ui| {
                    ui.label("Radius");
                    ui.add(DragValue::new(radius).range(0.5..=10000.0).speed(0.1).fixed_decimals(1).suffix(" px"));
                    if let Some(rr) = ref_radius
                        && id != reference
                    {
                        ui.label(format!("{:.1} % of ring", 100.0 * *radius / rr));
                    }
                });
            }
            LayerKind::Crosshair { angle_deg } => {
                ui.horizontal(|ui| {
                    ui.label("Angle");
                    ui.add(egui::Slider::new(angle_deg, -90.0..=90.0).suffix("°").fixed_decimals(1));
                });
            }
        }
        if id == reference {
            ui.small("This ring defines the shared centre. Auto-detect fits it.");
        } else {
            ui.horizontal(|ui| {
                ui.label("Offset");
                ui.add(DragValue::new(&mut layer.offset.x).speed(0.05).fixed_decimals(2).prefix("x "));
                ui.add(DragValue::new(&mut layer.offset.y).speed(0.05).fixed_decimals(2).prefix("y "));
                if ui.small_button("Zero").clicked() {
                    layer.offset = egui::Vec2::ZERO;
                }
            })
            .response
            .on_hover_text(
                "Offset from the shared centre (raw px). Set by the calibration wizard; Alt+drag on the image",
            );
            if layer.is_circle() {
                ui.horizontal(|ui| {
                    measure = ui
                        .button("Measure")
                        .on_hover_text("Fit this circle's feature in the image (search ±15 % of its radius)")
                        .clicked();
                    make_reference = ui.button("Make reference").clicked();
                });
            }
        }
    }
    if let Some(err) = measured_error {
        let pct = ref_radius.map(|r| 100.0 * err.length() / r).unwrap_or(0.0);
        let color = if pct < 0.5 { Color32::from_rgb(120, 220, 120) } else { Color32::from_rgb(240, 200, 80) };
        ui.colored_label(
            color,
            format!("Measured feature is off by {:+.1}, {:+.1} px ({:.2} % of ring)", err.x, err.y, pct),
        );
        if ui.small_button("Clear measurement").clicked()
            && let Some(l) = app.s.overlay.layer_mut(id)
        {
            l.measured = None;
        }
    }
    if measure {
        app.measure_layer(id);
    }
    if make_reference {
        app.s.overlay.set_reference(id);
    }
}
