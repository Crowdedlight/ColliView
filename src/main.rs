//! ColliView: live USB-camera collimation aid for Newtonian telescopes.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod calibrate;
mod camera;
mod detect;
mod overlay;
mod processing;
mod profile;
mod register;
mod snapshot;
// The page layout is only used by examples/make_target.rs and the tests.
#[allow(dead_code)]
mod target;
mod ui;
mod view;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ColliView")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native("ColliView", options, Box::new(|cc| Ok(Box::new(app::ColliViewApp::new(cc)))))
}
