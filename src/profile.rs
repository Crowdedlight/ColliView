//! Per-telescope profiles: layers, calibration offsets and image settings as JSON.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::overlay::Overlay;
use crate::processing::Processing;
use crate::view::Orientation;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub app: String,
    pub version: u32,
    pub overlay: Overlay,
    pub processing: Processing,
    pub orientation: Orientation,
}

impl Profile {
    pub const VERSION: u32 = 1;

    pub fn new(overlay: Overlay, processing: Processing, orientation: Orientation) -> Self {
        Self { app: "ColliView".into(), version: Self::VERSION, overlay, processing, orientation }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let p: Self = serde_json::from_str(&text).context("not a ColliView profile")?;
        anyhow::ensure!(p.version <= Self::VERSION, "profile is from a newer ColliView (v{})", p.version);
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_file() {
        let dir = std::env::temp_dir().join(format!("colliview-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.json");
        let mut overlay = Overlay::newtonian_preset(1280, 960);
        overlay.layers[1].offset = egui::vec2(1.5, -2.25);
        let p = Profile::new(
            overlay,
            Processing { gamma: 1.4, ..Default::default() },
            Orientation { flip_h: true, flip_v: false },
        );
        p.save(&path).unwrap();
        assert_eq!(Profile::load(&path).unwrap(), p);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
