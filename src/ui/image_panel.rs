use egui::Ui;

use crate::app::ColliViewApp;
use crate::processing::Processing;

pub fn image_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    let mut p = app.s.processing;
    ui.add(egui::Slider::new(&mut p.brightness, -0.5..=0.5).text("brightness"));
    ui.add(egui::Slider::new(&mut p.contrast, 0.3..=4.0).logarithmic(true).text("contrast"));
    ui.add(egui::Slider::new(&mut p.gamma, 0.3..=3.0).logarithmic(true).text("gamma"));
    ui.add(egui::Slider::new(&mut p.average_frames, 1..=16).text("average frames"))
        .on_hover_text("Running average to reduce noise in a dim tube (adds lag)");
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.grayscale, "Greyscale");
        ui.checkbox(&mut p.invert, "Invert");
        if ui.button("Reset").clicked() {
            p = Processing::default();
        }
    });
    app.set_processing(p);
    ui.small("Software adjustments; they apply on top of the camera's own controls and also feed auto-detect.");

    ui.separator();
    ui.label("Display orientation (overlays follow)");
    let o = &mut app.s.view.orientation;
    ui.horizontal(|ui| {
        ui.checkbox(&mut o.flip_h, "Flip H");
        ui.checkbox(&mut o.flip_v, "Flip V");
        let mut rot = o.flip_h && o.flip_v;
        if ui.checkbox(&mut rot, "Rotate 180°").changed() {
            o.flip_h = rot;
            o.flip_v = rot;
        }
    });
    ui.checkbox(&mut app.s.view.nearest_when_zoomed, "Sharp pixels when zoomed ≥ 2×");
    ui.checkbox(&mut app.s.show_image_center, "Mark the sensor centre")
        .on_hover_text("Grey ✕ at the image centre: helps centring the camera in the adapter");
    ui.checkbox(&mut app.s.snapshot_overlay, "Snapshots include overlays");
}
