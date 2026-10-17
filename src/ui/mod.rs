//! Panels around the live view.

mod calibration_panel;
mod camera_panel;
mod image_panel;
mod layers_panel;

use egui::{Context, RichText, Ui};

pub use layers_panel::layers_panel;

use crate::app::ColliViewApp;
use crate::camera::Command;
use crate::profile::Profile;

pub fn top_bar(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.strong("ColliView");
        ui.separator();

        let current_name = app.current.as_ref().map_or("— no camera —".to_owned(), |(d, _)| d.name.clone());
        let mut open = None;
        egui::ComboBox::from_id_salt("device").width(230.0).selected_text(current_name).show_ui(ui, |ui| {
            for d in &app.devices {
                let selected = app.current.as_ref().is_some_and(|(c, _)| c == d);
                if ui.selectable_label(selected, &d.name).clicked() && !selected {
                    open = Some(d.clone());
                }
            }
        });
        if ui.button("⟳").on_hover_text("Rescan cameras").clicked() {
            app.worker.send(Command::ListDevices);
        }
        if let Some(d) = open {
            app.open_device(d, None);
        }

        if let Some((dev, mode)) = app.current.clone() {
            let mut pick = None;
            egui::ComboBox::from_id_salt("mode").width(210.0).selected_text(mode.to_string()).show_ui(ui, |ui| {
                for m in &app.modes {
                    if ui.selectable_label(*m == mode, m.to_string()).clicked() && *m != mode {
                        pick = Some(*m);
                    }
                }
            });
            if let Some(m) = pick {
                app.open_device(dev, Some(m));
            }
        }
        ui.separator();

        ui.toggle_value(&mut app.frozen, "Freeze").on_hover_text("Hold the current frame (F)");
        if ui.button("Snapshot").on_hover_text("Save a PNG at sensor resolution").clicked() {
            app.save_snapshot();
        }
        ui.separator();

        ui.label("Zoom:");
        if ui.button("Fit").on_hover_text("0").clicked() {
            app.zoom_preset(None);
        }
        if ui.button("1:1").on_hover_text("One screen pixel per sensor pixel").clicked() {
            app.zoom_one_to_one();
        }
        for z in [2.0, 4.0, 8.0] {
            if ui.button(format!("{z}×")).clicked() {
                app.zoom_preset(Some(z));
            }
        }
        if ui.button("On centre").on_hover_text("Zoom 4× on the shared centre").clicked() {
            app.s.view.zoom_to(4.0, app.s.overlay.center);
        }
        ui.separator();
        ui.toggle_value(&mut app.s.show_panels, "Panels").on_hover_text("Show/hide side panels (H)");
        if ui.button("Help").clicked() {
            app.show_help = !app.show_help;
        }
    });
}

pub fn tools_panel(app: &mut ColliViewApp, ui: &mut Ui) {
    egui::CollapsingHeader::new(RichText::new("Camera").strong())
        .default_open(true)
        .show(ui, |ui| camera_panel::camera_panel(app, ui));
    egui::CollapsingHeader::new(RichText::new("Image & view").strong())
        .default_open(false)
        .show(ui, |ui| image_panel::image_panel(app, ui));
    egui::CollapsingHeader::new(RichText::new("Off-centre camera calibration").strong())
        .default_open(false)
        .show(ui, |ui| calibration_panel::calibration_panel(app, ui));
    if app.is_test_pattern() {
        egui::CollapsingHeader::new(RichText::new("Simulator").strong())
            .default_open(true)
            .show(ui, |ui| calibration_panel::simulator_panel(app, ui));
    }
    egui::CollapsingHeader::new(RichText::new("Profiles").strong())
        .default_open(false)
        .show(ui, |ui| profiles(app, ui));
}

fn profiles(app: &mut ColliViewApp, ui: &mut Ui) {
    ui.label("Save the layers, calibration offsets and image settings per telescope.");
    ui.horizontal(|ui| {
        if ui.button("Export…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_file_name("telescope.colliview.json")
                .add_filter("ColliView profile", &["json"])
                .save_file()
        {
            let p = Profile::new(app.s.overlay.clone(), app.s.processing, app.s.view.orientation);
            match p.save(&path) {
                Ok(()) => app.info(format!("Profile saved to {}", path.display())),
                Err(e) => app.error(format!("{e:#}")),
            }
        }
        if ui.button("Import…").clicked()
            && let Some(path) = rfd::FileDialog::new().add_filter("ColliView profile", &["json"]).pick_file()
        {
            match Profile::load(&path) {
                Ok(p) => {
                    app.s.overlay = p.overlay;
                    app.s.view.orientation = p.orientation;
                    app.set_processing(p.processing);
                    if let Some(size) = app.image_size() {
                        app.s.overlay.adapt_to_resolution(size.x as u32, size.y as u32);
                    }
                    app.info(format!("Loaded {}", path.display()));
                }
                Err(e) => app.error(format!("{e:#}")),
            }
        }
    });
    if ui.button("Reset layers to Newtonian preset").clicked()
        && let Some(size) = app.image_size()
    {
        app.s.overlay = crate::overlay::Overlay::newtonian_preset(size.x as u32, size.y as u32);
    }
}

pub fn help_window(app: &mut ColliViewApp, ctx: &Context) {
    egui::Window::new("ColliView help").open(&mut app.show_help).default_width(460.0).show(ctx, |ui| {
        ui.heading("Workflow");
        ui.label(
            "1. Put the camera adapter in the focuser and light the tube.\n\
             2. Auto-detect (D) fits the yellow focuser ring and the shared centre to the drawtube's far edge. \
             Check the dashed preview and Accept.\n\
             3. Secondary: tilt/rotate the secondary until its outline sits concentric in the green circle.\n\
             4. Primary: adjust the primary until its reflection, the clips and the centre spot line up with \
             the red, magenta and cyan circles.\n\
             5. If your camera isn't perfectly centred, run 'Off-centre camera calibration' on the secondary \
             and use Live to centre it with the adapter's carrier screws. Then calibrate once per layer.",
        );
        ui.heading("Mouse");
        ui.label(
            "Wheel: zoom around the cursor   ·   Drag empty space / right / middle: pan\n\
             Drag the white centre cross: move all circles   ·   Double-click: set centre\n\
             Drag the selected circle's edge: resize   ·   Alt+drag: move the selected layer's offset",
        );
        ui.heading("Keys");
        ui.label(
            "Arrows: nudge centre 1 px (Shift 10 px, Ctrl 0.1 px)\n\
             [ / ]: shrink/grow selected circle (same modifiers)\n\
             + / −: zoom   ·   0: fit   ·   D: auto-detect   ·   F: freeze\n\
             H: hide panels   ·   F11: full screen",
        );
    });
}
