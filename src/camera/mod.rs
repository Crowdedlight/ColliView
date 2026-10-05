//! Camera worker thread: owns capture and controls, publishes processed frames.
//!
//! No nokhwa or OS types leave this module; the UI only sees [`DeviceInfo`],
//! [`VideoMode`], [`ControlDesc`] and [`Frame`].

pub mod controls;
mod nokhwa_capture;
pub mod test_pattern;

use std::fmt;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use egui::ColorImage;
use serde::{Deserialize, Serialize};

use controls::ControlBackend;
pub use controls::{ControlDesc, ControlId, ControlKind, ControlRole};
use test_pattern::Scene;

use crate::processing::{Processing, Processor};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    TestPattern,
    Camera { index: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub kind: DeviceKind,
    pub name: String,
    /// Platform key used to open the control side channel:
    /// the Media Foundation symbolic link on Windows, `/dev/videoN` on Linux.
    pub key: String,
}

impl DeviceInfo {
    pub fn test_pattern() -> Self {
        Self { kind: DeviceKind::TestPattern, name: "Test pattern (simulated Newtonian)".into(), key: String::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PixelFormat {
    Mjpeg,
    Yuyv,
    Nv12,
    Rgb,
    Gray,
    Synthetic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoMode {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: PixelFormat,
}

impl fmt::Display for VideoMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}×{} @ {} fps ({:?})", self.width, self.height, self.fps, self.format)
    }
}

/// An RGB8 frame straight from a capture backend.
pub(crate) struct RgbFrame {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

pub(crate) trait Capture {
    fn mode(&self) -> VideoMode;
    fn modes(&self) -> Vec<VideoMode>;
    fn next_frame(&mut self) -> anyhow::Result<RgbFrame>;
    fn set_scene(&mut self, _scene: &Scene) {}
}

/// A processed frame ready for display.
#[derive(Clone)]
pub struct Frame {
    pub image: Arc<ColorImage>,
    /// Measured frames per second.
    pub fps: f32,
}

pub enum Command {
    ListDevices,
    Open { device: DeviceInfo, mode: Option<VideoMode> },
    SetControl { id: ControlId, value: i64 },
    SetAuto { id: ControlId, auto: bool },
    ResetControls,
    RefreshControls,
    SetProcessing(Processing),
    SetScene(Scene),
    Shutdown,
}

pub enum Event {
    Devices(Vec<DeviceInfo>),
    Opened { device: DeviceInfo, mode: VideoMode, modes: Vec<VideoMode> },
    Controls(Vec<ControlDesc>),
    Closed,
    Error(String),
}

pub struct CameraWorker {
    tx: Sender<Command>,
    rx: Receiver<Event>,
    mailbox: Arc<Mutex<Option<Frame>>>,
    handle: Option<JoinHandle<()>>,
}

impl CameraWorker {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (tx, cmd_rx) = mpsc::channel();
        let (evt_tx, rx) = mpsc::channel();
        let mailbox = Arc::new(Mutex::new(None));
        let mb = mailbox.clone();
        let handle = std::thread::Builder::new()
            .name("camera-worker".into())
            .spawn(move || Worker::new(ctx, evt_tx, mb).run(cmd_rx))
            .expect("failed to spawn camera worker");
        Self { tx, rx, mailbox, handle: Some(handle) }
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn poll_events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }

