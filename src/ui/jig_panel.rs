use egui::{Color32, DragValue, RichText, Ui};

use super::endless::endless;

use crate::app::{ColliViewApp, JobKind};
use crate::jig::JigGeometry;

const GOOD: Color32 = Color32::from_rgb(140, 220, 140);
const SEARCHING: Color32 = Color32::from_rgb(255, 170, 60);

/// Below this the lens counts as centred (mm).
const CENTRED_MM: f32 = 0.05;

/// The live readout for the printed centring jig.
pub fn jig_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(
        "Checks that the lens sits on the barrel's axis, with the printed centring jig (part \"jig\" in \
         the adapter model). Stand the jig on a backlight, such as a phone showing white or a light pad, \
         and put the adapter in it.",
    );
    ui.label(
        "1. Find the aperture and Accept the preview (left panel): the yellow ring goes on the jig's wide \
         aperture. If its edge is too dark, dim the backlight or set the exposure by hand.\n\
         2. Tick Live, loosen the camera board's screws and nudge the board until the offset reads about \
         0. Tighten them and check again.",
    );
    let busy = app.job.as_ref().is_some_and(|j| j.kind == JobKind::Reference);
    if ui
        .add_enabled(!busy && app.frame.is_some(), egui::Button::new("Find aperture"))
        .on_hover_text("Searches the whole view for the aperture's edge (dark inside, bright outside)")
        .clicked()
    {
        app.jig.stop();
        app.find_jig_aperture();
    }
    let mut live = app.jig.live;
    let ready = app.frame.is_some() && app.s.overlay.reference_radius().is_some();
    ui.add_enabled(ready || live, egui::Checkbox::new(&mut live, "Live"))
        .on_hover_text("Fits the aperture and the hole several times a second");
    if live != app.jig.live {
        if live {
            app.jig.live = true;
        } else {
            app.jig.stop();
        }
    }
    if app.jig.live {
        match app.jig.fits {
            Some((near, far)) => {
                let e = app.s.jig.lens_offset_mm(&near, &far);
                let text = RichText::new(format!("Lens offset:  {:+.2}, {:+.2} mm  ({:.2} mm)", e.x, e.y, e.length()))
                    .strong();
                ui.label(if e.length() <= CENTRED_MM { text.color(GOOD) } else { text });
                ui.label(format!("Aperture fit {}, hole fit {}", near.quality(), far.quality()));
                ui.small(
                    "The arrow in the view shows which way to push the board; check it with a small nudge, as \
                     some cameras mirror the image. Under 0.1 mm is plenty for f/4.5 and slower: the primary \
                     ends up off by about half the lens offset. Turn the adapter 180° in the jig and check \
                     again: the average of the two readings removes a hole printed off-centre. Seat the \
                     adapter the same way each time; play in the socket doesn't average out.",
                );
            }
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Measuring…");
                });
            }
        }
        if app.jig.misses > 0 {
            ui.colored_label(
                SEARCHING,
                format!("Searching… ({} misses): keep both holes in view and lit from behind", app.jig.misses),
            );
        }
    }
    egui::CollapsingHeader::new("Jig dimensions").default_open(false).show(ui, |ui| {
        let g = &mut app.s.jig;
        let mm = |ui: &mut Ui, label: &str, v: &mut f32| {
            ui.horizontal(|ui| {
                ui.label(label);
                endless(ui.add(DragValue::new(v).range(1.0..=500.0).speed(0.1).fixed_decimals(1).suffix(" mm")));
            });
        };
        mm(ui, "Aperture Ø", &mut g.near_d);
        mm(ui, "Aperture from the lens", &mut g.near_dist);
        mm(ui, "Hole Ø", &mut g.far_d);
        mm(ui, "Hole from the lens", &mut g.far_dist);
        ui.small(
            "The adapter model prints them in its console (\"Jig: …\"). They only scale the reading: zero \
             stays zero.",
        );
        if ui.button("Defaults").clicked() {
            *g = JigGeometry::default();
        }
    });
}

/// Controls for the synthetic camera, to try the tools without hardware.
pub fn simulator_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    let mut s = app.s.scene;
    let vec = |ui: &mut Ui, label: &str, v: &mut [f32; 2], range: f32, speed: f32| {
        ui.horizontal(|ui| {
            ui.label(label);
            endless(ui.add(DragValue::new(&mut v[0]).speed(speed).range(-range..=range).prefix("x ")));
            endless(ui.add(DragValue::new(&mut v[1]).speed(speed).range(-range..=range).prefix("y ")));
        });
    };
    ui.checkbox(&mut s.jig, "Show the centring jig instead of the telescope");
    if s.jig {
        vec(ui, "Lens offset (mm)", &mut s.jig_lens_offset_mm, 1.0, 0.005);
        vec(ui, "Sensor offset / tilt (px)", &mut s.axis_offset, 60.0, 0.1);
    } else {
        vec(ui, "Axis offset", &mut s.axis_offset, 60.0, 0.1);
        vec(ui, "Camera parallax", &mut s.parallax, 60.0, 0.1);
        vec(ui, "Secondary error", &mut s.secondary_error, 60.0, 0.1);
        vec(ui, "Primary error", &mut s.primary_error, 60.0, 0.1);
        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut s.rotation_deg, 0.0..=359.0)
                    .step_by(1.0)
                    .smart_aim(false)
                    .text("adapter rotation °"),
            );
            if ui.button("+180°").clicked() {
                s.rotation_deg = (s.rotation_deg + 180.0) % 360.0;
            }
        });
        if ui.button("Perfectly collimated").clicked() {
            s.secondary_error = [0.0; 2];
            s.primary_error = [0.0; 2];
        }
    }
    ui.add(egui::Slider::new(&mut s.noise, 0.0..=40.0).text("noise"));
    app.set_scene(s);
}
