//! Native camera controls (focus, exposure, gain, ...).
//!
//! Each OS models auto/manual differently:
//! - Windows: auto is a flag on the same property (`CameraControl_Flags_Auto`).
//! - Linux: auto is a separate control (e.g. `focus_automatic_continuous` next to
//!   `focus_absolute`).
//!
//! Both are folded into one [`ControlDesc`] with an optional `auto` state. Setting a
//! value always switches that control to manual.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use super::{DeviceInfo, DeviceKind};

/// Opaque, backend-specific control identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlId(pub u64);

/// What a control means, so the UI can give focus/exposure a prominent spot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlRole {
    Focus,
    Exposure,
    Gain,
    WhiteBalance,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
    Int {
        min: i64,
        max: i64,
        step: i64,
        default: i64,
    },
    Bool {
        default: bool,
    },
    // Only V4L2 has menu controls.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Menu {
        items: Vec<(i64, String)>,
        default: i64,
    },
}

impl ControlKind {
    pub fn default_value(&self) -> i64 {
        match self {
            ControlKind::Int { default, .. } | ControlKind::Menu { default, .. } => *default,
            ControlKind::Bool { default } => *default as i64,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlDesc {
    pub id: ControlId,
    pub name: String,
    pub role: ControlRole,
    pub kind: ControlKind,
    pub value: i64,
    /// `Some(is_auto)` if the control has an automatic mode.
    pub auto: Option<bool>,
    /// The device reports the control as currently not applicable.
    pub inactive: bool,
}

pub trait ControlBackend {
    fn list(&mut self) -> Vec<ControlDesc>;
    /// Sets a value and switches the control to manual.
    fn set_value(&mut self, id: ControlId, value: i64) -> anyhow::Result<()>;
    fn set_auto(&mut self, id: ControlId, auto: bool) -> anyhow::Result<()>;

    /// Restore defaults, re-enabling automatic modes where available.
    fn reset(&mut self) -> anyhow::Result<()> {
        for c in self.list() {
            // Some controls refuse their own default (e.g. read-only); keep going.
            if let Err(e) = self.set_value(c.id, c.kind.default_value()) {
                log::debug!("reset {}: {e:#}", c.name);
            }
            if c.auto.is_some() {
                self.set_auto(c.id, true)?;
            }
        }
        Ok(())
    }
}

/// For devices without controls (the test pattern) or when the side channel fails.
pub struct NullControls;

impl ControlBackend for NullControls {
    fn list(&mut self) -> Vec<ControlDesc> {
        Vec::new()
    }

    fn set_value(&mut self, _: ControlId, _: i64) -> anyhow::Result<()> {
        anyhow::bail!("this device has no controls")
    }

    fn set_auto(&mut self, _: ControlId, _: bool) -> anyhow::Result<()> {
        anyhow::bail!("this device has no controls")
    }
}

pub fn open(device: &DeviceInfo) -> anyhow::Result<Box<dyn ControlBackend>> {
    match device.kind {
        DeviceKind::TestPattern => Ok(Box::new(NullControls)),
        DeviceKind::Camera { .. } => open_native(&device.key),
    }
}

#[cfg(windows)]
fn open_native(key: &str) -> anyhow::Result<Box<dyn ControlBackend>> {
    Ok(Box::new(windows::WinControls::open(key)?))
}

#[cfg(target_os = "linux")]
fn open_native(key: &str) -> anyhow::Result<Box<dyn ControlBackend>> {
    Ok(Box::new(linux::V4lControls::open(key)?))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn open_native(_key: &str) -> anyhow::Result<Box<dyn ControlBackend>> {
    Ok(Box::new(NullControls))
}
