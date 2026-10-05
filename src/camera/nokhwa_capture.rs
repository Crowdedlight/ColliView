//! Capture through nokhwa (Media Foundation on Windows, V4L2 on Linux).
//!
//! Camera controls deliberately do *not* go through nokhwa: it cannot switch a
//! control from auto to manual on Windows. See `controls/`.

use anyhow::{Context, Result};
use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType};

use super::{Capture, DeviceInfo, DeviceKind, PixelFormat, RgbFrame, VideoMode};

pub fn query() -> Result<Vec<DeviceInfo>> {
    let cams = nokhwa::query(ApiBackend::Auto)?;
    Ok(cams
        .into_iter()
        .filter_map(|c| {
            let CameraIndex::Index(index) = *c.index() else { return None };
            let key = if cfg!(target_os = "linux") { format!("/dev/video{index}") } else { c.misc() };
            Some(DeviceInfo { kind: DeviceKind::Camera { index }, name: c.human_name(), key })
        })
        .collect())
}

pub struct NokhwaCapture {
    cam: Camera,
    mode: VideoMode,
    modes: Vec<VideoMode>,
}

impl NokhwaCapture {
    pub fn open(index: u32, requested: Option<VideoMode>) -> Result<Self> {
        let mut cam =
            Camera::new(CameraIndex::Index(index), RequestedFormat::new::<RgbFormat>(RequestedFormatType::None))?;
        let formats = cam.compatible_camera_formats().context("listing camera formats")?;
        let mut modes: Vec<VideoMode> = formats.iter().filter_map(to_mode).collect();
        modes.sort_by_key(|m| (std::cmp::Reverse(m.width * m.height), std::cmp::Reverse(m.fps), m.format as u8));
        modes.dedup();
        let mode = requested
            .filter(|r| modes.contains(r))
            .or_else(|| default_mode(&modes))
            .context("camera reports no usable video modes")?;
        let fmt = formats.iter().find(|f| to_mode(f) == Some(mode)).copied().context("selected mode vanished")?;
        cam.set_camera_requset(RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(fmt)))?;
        cam.open_stream()?;
        Ok(Self { cam, mode, modes })
    }
}

impl Capture for NokhwaCapture {
    // Note: nokhwa's MSMF backend misreports the frame rate after opening (it reads the
    // denominator), so we keep the mode we asked for.
    fn mode(&self) -> VideoMode {
        self.mode
    }

    fn modes(&self) -> Vec<VideoMode> {
        self.modes.clone()
    }

    fn next_frame(&mut self) -> Result<RgbFrame> {
        let image = self.cam.frame()?.decode_image::<RgbFormat>()?;
        let (w, h) = (image.width() as usize, image.height() as usize);
        Ok(RgbFrame { width: w, height: h, data: image.into_raw() })
    }
}

impl Drop for NokhwaCapture {
    fn drop(&mut self) {
        let _ = self.cam.stop_stream();
    }
}

fn to_mode(f: &CameraFormat) -> Option<VideoMode> {
    let format = match f.format() {
        FrameFormat::MJPEG => PixelFormat::Mjpeg,
        FrameFormat::YUYV => PixelFormat::Yuyv,
        FrameFormat::NV12 => PixelFormat::Nv12,
        FrameFormat::GRAY => PixelFormat::Gray,
        FrameFormat::RAWRGB | FrameFormat::RAWBGR => PixelFormat::Rgb,
    };
    (f.frame_rate() > 0).then(|| VideoMode { width: f.width(), height: f.height(), fps: f.frame_rate(), format })
}

/// Largest resolution that still runs at ≥ 15 fps (MJPEG preferred, it's what
/// makes high resolutions fast over USB 2), falling back to anything.
fn default_mode(modes: &[VideoMode]) -> Option<VideoMode> {
    let score = |m: &VideoMode| (m.width * m.height, m.format == PixelFormat::Mjpeg, m.fps.min(30));
    modes
        .iter()
        .filter(|m| m.fps >= 15)
        .max_by_key(|m| score(m))
        .or_else(|| modes.iter().max_by_key(|m| score(m)))
        .copied()
}
