use egui::{Color32, DragValue, RichText, Ui};

use crate::app::{CalibMode, ColliViewApp, JobKind};
use crate::calibrate::{CENTRED_TOLERANCE, match_quality};
use crate::camera::test_pattern::PaperKind;

const GOOD: Color32 = Color32::from_rgb(140, 220, 140);
const SEARCHING: Color32 = Color32::from_rgb(255, 170, 60);

pub fn calibration_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let mut mode = app.calib.mode;
        ui.selectable_value(&mut mode, CalibMode::Paper, "Paper target")
            .on_hover_text("Centre the camera on the focuser axis with a sheet of squares in the tube");
        ui.selectable_value(&mut mode, CalibMode::Layer, "Layer circle")
            .on_hover_text("Separate a layer's camera bias from the telescope's real misalignment, e.g. the secondary");
        if mode != app.calib.mode {
            // The screws may have moved the camera during Live: those captures no longer apply.
            if app.calib.live {
                app.calib.reset();
            }
            if app.calib.paper.live {
                app.calib.paper.reset();
            }
            app.calib.mode = mode;
        }
    });
    ui.separator();
    match app.calib.mode {
        CalibMode::Paper => paper_panel(app, ui),
        CalibMode::Layer => layer_panel(app, ui),
    }
}

fn paper_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(
        "Centres the lens on the focuser axis with the carrier screws. Tape a sheet with squares flat \
         inside the tube, in front of the focuser and as far down as you can, and light it. Plain squared \
         paper needs a few bold, irregular pen marks; or print calibration/centring_target.pdf from the \
         repository. Don't touch the paper between the captures.",
    );
    ui.label(
        "Only the paper is measured, against the sensor centre (○ in the view). This assumes the camera \
         looks straight down the focuser; Layer circle can show what is left. The \
         yellow ring only masks out the drawtube: fitting it roughly is enough.",
    );
    let busy = app.job.as_ref().is_some_and(|j| matches!(j.kind, JobKind::PaperCapture { .. }));
    ui.label(
        "Capture at 0°, rotate the adapter 180° in the focuser, then capture again. With frame averaging \
         on, wait a moment after turning.",
    );
    let steps = ["1. Capture at 0°", "2. Capture at 180°"];
    for (i, label) in steps.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let enabled = !busy
                && app.frame.is_some()
                && app.s.overlay.reference_radius().is_some()
                && (i == 0 || app.calib.paper.has_0deg_capture());
            if ui.add_enabled(enabled, egui::Button::new(*label)).clicked() {
                app.paper_capture(i);
            }
            if app.calib.paper.captured[i] {
                ui.label("✔");
            }
        });
    }
    if busy {
        ui.spinner();
    }

    let Some(r) = app.calib.paper.result else { return };
    ui.separator();
    ui.label(format!(
        "Turned {:.1}°, paper match {:.2} ({})",
        r.rotation_deg,
        r.match_score,
        match_quality(r.match_score)
    ));
    ui.label(format!("Camera bias:  {:+.2}, {:+.2} px  ({:.2} % of ring)", r.bias.x, r.bias.y, r.percent(r.bias)))
        .on_hover_text(
            "Where the focuser axis meets the paper (✕ in the view), relative to the sensor centre (○). \
             It is 0 when the lens sits on the axis.",
        );
    if ui.button("Start over").clicked() {
        app.calib.paper.reset();
        return;
    }

    ui.separator();
    let mut live = app.calib.paper.live;
    ui.checkbox(&mut live, "Live: centre the camera with the carrier screws")
        .on_hover_text("Measures the camera bias continuously. Leave the adapter as it is after the 180° capture.");
    if live && !app.calib.paper.live {
        app.calib.paper.start_live();
    } else if !live && app.calib.paper.live {
        // The screws may have moved the camera: the captures no longer apply.
        app.calib.paper.reset();
    }
    let p = &app.calib.paper;
    if !p.live {
        return;
    }
    ui.label(
        "Leave the adapter and the paper as they are and turn the carrier screws until this reads about 0 \
         (the ✕ sits on the ○). Then untick Live, remove the paper and Auto-detect (D) the drawtube again: \
         the screws have moved it.",
    );
    match (p.live_bias, p.live_reg) {
        (Some(b), Some(reg)) => {
            let text = RichText::new(format!(
                "Camera bias now:  {:+.2}, {:+.2} px  ({:.2} % of ring)",
                b.x,
                b.y,
                r.percent(b)
            ))
            .strong();
            let centred = b.length() <= CENTRED_TOLERANCE * r.ring_radius;
            ui.label(if centred { text.color(GOOD) } else { text });
            ui.label(format!("Paper match {:.2} ({})", reg.score, match_quality(reg.score)));
        }
        _ => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Measuring…");
            });
        }
    }
    if p.live_failures > 0 {
        ui.colored_label(SEARCHING, format!("Searching… ({} misses): keep the paper in view", p.live_failures));
    }
}