    /// Latest frame, if a new one arrived since the last call.
    pub fn take_frame(&self) -> Option<Frame> {
        self.mailbox.lock().ok()?.take()
    }
}

impl Drop for CameraWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

struct Session {
    capture: Box<dyn Capture>,
    controls: Box<dyn ControlBackend>,
}

struct Worker {
    ctx: egui::Context,
    events: Sender<Event>,
    mailbox: Arc<Mutex<Option<Frame>>>,
    session: Option<Session>,
    processor: Processor,
    scene: Scene,
    fps: f32,
    last_frame: Option<Instant>,
    consecutive_errors: u32,
}

impl Worker {
    fn new(ctx: egui::Context, events: Sender<Event>, mailbox: Arc<Mutex<Option<Frame>>>) -> Self {
        Self {
            ctx,
            events,
            mailbox,
            session: None,
            processor: Processor::default(),
            scene: Scene::default(),
            fps: 0.0,
            last_frame: None,
            consecutive_errors: 0,
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
        self.ctx.request_repaint();
    }

    fn run(mut self, commands: Receiver<Command>) {
        loop {
            // Handle every pending command before grabbing the next frame. Block
            // while idle so the thread doesn't spin.
            let cmd = if self.session.is_some() {
                match commands.try_recv() {
                    Ok(c) => Some(c),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return,
                }
            } else {
                match commands.recv() {
                    Ok(c) => Some(c),
                    Err(_) => return,
                }
            };
            if let Some(cmd) = cmd {
                if !self.handle(cmd) {
                    return;
                }
                continue;
            }
            self.grab();
        }
    }

    /// Returns false on shutdown.
    fn handle(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::ListDevices => self.emit(Event::Devices(list_devices())),
            Command::Open { device, mode } => self.open(device, mode),
            Command::SetControl { id, value } => self.with_controls(|c| c.set_value(id, value)),
            Command::SetAuto { id, auto } => self.with_controls(|c| c.set_auto(id, auto)),
            Command::ResetControls => self.with_controls(|c| c.reset()),
            Command::RefreshControls => self.with_controls(|_| Ok(())),
            Command::SetProcessing(p) => self.processor.set(p),
            Command::SetScene(s) => {
                self.scene = s;
                if let Some(sess) = &mut self.session {
                    sess.capture.set_scene(&s);
                }
            }
            Command::Shutdown => return false,
        }
        true
    }

    fn open(&mut self, device: DeviceInfo, mode: Option<VideoMode>) {
        // Release the current device first; Windows won't open a camera twice.
        self.session = None;
        if let Ok(mut slot) = self.mailbox.lock() {
            *slot = None;
        }
        let capture: anyhow::Result<Box<dyn Capture>> = match device.kind {
            DeviceKind::TestPattern => Ok(Box::new(test_pattern::TestPatternCapture::new(self.scene, mode))),
            DeviceKind::Camera { index } => {
                nokhwa_capture::NokhwaCapture::open(index, mode).map(|c| Box::new(c) as Box<dyn Capture>)
            }
        };
        let capture = match capture {
            Ok(c) => c,
            Err(e) => {
                self.emit(Event::Error(format!("Could not open {}: {e:#}", device.name)));
                self.emit(Event::Closed);
                return;
            }
        };
        let controls = controls::open(&device).unwrap_or_else(|e| {
            log::warn!("camera controls unavailable: {e:#}");
            Box::new(controls::NullControls)
        });
        let (mode, modes) = (capture.mode(), capture.modes());
        self.session = Some(Session { capture, controls });
        self.last_frame = None;
        self.consecutive_errors = 0;
        self.emit(Event::Opened { device, mode, modes });
        self.with_controls(|_| Ok(()));
    }

    /// Run a control operation, then re-query everything: on Linux a manual control
    /// becomes inactive while its auto partner is on, so values can change together.
    fn with_controls(&mut self, f: impl FnOnce(&mut dyn ControlBackend) -> anyhow::Result<()>) {
        let Some(sess) = &mut self.session else { return };
        let result = f(sess.controls.as_mut());
        let list = sess.controls.list();
        if let Err(e) = result {
            self.emit(Event::Error(format!("Camera control failed: {e:#}")));
        }
        self.emit(Event::Controls(list));
    }

    fn grab(&mut self) {
        let Some(sess) = &mut self.session else { return };
        match sess.capture.next_frame() {
            Ok(f) => {
                self.consecutive_errors = 0;
                let image = self.processor.process(f.width, f.height, &f.data);
                let now = Instant::now();
                if let Some(prev) = self.last_frame {
                    let inst = 1.0 / now.duration_since(prev).as_secs_f32().max(1e-4);
                    self.fps = if self.fps == 0.0 { inst } else { self.fps * 0.9 + inst * 0.1 };
                }
                self.last_frame = Some(now);
                if let Ok(mut slot) = self.mailbox.lock() {
                    *slot = Some(Frame { image: Arc::new(image), fps: self.fps });
                }
                self.ctx.request_repaint();
            }
            Err(e) => {
                self.consecutive_errors += 1;
                log::warn!("frame error: {e:#}");
                if self.consecutive_errors >= 20 {
                    self.session = None;
                    self.emit(Event::Error(format!("Camera stopped delivering frames: {e:#}")));
                    self.emit(Event::Closed);
                }
            }
        }
    }
}

fn list_devices() -> Vec<DeviceInfo> {
    let mut out = vec![DeviceInfo::test_pattern()];
    match nokhwa_capture::query() {
        Ok(cams) => out.extend(cams),
        Err(e) => log::warn!("camera enumeration failed: {e:#}"),
    }
    out
}
