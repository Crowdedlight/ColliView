//! Milestone-0 spike: can we set manual focus through a second Media Foundation
//! source while nokhwa is streaming from the same camera?
//!
//! Usage: `cargo run --example controls_spike -- [camera-index]`
//! Writes `spike_focus_a.png` / `spike_focus_b.png` into the scratch dir given by
//! `SPIKE_OUT` (default: current directory) and prints a sharpness metric per frame.

#[cfg(not(windows))]
fn main() {
    eprintln!("The controls spike is Windows-only (Linux uses v4l, which allows concurrent opens).");
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    win::run()
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result};
    use nokhwa::Camera;
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{
        ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
    };
    use windows::Win32::Media::DirectShow::{
        CameraControl_Exposure, CameraControl_Flags_Auto, CameraControl_Flags_Manual, CameraControl_Focus,
        IAMCameraControl,
    };
    use windows::Win32::Media::MediaFoundation::{
        IMFAttributes, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION, MFCreateAttributes, MFCreateDeviceSource,
        MFSTARTUP_NOSOCKET, MFStartup,
    };
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    use windows::core::{HSTRING, Interface};

    pub fn run() -> Result<()> {
        let index: u32 = std::env::args().nth(1).map(|s| s.parse()).transpose()?.unwrap_or(0);
        let out_dir = std::env::var_os("SPIKE_OUT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

        let cams = nokhwa::query(ApiBackend::Auto)?;
        for c in &cams {
            println!("camera {}: {} | misc={}", c.index(), c.human_name(), c.misc());
        }
        let info = cams.iter().find(|c| c.index() == &CameraIndex::Index(index)).context("camera index not found")?;
        let symlink = info.misc();

        let want = CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30);
        let mut cam = Camera::new(
            CameraIndex::Index(index),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(want)),
        )?;
        cam.open_stream()?;
        println!("streaming {}", cam.camera_format());
        drain(&mut cam, Duration::from_millis(500))?;

        // Side channel: a second media source for the same device.
        let ctrl = unsafe { open_camera_control(&symlink)? };
        let focus = unsafe { range(&ctrl, CameraControl_Focus.0) };
        let exposure = unsafe { range(&ctrl, CameraControl_Exposure.0) };
        println!("focus range    {focus:?}");
        println!("exposure range {exposure:?}");
        let (fmin, fmax, ..) = focus.context("camera exposes no focus control")?;

        let mut results = Vec::new();
        for (label, value) in [("a", fmin), ("b", fmax.min(fmin + (fmax - fmin) / 2))] {
            unsafe { ctrl.Set(CameraControl_Focus.0, value, CameraControl_Flags_Manual.0)? };
            drain(&mut cam, Duration::from_millis(1500))?;
            let (mut v, mut f) = (0, 0);
            unsafe { ctrl.Get(CameraControl_Focus.0, &mut v, &mut f)? };
            let img = cam.frame()?.decode_image::<RgbFormat>()?;
            let sharp = sharpness(&img);
            let path = out_dir.join(format!("spike_focus_{label}.png"));
            img.save(&path)?;
            println!("set focus={value} -> read back {v} flags={f}  sharpness={sharp:.2}  {path:?}");
            results.push(sharp);
        }
        unsafe { ctrl.Set(CameraControl_Focus.0, fmin, CameraControl_Flags_Auto.0)? };
        println!("restored auto focus");

        let (a, b) = (results[0], results[1]);
        let changed = (a - b).abs() / a.max(b) > 0.1;
        println!(
            "RESULT: {}",
            if changed { "PASS (image changed with focus)" } else { "INCONCLUSIVE (compare the PNGs)" }
        );
        Ok(())
    }

    fn drain(cam: &mut Camera, d: Duration) -> Result<()> {
        let t = Instant::now();
        while t.elapsed() < d {
            cam.frame()?;
        }
        Ok(())
    }

    unsafe fn open_camera_control(symlink: &str) -> Result<IAMCameraControl> {
        unsafe {
            // nokhwa already initialised COM/MF on this thread; repeating is harmless.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)?;
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 2)?;
            let attrs = attrs.context("MFCreateAttributes returned null")?;
            attrs.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
            attrs.SetString(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, &HSTRING::from(symlink))?;
            let source = MFCreateDeviceSource(&attrs)?;
            Ok(source.cast::<IAMCameraControl>()?)
        }
    }

    /// (min, max, step, default, caps)
    unsafe fn range(ctrl: &IAMCameraControl, prop: i32) -> Option<(i32, i32, i32, i32, i32)> {
        let (mut min, mut max, mut step, mut def, mut caps) = (0, 0, 0, 0, 0);
        unsafe { ctrl.GetRange(prop, &mut min, &mut max, &mut step, &mut def, &mut caps) }
            .ok()
            .map(|_| (min, max, step, def, caps))
    }

    /// Variance of a 4-neighbour Laplacian on the green channel.
    fn sharpness(img: &image::RgbImage) -> f64 {
        let (w, h) = img.dimensions();
        let g = |x: u32, y: u32| img.get_pixel(x, y)[1] as f64;
        let mut vals = Vec::with_capacity((w * h) as usize);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                vals.push(4.0 * g(x, y) - g(x - 1, y) - g(x + 1, y) - g(x, y - 1) - g(x, y + 1));
            }
        }
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64
    }
}
