use egui::{Color32, DragValue, RichText, Ui};

use super::endless::endless;

use crate::app::ColliViewApp;
use crate::jig::JigGeometry;

const GOOD: Color32 = Color32::from_rgb(140, 220, 140);
const SEARCHING: Color32 = Color32::from_rgb(255, 170, 60);

/// Below this the lens counts as centred (mm).
const CENTRED_MM: f32 = 0.05;

/// The live readout for the printed centring jig.
pub fn jig_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label(
        "Checks that the lens sits on the barrel's axis, with the printed centring jig (part \"jig\" in \
         the adapter model). The two holes line up when the lens is on the line through them, whichever \
         way the camera points, so camera tilt doesn't affect the reading. Stand the jig on a backlight, \
         such as a phone showing white or a light pad, and put the adapter in it.",
    );
    ui.label(
        "Tick Live, loosen the camera board's screws and nudge the board until the lens offset reads about \
         0. Tighten them and check again. Only the camera moves: the rings and the shared centre stay put. \
         If the aperture isn't found, dim the backlight or set the exposure by hand.",
    );
    let mut live = app.jig.live;
    ui.add_enabled(app.frame.is_some() || live, egui::Checkbox::new(&mut live, "Live"))
        .on_hover_text("Finds the wide aperture and the backlit hole several times a second");
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
                let g = app.s.jig;
                let e = g.lens_offset_mm(&near, &far);
                let text = RichText::new(format!("Lens offset:  {:+.2}, {:+.2} mm  ({:.2} mm)", e.x, e.y, e.length()))
                    .strong();
                ui.label(if e.length() <= CENTRED_MM { text.color(GOOD) } else { text });
                if let Some(ic) = app.image_center() {
                    let a = g.axis_point(&near, &far) - ic;
                    let deg = (a.length() / g.focal_px(near.radius).max(1e-6)).atan().to_degrees();
                    ui.label(format!("Axis vs sensor centre:  {:+.1}, {:+.1} px  (≈ {deg:.2}°)", a.x, a.y))
                        .on_hover_text(
                            "Where the two holes meet once the lens is centred: where the barrel's axis lands \
                             on the sensor.\nCamera tilt, or the lens sitting off-centre over the sensor, moves \
                             both holes alike, so the lens offset ignores it and the screws can't fix it.\n\
                             Collimation doesn't need it either: the circles follow the drawtube. If it's large, \
                             check that the carrier sits flat in the head.",
                        );
                }
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
