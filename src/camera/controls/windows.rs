//! Windows controls through a second Media Foundation source for the same device.
//!
//! nokhwa keeps its own `IMFMediaSource` private, so we open another one by symbolic
//! link and query `IAMCameraControl` / `IAMVideoProcAmp` on it. UVC property requests
//! work alongside the streaming source (verified with `examples/controls_spike.rs`).

use anyhow::{Context, Result};
use windows::Win32::Media::DirectShow::*;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaSource, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION, MFCreateAttributes, MFCreateDeviceSource,
    MFSTARTUP_NOSOCKET, MFShutdown, MFStartup,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::{HSTRING, Interface};

use super::{ControlBackend, ControlDesc, ControlId, ControlKind, ControlRole};

// Both interfaces share the same flag values.
const FLAG_AUTO: i32 = CameraControl_Flags_Auto.0;
const FLAG_MANUAL: i32 = CameraControl_Flags_Manual.0;

const CAMERA: u64 = 1;
const PROCAMP: u64 = 2;

/// (interface, property, name, role) in display order.
const PROPERTIES: &[(u64, i32, &str, ControlRole)] = &[
    (CAMERA, CameraControl_Focus.0, "Focus", ControlRole::Focus),
    (CAMERA, CameraControl_Exposure.0, "Exposure", ControlRole::Exposure),
    (PROCAMP, VideoProcAmp_Gain.0, "Gain", ControlRole::Gain),
    (PROCAMP, VideoProcAmp_WhiteBalance.0, "White balance", ControlRole::WhiteBalance),
    (PROCAMP, VideoProcAmp_Brightness.0, "Brightness", ControlRole::Other),
    (PROCAMP, VideoProcAmp_Contrast.0, "Contrast", ControlRole::Other),
    (PROCAMP, VideoProcAmp_Gamma.0, "Gamma", ControlRole::Other),
    (PROCAMP, VideoProcAmp_Saturation.0, "Saturation", ControlRole::Other),
    (PROCAMP, VideoProcAmp_Hue.0, "Hue", ControlRole::Other),
    (PROCAMP, VideoProcAmp_Sharpness.0, "Sharpness", ControlRole::Other),
    (PROCAMP, VideoProcAmp_BacklightCompensation.0, "Backlight compensation", ControlRole::Other),
    (PROCAMP, VideoProcAmp_ColorEnable.0, "Colour", ControlRole::Other),
    (CAMERA, CameraControl_Zoom.0, "Zoom", ControlRole::Other),
    (CAMERA, CameraControl_Pan.0, "Pan", ControlRole::Other),
    (CAMERA, CameraControl_Tilt.0, "Tilt", ControlRole::Other),
    (CAMERA, CameraControl_Roll.0, "Roll", ControlRole::Other),
    (CAMERA, CameraControl_Iris.0, "Iris", ControlRole::Other),
];

struct Range {
    min: i32,
    max: i32,
    step: i32,
    default: i32,
    caps: i32,
}

pub struct WinControls {
    source: IMFMediaSource,
    camera: Option<IAMCameraControl>,
    procamp: Option<IAMVideoProcAmp>,
}

impl WinControls {
    pub fn open(symlink: &str) -> Result<Self> {
        unsafe {
            // nokhwa already initialised COM (MTA) and MF on this worker thread; these
            // calls just add references.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)?;
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 2)?;
            let attrs = attrs.context("MFCreateAttributes returned null")?;
            attrs.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
            attrs.SetString(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, &HSTRING::from(symlink))?;
            let source = MFCreateDeviceSource(&attrs).context("opening control side channel")?;
            let camera = source.cast::<IAMCameraControl>().ok();
            let procamp = source.cast::<IAMVideoProcAmp>().ok();
            Ok(Self { source, camera, procamp })
        }
    }

    fn range(&self, iface: u64, prop: i32) -> Option<Range> {
        let mut r = Range { min: 0, max: 0, step: 0, default: 0, caps: 0 };
        let res = unsafe {
            match iface {
                CAMERA => self.camera.as_ref()?.GetRange(
                    prop,
                    &mut r.min,
                    &mut r.max,
                    &mut r.step,
                    &mut r.default,
                    &mut r.caps,
                ),
                _ => self.procamp.as_ref()?.GetRange(
                    prop,
                    &mut r.min,
                    &mut r.max,
                    &mut r.step,
                    &mut r.default,
                    &mut r.caps,
                ),
            }
        };
        res.ok().map(|_| r)
    }

    /// (value, flags)
    fn get(&self, iface: u64, prop: i32) -> Result<(i32, i32)> {
        let (mut v, mut f) = (0, 0);
        unsafe {
            match iface {
                CAMERA => self.camera.as_ref().context("no camera control")?.Get(prop, &mut v, &mut f)?,
                _ => self.procamp.as_ref().context("no video proc amp")?.Get(prop, &mut v, &mut f)?,
            }
        }
        Ok((v, f))
    }

    fn set(&self, iface: u64, prop: i32, value: i32, flags: i32) -> Result<()> {
        unsafe {
            match iface {
                CAMERA => self.camera.as_ref().context("no camera control")?.Set(prop, value, flags)?,
                _ => self.procamp.as_ref().context("no video proc amp")?.Set(prop, value, flags)?,
            }
        }
        Ok(())
    }
}

fn split(id: ControlId) -> (u64, i32) {
    (id.0 >> 32, id.0 as u32 as i32)
}

impl ControlBackend for WinControls {
    fn list(&mut self) -> Vec<ControlDesc> {
        let mut out = Vec::new();
        for &(iface, prop, name, role) in PROPERTIES {
            let Some(r) = self.range(iface, prop) else { continue };
            if r.max <= r.min {
                continue;
            }
            let Ok((value, flags)) = self.get(iface, prop) else { continue };
            let kind = if r.min == 0 && r.max == 1 {
                ControlKind::Bool { default: r.default != 0 }
            } else {
                ControlKind::Int {
                    min: r.min as i64,
                    max: r.max as i64,
                    step: r.step.max(1) as i64,
                    default: r.default as i64,
                }
            };
            let auto = (r.caps & FLAG_AUTO != 0).then_some(flags & FLAG_AUTO != 0);
            out.push(ControlDesc {
                id: ControlId(iface << 32 | prop as u32 as u64),
                name: name.to_owned(),
                role,
                kind,
                value: value as i64,
                auto,
                inactive: false,
            });
        }
        out
    }

    fn set_value(&mut self, id: ControlId, value: i64) -> Result<()> {
        let (iface, prop) = split(id);
        self.set(iface, prop, value as i32, FLAG_MANUAL)
    }

    fn set_auto(&mut self, id: ControlId, auto: bool) -> Result<()> {
        let (iface, prop) = split(id);
        let (value, _) = self.get(iface, prop)?;
        self.set(iface, prop, value, if auto { FLAG_AUTO } else { FLAG_MANUAL })
    }
}

impl Drop for WinControls {
    fn drop(&mut self) {
        self.camera = None;
        self.procamp = None;
        unsafe {
            let _ = self.source.Shutdown();
            let _ = MFShutdown();
        }
    }
}
