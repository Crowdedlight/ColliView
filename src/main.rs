//! ColliView: live USB-camera collimation aid for Newtonian telescopes.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod detect;
mod jig;
mod overlay;
mod processing;
mod profile;
mod snapshot;
mod ui;
mod view;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ColliView")
            .with_icon(app_icon())
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native("ColliView", options, Box::new(|cc| Ok(Box::new(app::ColliViewApp::new(cc)))))
}

/// The window and taskbar icon. The executable's own icon is embedded by `build.rs`.
fn app_icon() -> egui::IconData {
    // Window icons are at most a few dozen pixels, so a 256 px copy is plenty and keeps
    // the 1024 px original from being resized or sent to the window manager at full size.
    let png = image::load_from_memory_with_format(include_bytes!("../assets/colliview.png"), image::ImageFormat::Png)
        .expect("the bundled icon is a valid PNG");
    let icon = png.thumbnail_exact(256, 256).into_rgba8();
    egui::IconData { width: icon.width(), height: icon.height(), rgba: icon.into_raw() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_icon_loads() {
        let icon = super::app_icon();
        assert_eq!((icon.width, icon.height, icon.rgba.len()), (256, 256, 256 * 256 * 4));
    }
}
