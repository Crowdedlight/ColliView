use egui::{RichText, Ui};

use crate::app::ColliViewApp;
use crate::camera::{Command, ControlDesc, ControlKind, ControlRole};

pub fn camera_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    if app.current.is_none() {
        ui.label("No camera open.");
        return;
    }
    if app.controls.is_empty() {
        ui.label(if app.is_test_pattern() {
            "The test pattern has no hardware controls."
        } else {
            "This camera exposes no adjustable controls."
        });
        return;
    }
    let mut commands = Vec::new();
    if let Some(focus) = app.controls.iter_mut().find(|c| c.role == ControlRole::Focus) {
        focus_section(focus, ui, &mut commands);
        ui.separator();
    }
    for c in app.controls.iter_mut().filter(|c| c.role != ControlRole::Focus) {
        control_row(c, ui, &mut commands);
    }
    ui.horizontal(|ui| {
        if ui.button("Reset to defaults").clicked() {
            commands.push(Command::ResetControls);
        }
        if ui.button("Re-read").on_hover_text("Refresh values from the camera").clicked() {
            commands.push(Command::RefreshControls);
        }
    });
    for c in commands {
        app.worker.send(c);
    }
}

fn focus_section(c: &mut ControlDesc, ui: &mut Ui, out: &mut Vec<Command>) {
    ui.label(RichText::new("Focus").strong());
    if let Some(auto) = &mut c.auto
        && ui.checkbox(auto, "Autofocus").changed()
    {
        out.push(Command::SetAuto { id: c.id, auto: *auto });
    }
    let ControlKind::Int { min, max, step, .. } = c.kind else {
        control_row(c, ui, out);
        return;
    };
    let set = |c: &mut ControlDesc, v: i64, out: &mut Vec<Command>| {
        let v = v.clamp(min, max);
        c.value = v;
        if let Some(a) = &mut c.auto {
            *a = false;
        }
        out.push(Command::SetControl { id: c.id, value: v });
    };
    let mut v = c.value;
    if ui.add(egui::Slider::new(&mut v, min..=max).step_by(step as f64).text("manual")).changed() {
        set(c, v, out);
    }
    ui.horizontal(|ui| {
        for (label, delta) in [("−10", -10), ("−1", -1), ("+1", 1), ("+10", 10)] {
            if ui.button(label).clicked() {
                set(c, c.value + delta * step, out);
            }
        }
    });
    ui.small("Moving the focus switches autofocus off. Focus on the drawtube/secondary first, then on the primary reflections.");
}

fn control_row(c: &mut ControlDesc, ui: &mut Ui, out: &mut Vec<Command>) {
    ui.add_enabled_ui(!c.inactive || c.auto.is_some(), |ui| {
        ui.horizontal(|ui| {
            ui.label(&c.name);
            if let Some(auto) = &mut c.auto
                && ui.checkbox(auto, "auto").changed()
            {
                out.push(Command::SetAuto { id: c.id, auto: *auto });
            }
        });
        let mut v = c.value;
        let changed = match &c.kind {
            ControlKind::Int { min, max, step, .. } => {
                ui.add(egui::Slider::new(&mut v, *min..=*max).step_by(*step as f64)).changed()
            }
            ControlKind::Bool { .. } => {
                let mut b = v != 0;
                let ch = ui.checkbox(&mut b, "on").changed();
                v = b as i64;
                ch
            }
            ControlKind::Menu { items, .. } => {
                let current = items.iter().find(|i| i.0 == v).map_or_else(|| v.to_string(), |i| i.1.clone());
                let mut changed = false;
                egui::ComboBox::from_id_salt(("ctrl", c.id.0)).selected_text(current).show_ui(ui, |ui| {
                    for (val, label) in items {
                        changed |= ui.selectable_value(&mut v, *val, label).changed();
                    }
                });
                changed
            }
        };
        if changed {
            c.value = v;
            if let Some(a) = &mut c.auto {
                *a = false;
            }
            out.push(Command::SetControl { id: c.id, value: v });
        }
    });
}
