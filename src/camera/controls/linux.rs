//! Linux controls via V4L2. The device node is opened a second time next to nokhwa's
//! streaming handle; V4L2 allows any number of opens for control access.
//!
//! Controls are matched by CID, not by name (names changed between kernel versions),
//! and auto/manual pairs are folded into one `ControlDesc`.

use std::collections::HashMap;

use anyhow::{Context, Result};
use v4l::Device;
use v4l::control::{Control, Description, Flags, MenuItem, Type, Value};

use super::{ControlBackend, ControlDesc, ControlId, ControlKind, ControlRole};

const USER_BASE: u32 = 0x0098_0900;
const CAMERA_BASE: u32 = 0x009a_0900;

const BRIGHTNESS: u32 = USER_BASE;
const CONTRAST: u32 = USER_BASE + 1;
const SATURATION: u32 = USER_BASE + 2;
const HUE: u32 = USER_BASE + 3;
const AUTO_WHITE_BALANCE: u32 = USER_BASE + 12;
const GAMMA: u32 = USER_BASE + 16;
const AUTOGAIN: u32 = USER_BASE + 18;
const GAIN: u32 = USER_BASE + 19;
const POWER_LINE_FREQUENCY: u32 = USER_BASE + 24;
const WHITE_BALANCE_TEMPERATURE: u32 = USER_BASE + 26;
const SHARPNESS: u32 = USER_BASE + 27;
const BACKLIGHT_COMPENSATION: u32 = USER_BASE + 28;

const EXPOSURE_AUTO: u32 = CAMERA_BASE + 1;
const EXPOSURE_ABSOLUTE: u32 = CAMERA_BASE + 2;
const FOCUS_ABSOLUTE: u32 = CAMERA_BASE + 10;
const FOCUS_AUTO: u32 = CAMERA_BASE + 12;
const ZOOM_ABSOLUTE: u32 = CAMERA_BASE + 13;

/// V4L2_EXPOSURE_MANUAL / V4L2_EXPOSURE_AUTO / V4L2_EXPOSURE_APERTURE_PRIORITY
const EXPOSURE_MANUAL: i64 = 1;
const EXPOSURE_FULL_AUTO: i64 = 0;
const EXPOSURE_APERTURE_PRIORITY: i64 = 3;

/// value control → its auto partner.
const PAIRS: &[(u32, u32)] = &[
    (FOCUS_ABSOLUTE, FOCUS_AUTO),
    (EXPOSURE_ABSOLUTE, EXPOSURE_AUTO),
    (WHITE_BALANCE_TEMPERATURE, AUTO_WHITE_BALANCE),
    (GAIN, AUTOGAIN),
];

/// Preferred display order and roles; unknown controls follow in driver order.
const ORDER: &[(u32, ControlRole)] = &[
    (FOCUS_ABSOLUTE, ControlRole::Focus),
    (EXPOSURE_ABSOLUTE, ControlRole::Exposure),
    (GAIN, ControlRole::Gain),
    (WHITE_BALANCE_TEMPERATURE, ControlRole::WhiteBalance),
    (BRIGHTNESS, ControlRole::Other),
    (CONTRAST, ControlRole::Other),
    (GAMMA, ControlRole::Other),
    (SATURATION, ControlRole::Other),
    (HUE, ControlRole::Other),
    (SHARPNESS, ControlRole::Other),
    (BACKLIGHT_COMPENSATION, ControlRole::Other),
    (POWER_LINE_FREQUENCY, ControlRole::Other),
    (ZOOM_ABSOLUTE, ControlRole::Other),
];

pub struct V4lControls {
    dev: Device,
    /// Last queried descriptions, by CID.
    descs: HashMap<u32, Description>,
}

impl V4lControls {
    pub fn open(path: &str) -> Result<Self> {
        let dev = Device::with_path(path).with_context(|| format!("opening {path}"))?;
        Ok(Self { dev, descs: HashMap::new() })
    }

    fn read(&self, id: u32) -> Option<i64> {
        match self.dev.control(id).ok()?.value {
            Value::Integer(v) => Some(v),
            Value::Boolean(b) => Some(b as i64),
            _ => None,
        }
    }

    fn write(&self, id: u32, value: i64) -> Result<()> {
        let value = match self.descs.get(&id).map(|d| d.typ) {
            Some(Type::Boolean) => Value::Boolean(value != 0),
            _ => Value::Integer(value),
        };
        self.dev.set_control(Control { id, value }).with_context(|| format!("setting control {id:#x}"))
    }