fn layer_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(
        "If the camera isn't exactly on the focuser axis, features further down the tube look \
         shifted. Measuring at 0° and 180° adapter rotation separates that camera bias from real \
         misalignment. With a floating carrier, remove the bias with the carrier screws (Live).",
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
            app.calib.reset();
        }
    });

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
    if ui.button("Start over").clicked() {
        app.calib.reset();
        return;
    }

    // Live bias, for centring the camera with the carrier screws.
    ui.separator();
    let mut live = app.calib.live;
    ui.add_enabled(r.coaxial, egui::Checkbox::new(&mut live, "Live: centre the camera with the carrier screws"))
        .on_hover_text("Measures the camera bias continuously. Leave the adapter as it is after the 180° capture.")
        .on_disabled_hover_text("Needs a calibration without the rim moving");
    if live && !app.calib.live {
        app.calib.live = true;
    } else if !live && app.calib.live {
        // The screws may have moved the camera: the captures no longer apply.
        app.calib.reset();
    }
    if !app.calib.live {
        return;
    }
    ui.label(
        "Leave the adapter as it is and turn the carrier screws until this reads about 0: the lens is \
         then on the focuser axis. Then untick Live and Auto-detect (D) again; the old captures no \
         longer apply.",
    );
    match app.calib.live_bias {
        Some(b) => {
            let text = RichText::new(format!(
                "Camera bias now:  {:+.2}, {:+.2} px  ({:.2} % of ring)",
                b.x,
                b.y,
                100.0 * b.length() / r.rim_radius.max(1e-3)
            ))
            .strong();
            let centred = b.length() <= CENTRED_TOLERANCE * r.rim_radius;
            ui.label(if centred { text.color(Color32::from_rgb(140, 220, 140)) } else { text });
        }
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Measuring…");
            });
        }
    }
    if app.calib.live_failures > 0 {
        ui.colored_label(
            Color32::from_rgb(255, 170, 60),
            format!("Searching… ({} misses): keep the rim and the layer's feature in view", app.calib.live_failures),
        );
    }
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
    ui.horizontal(|ui| {
        ui.label("Paper");
        egui::ComboBox::from_id_salt("sim_paper").selected_text(s.paper.label()).show_ui(ui, |ui| {
            for k in PaperKind::ALL {
                ui.selectable_value(&mut s.paper, k, k.label());
            }
        });
    });
    if s.paper != PaperKind::Off {
        ui.add(egui::Slider::new(&mut s.paper_depth, 0.5..=2.0).text("paper depth"));
        vec(ui, "Paper position (mm)", &mut s.paper_offset);
    }
    if ui.button("Perfectly collimated").clicked() {
        s.secondary_error = [0.0; 2];
        s.primary_error = [0.0; 2];
    }
    app.set_scene(s);
}