    fn partner(&self, id: u32) -> Option<u32> {
        PAIRS.iter().find(|p| p.0 == id).map(|p| p.1).filter(|a| self.descs.contains_key(a))
    }

    /// Value that turns the auto partner on or off.
    fn auto_value(&self, auto_id: u32, on: bool) -> i64 {
        if auto_id != EXPOSURE_AUTO {
            return on as i64;
        }
        if !on {
            return EXPOSURE_MANUAL;
        }
        // Prefer aperture priority (auto exposure, fixed iris): most UVC webcams
        // support only that and manual.
        let has = |v: i64| {
            self.descs
                .get(&EXPOSURE_AUTO)
                .and_then(|d| d.items.as_ref())
                .is_some_and(|items| items.iter().any(|(i, _)| *i as i64 == v))
        };
        if has(EXPOSURE_APERTURE_PRIORITY) { EXPOSURE_APERTURE_PRIORITY } else { EXPOSURE_FULL_AUTO }
    }

    fn is_auto(&self, auto_id: u32) -> Option<bool> {
        let v = self.read(auto_id)?;
        Some(if auto_id == EXPOSURE_AUTO { v != EXPOSURE_MANUAL } else { v != 0 })
    }
}

impl ControlBackend for V4lControls {
    fn list(&mut self) -> Vec<ControlDesc> {
        let descs = match self.dev.query_controls() {
            Ok(d) => d,
            Err(e) => {
                log::warn!("VIDIOC_QUERY_EXT_CTRL failed: {e}");
                return Vec::new();
            }
        };
        self.descs = descs.into_iter().map(|d| (d.id, d)).collect();
        let auto_ids: Vec<u32> = PAIRS.iter().filter(|p| self.descs.contains_key(&p.0)).map(|p| p.1).collect();

        let mut ids: Vec<u32> = ORDER.iter().map(|o| o.0).filter(|id| self.descs.contains_key(id)).collect();
        let mut rest: Vec<u32> = self.descs.keys().copied().filter(|id| !ids.contains(id)).collect();
        rest.sort_unstable();
        ids.extend(rest);

        let mut out = Vec::new();
        for id in ids {
            if auto_ids.contains(&id) {
                continue; // folded into its value control
            }
            let d = &self.descs[&id];
            if d.flags.intersects(Flags::DISABLED | Flags::READ_ONLY | Flags::WRITE_ONLY) {
                continue;
            }
            let kind = match d.typ {
                Type::Integer | Type::Integer64 => ControlKind::Int {
                    min: d.minimum,
                    max: d.maximum,
                    step: (d.step as i64).max(1),
                    default: d.default,
                },
                Type::Boolean => ControlKind::Bool { default: d.default != 0 },
                Type::Menu | Type::IntegerMenu => ControlKind::Menu {
                    items: d
                        .items
                        .iter()
                        .flatten()
                        .map(|(i, item)| {
                            let label = match item {
                                MenuItem::Name(n) => n.clone(),
                                MenuItem::Value(v) => v.to_string(),
                            };
                            (*i as i64, label)
                        })
                        .collect(),
                    default: d.default,
                },
                _ => continue,
            };
            let Some(value) = self.read(id) else { continue };
            let auto = self.partner(id).and_then(|a| self.is_auto(a));
            let role = ORDER.iter().find(|o| o.0 == id).map_or(ControlRole::Other, |o| o.1);
            out.push(ControlDesc {
                id: ControlId(id as u64),
                name: d.name.clone(),
                role,
                kind,
                value,
                auto,
                inactive: d.flags.contains(Flags::INACTIVE),
            });
        }
        out
    }

    fn set_value(&mut self, id: ControlId, value: i64) -> Result<()> {
        let id = id.0 as u32;
        // A manual control is inactive while its auto partner is on.
        if let Some(a) = self.partner(id)
            && self.is_auto(a) == Some(true)
        {
            self.write(a, self.auto_value(a, false))?;
        }
        self.write(id, value)
    }

    fn set_auto(&mut self, id: ControlId, auto: bool) -> Result<()> {
        let a = self.partner(id.0 as u32).context("control has no automatic mode")?;
        self.write(a, self.auto_value(a, auto))
    }
}
