use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use egui::{
    Align2, Color32, ColorImage, CursorIcon, FontId, Key, Mesh, PointerButton, Pos2, Rect, Response, Sense, Shape,
    Stroke, TextureHandle, TextureOptions, Ui, Vec2, vec2,
};
use serde::{Deserialize, Serialize};

use crate::calibrate::{CalibrationResult, RotationSample};
use crate::camera::test_pattern::Scene;
use crate::camera::{CameraWorker, Command, ControlDesc, DeviceInfo, DeviceKind, Event, Frame, VideoMode};
use crate::detect::{DetectParams, Detection, GrayImage, detect_circle};
use crate::overlay::draw::{paint_dashed_circle, paint_overlay};
use crate::overlay::{LayerId, Measurement, Overlay, Rescale};
use crate::processing::Processing;
use crate::ui;
use crate::view::{ViewState, ViewTransform};

/// Everything that survives a restart (stored by eframe).
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub overlay: Overlay,
    pub view: ViewState,
    pub processing: Processing,
    pub scene: Scene,
    pub device: Option<DeviceInfo>,
    pub mode: Option<VideoMode>,
    /// Relative radius tolerance for auto-detect (0.25 = ±25 %).
    pub detect_tolerance: f32,
    pub detect_wide: bool,
    pub show_image_center: bool,
    pub show_panels: bool,
    pub snapshot_overlay: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            overlay: Overlay::newtonian_preset(1280, 960),
            view: ViewState::default(),
            processing: Processing::default(),
            scene: Scene::default(),
            device: None,
            mode: None,
            detect_tolerance: 0.25,
            detect_wide: false,
            show_image_center: false,
            show_panels: true,
            snapshot_overlay: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DragMode {
    Pan,
    /// Move the shared centre; `grab` keeps the offset from where it was grabbed.
    Center {
        grab: Vec2,
    },
    Radius(LayerId),
    LayerOffset {
        id: LayerId,
        grab: Vec2,
    },
}

/// What a background detection job is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JobKind {
    /// Fit the reference ring; result is shown as a preview to accept/reject.
    Reference,
    /// Fit another layer's feature; stored as that layer's measurement.
    Measure(LayerId),
    /// Rotation calibration capture (0 = 0°, 1 = 180°): rim + target.
    Calibration { step: usize, target: LayerId },
    /// Live camera-bias readout after a calibration, while the carrier screws turn: rim + target.
    LiveBias { target: LayerId },
}

/// Time between live bias measurements.
const LIVE_INTERVAL: Duration = Duration::from_millis(250);

pub struct Job {
    pub kind: JobKind,
    rx: Receiver<Vec<Option<Detection>>>,
}

#[derive(Default)]
pub struct CalibrationState {
    pub target: Option<LayerId>,
    /// Use the current circle positions instead of auto-detect.
    pub manual: bool,
    pub samples: [Option<RotationSample>; 2],
    pub result: Option<CalibrationResult>,
    /// Measure the camera bias live, for centring the camera with the carrier screws.
    pub live: bool,
    pub live_bias: Option<Vec2>,
    /// Live measurements that found no rim or target since the last good one.
    pub live_failures: u32,
    live_next: Option<Instant>,
}

impl CalibrationState {
    /// Forget the captures and stop the live readout; keeps the layer and the mode.
    pub fn reset(&mut self) {
        self.samples = [None, None];
        self.result = None;
        self.stop_live();
    }

    pub fn stop_live(&mut self) {
        self.live = false;
        self.live_bias = None;
        self.live_failures = 0;
    }
}

pub struct Message {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

pub struct ColliViewApp {
    pub s: Settings,
    ctx: egui::Context,
    pub worker: CameraWorker,
    pub devices: Vec<DeviceInfo>,
    pub current: Option<(DeviceInfo, VideoMode)>,
    pub modes: Vec<VideoMode>,
    pub controls: Vec<ControlDesc>,
    texture: Option<TextureHandle>,
    texture_opts: TextureOptions,
    pub frame: Option<Frame>,
    pub frozen: bool,
    drag: Option<DragMode>,
    pub cursor_image: Option<Pos2>,
    pub job: Option<Job>,
    pub preview: Option<Detection>,
    pub calib: CalibrationState,
    pub messages: Vec<Message>,
    devices_listed: bool,
    pub show_help: bool,
    last_view_rect: Rect,
    /// The live view's widget id; arrow keys nudge the centre only while it has focus.
    view_id: Option<egui::Id>,
    pub fullscreen: bool,
}

impl ColliViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let s: Settings = cc.storage.and_then(|st| eframe::get_value(st, eframe::APP_KEY)).unwrap_or_default();
        let worker = CameraWorker::spawn(cc.egui_ctx.clone());
        worker.send(Command::SetProcessing(s.processing));
        worker.send(Command::SetScene(s.scene));
        worker.send(Command::ListDevices);
        Self {
            s,
            ctx: cc.egui_ctx.clone(),
            worker,
            devices: Vec::new(),
            current: None,
            modes: Vec::new(),
            controls: Vec::new(),
            texture: None,
            texture_opts: TextureOptions::LINEAR,
            frame: None,
            frozen: false,
            drag: None,
            cursor_image: None,
            job: None,
            preview: None,
            calib: CalibrationState::default(),
            messages: Vec::new(),
            devices_listed: false,
            show_help: false,
            last_view_rect: Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 960.0)),
            view_id: None,
            fullscreen: false,
        }
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.messages.push(Message { text: text.into(), error: false, at: Instant::now() });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::error!("{text}");
        self.messages.push(Message { text, error: true, at: Instant::now() });
    }

    pub fn open_device(&mut self, device: DeviceInfo, mode: Option<VideoMode>) {
        self.worker.send(Command::Open { device, mode });
    }

    pub fn image_size(&self) -> Option<Vec2> {
        if let Some(f) = &self.frame {
            return Some(vec2(f.image.size[0] as f32, f.image.size[1] as f32));
        }
        self.current.as_ref().map(|(_, m)| vec2(m.width as f32, m.height as f32))
    }

    pub fn image_center(&self) -> Option<Pos2> {
        self.image_size().map(|s| (s / 2.0).to_pos2())
    }

    pub fn set_processing(&mut self, p: Processing) {
        if p != self.s.processing {
            self.s.processing = p;
            self.worker.send(Command::SetProcessing(p));
        }
    }

    pub fn set_scene(&mut self, scene: Scene) {
        if scene != self.s.scene {
            self.s.scene = scene;
            self.worker.send(Command::SetScene(scene));
        }
    }

    pub fn is_test_pattern(&self) -> bool {
        matches!(&self.current, Some((d, _)) if d.kind == DeviceKind::TestPattern)
    }

    /// Zoom presets relative to the last known view rectangle.
    pub fn zoom_preset(&mut self, zoom: Option<f32>) {
        let Some(size) = self.image_size() else { return };
        match zoom {
            None => self.s.view.reset(),
            Some(z) => {
                let pan = self.s.view.pan.unwrap_or((size / 2.0).to_pos2());
                self.s.view.zoom_to(z, pan);
            }
        }
    }

    pub fn zoom_one_to_one(&mut self) {
        let Some(size) = self.image_size() else { return };
        let vt = ViewTransform::new(self.last_view_rect, size, &self.s.view);
        let pan = self.s.view.pan.unwrap_or((size / 2.0).to_pos2());
        self.s.view.zoom_to(1.0 / vt.fit_scale(), pan);
    }

    // ---------------------------------------------------------------- events

    fn poll_worker(&mut self) {
        for e in self.worker.poll_events() {
            match e {
                Event::Devices(list) => {
                    self.devices = list;
                    if !self.devices_listed {
                        self.devices_listed = true;
                        self.open_initial_device();
                    }
                }
                Event::Opened { device, mode, modes } => self.on_opened(device, mode, modes),
                Event::Controls(c) => self.controls = c,
                Event::Closed => {
                    self.current = None;
                    self.controls.clear();
                }
                Event::Error(msg) => self.error(msg),
            }
        }
        if let Some(f) = self.worker.take_frame()
            && !self.frozen
        {
            self.set_frame(f);
        }
    }

    fn open_initial_device(&mut self) {
        // Prefer the last used device (matched by name; indices can change), then the
        // first real camera, then the test pattern.
        let last = self.s.device.clone();
        let pick = last
            .as_ref()
            .and_then(|d| self.devices.iter().find(|x| x.name == d.name))
            .or_else(|| self.devices.iter().find(|d| d.kind != DeviceKind::TestPattern))
            .or_else(|| self.devices.first())
            .cloned();
        if let Some(dev) = pick {
            let mode = if last.as_ref().is_some_and(|l| l.name == dev.name) { self.s.mode } else { None };
            self.open_device(dev, mode);
        }
    }

    fn on_opened(&mut self, device: DeviceInfo, mode: VideoMode, modes: Vec<VideoMode>) {
        let [ow, oh] = self.s.overlay.ref_resolution;
        let pristine = self.s.overlay == Overlay::newtonian_preset(ow, oh);
        match self.s.overlay.adapt_to_resolution(mode.width, mode.height) {
            Rescale::Unchanged => {}
            Rescale::Scaled(k) if (k - 1.0).abs() > 1e-6 => {
                self.info(format!("Resolution changed: circles rescaled ×{k:.3}"))
            }
            Rescale::Scaled(_) => {}
            Rescale::AspectChanged if pristine => {
                self.s.overlay = Overlay::newtonian_preset(mode.width, mode.height);
            }
            Rescale::AspectChanged => {
                self.info("Aspect ratio changed (the camera may crop): circles were kept as-is — re-run Auto-detect")
            }
        }
        self.s.device = Some(device.clone());
        self.s.mode = Some(mode);
        self.current = Some((device, mode));
        self.modes = modes;
        self.frame = None;
        self.texture = None;
        self.frozen = false;
        self.s.view.reset();
        self.calib.reset();
    }

    fn set_frame(&mut self, f: Frame) {
        let opts = self.texture_opts;
        match &mut self.texture {
            Some(t) if t.size() == f.image.size => t.set(f.image.clone(), opts),
            _ => {
                self.texture = Some(self.ctx.load_texture("camera", f.image.clone(), opts));
            }
        }
        self.frame = Some(f);
    }

    /// Switch nearest/linear sampling depending on magnification.
    fn update_texture_filter(&mut self, scale: f32) {
        let want = if self.s.view.nearest_when_zoomed && scale >= 2.0 {
            TextureOptions::NEAREST
        } else {
            TextureOptions::LINEAR
        };
        if want != self.texture_opts {
            self.texture_opts = want;
            if let (Some(t), Some(f)) = (&mut self.texture, &self.frame) {
                t.set(f.image.clone(), want);
            }
        }
    }

    // ------------------------------------------------------------- detection

    pub fn current_image(&self) -> Option<Arc<ColorImage>> {
        self.frame.as_ref().map(|f| f.image.clone())
    }

    /// Start a detection job. `searches` are run in order on the same frame.
    fn spawn_job(&mut self, kind: JobKind, searches: Vec<DetectParams>) {
        let Some(image) = self.current_image() else {
            self.error("No frame to analyse yet");
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let gray = GrayImage::from_color_image(&image);
            let mut results = Vec::new();
            let mut prev: Option<Detection> = None;
            for (i, mut p) in searches.into_iter().enumerate() {
                // Searches after the first start from the previous (rim) result.
                if i > 0
                    && let Some(d) = prev
                {
                    p.center = d.center;
                }
                let d = detect_circle(&gray, &p);
                results.push(d);
                prev = prev.or(d);
            }
            let _ = tx.send(results);
            ctx.request_repaint();
        });
        self.job = Some(Job { kind, rx });
    }

    pub fn detect_reference(&mut self) {
        let Some(size) = self.image_size() else { return };
        let o = &self.s.overlay;
        let params = if self.s.detect_wide {
            let m = size.x.min(size.y);
            DetectParams {
                radius_min: 0.08 * m,
                radius_max: 0.55 * m,
                ..DetectParams::around((size / 2.0).to_pos2(), 0.3 * m, 0.0)
            }
        } else {
            DetectParams::around(o.center, o.reference_radius().unwrap_or(0.4 * size.y), self.s.detect_tolerance)
        };
        self.preview = None;
        self.spawn_job(JobKind::Reference, vec![params]);
    }

    pub fn measure_layer(&mut self, id: LayerId) {
        let o = &self.s.overlay;
        let Some(layer) = o.layer(id) else { return };
        let Some(r) = layer.radius() else { return };
        let params = DetectParams::around(o.layer_center(layer), r, 0.15);
        self.spawn_job(JobKind::Measure(id), vec![params]);
    }

    pub fn calibration_capture(&mut self, step: usize) {
        let Some(target) = self.calib.target else { return };
        let o = &self.s.overlay;
        let (Some(rim_r), Some(layer)) = (o.reference_radius(), o.layer(target)) else { return };
        let Some(r) = layer.radius() else { return };
        if self.calib.manual {
            let sample =
                RotationSample { rim: Measurement { center: o.center, radius: rim_r }, target: o.layer_center(layer) };
            self.store_calibration_sample(step, sample);
            return;
        }
        let rim = DetectParams::around(o.center, rim_r, 0.15);
        // The target search starts at the rim result (see `spawn_job`), so it does not
        // matter where the target circle currently sits.
        let tgt = DetectParams::around(o.center, r, 0.15);
        self.spawn_job(JobKind::Calibration { step, target }, vec![rim, tgt]);
    }

    /// One live measurement of the rim and the target, for the carrier-screw readout.
    fn calibration_live_measure(&mut self) {
        let Some(target) = self.calib.target else { return };
        let o = &self.s.overlay;
        let (Some(rim_r), Some(r)) = (o.reference_radius(), o.layer(target).and_then(|l| l.radius())) else {
            return;
        };
        let rim = DetectParams::around(o.center, rim_r, 0.15);
        let tgt = DetectParams::around(o.center, r, 0.15);
        self.calib.live_next = Some(Instant::now() + LIVE_INTERVAL);
        self.spawn_job(JobKind::LiveBias { target }, vec![rim, tgt]);
    }

    /// Start the next live bias measurement when the previous one is done.
    fn run_live_bias(&mut self) {
        let ready = self.job.is_none()
            && !self.frozen
            && self.frame.is_some()
            && self.calib.result.is_some_and(|r| r.coaxial)
            && self.calib.live_next.is_none_or(|t| Instant::now() >= t);
        if ready {
            self.calibration_live_measure();
        }
    }

    fn store_calibration_sample(&mut self, step: usize, sample: RotationSample) {
        self.calib.stop_live();
        self.calib.samples[step] = Some(sample);
        self.calib.result = match self.calib.samples {
            [Some(a), Some(b)] => Some(crate::calibrate::solve(&a, &b, crate::calibrate::COAXIAL_TOLERANCE)),
            _ => None,
        };
    }

    fn poll_job(&mut self) {
        let Some(job) = &self.job else { return };
        let results = match job.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                self.error("Detection failed unexpectedly");
                return;
            }
        };
        let kind = job.kind;
        self.job = None;
        let failed = "No clear circular edge found. Put the circle roughly on the feature, or widen the search";
        match kind {
            JobKind::Reference => match results.first().copied().flatten() {
                Some(d) => self.preview = Some(d),
                None => self.error(failed),
            },
            JobKind::Measure(id) => match results.first().copied().flatten() {
                Some(d) => {
                    if let Some(l) = self.s.overlay.layer_mut(id) {
                        l.measured = Some(Measurement { center: d.center, radius: d.radius });
                    }
                    self.info(format!("Measured: r = {:.1} px, fit {} (rms {:.2} px)", d.radius, d.quality(), d.rms));
                }
                None => self.error(failed),
            },
            JobKind::Calibration { step, target } => {
                match (results.first().copied().flatten(), results.get(1).copied().flatten()) {
                    (Some(rim), Some(t)) => {
                        let sample = RotationSample {
                            rim: Measurement { center: rim.center, radius: rim.radius },
                            target: t.center,
                        };
                        if let Some(l) = self.s.overlay.layer_mut(target) {
                            l.measured = Some(Measurement { center: t.center, radius: t.radius });
                        }
                        self.store_calibration_sample(step, sample);
                    }
                    (None, _) => self.error(format!("Calibration: could not find the drawtube rim. {failed}")),
                    (_, None) => self.error(format!("Calibration: could not find the target feature. {failed}")),
                }
            }
            // Dropped if the readout was turned off meanwhile. Misses only count, so a
            // bad frame doesn't flood the status bar.
            JobKind::LiveBias { target } if self.calib.live && self.calib.target == Some(target) => {
                match (results.first().copied().flatten(), results.get(1).copied().flatten(), self.calib.result) {
                    (Some(rim), Some(t), Some(result)) => {
                        let rim = Measurement { center: rim.center, radius: rim.radius };
                        // The rim moves with the camera: keep the circles on it.
                        self.s.overlay.fit_reference(rim.center, rim.radius);
                        if let Some(l) = self.s.overlay.layer_mut(target) {
                            l.measured = Some(Measurement { center: t.center, radius: t.radius });
                        }
                        let sample = RotationSample { rim, target: t.center };
                        self.calib.live_bias = Some(crate::calibrate::live_bias(&result, &sample));
                        self.calib.live_failures = 0;
                    }
                    _ => self.calib.live_failures += 1,
                }
            }
            JobKind::LiveBias { .. } => {}
        }
    }

    pub fn accept_preview(&mut self) {
        if let Some(d) = self.preview.take() {
            self.s.overlay.fit_reference(d.center, d.radius);
            self.info(format!(
                "Focuser ring fitted: r = {:.1} px ({} fit, rms {:.2} px)",
                d.radius,
                d.quality(),
                d.rms
            ));
        }
    }

    // ------------------------------------------------------------ snapshots

    pub fn save_snapshot(&mut self) {
        let Some(img) = self.current_image() else {
            self.error("No frame to save");
            return;
        };
        let overlay = self.s.snapshot_overlay.then_some(&self.s.overlay);
        let composed = crate::snapshot::compose(&img, overlay, self.s.view.orientation);
        let name = format!("colliview-{}.png", chrono_like_stamp());
        if let Some(path) = rfd::FileDialog::new().set_file_name(name).add_filter("PNG image", &["png"]).save_file() {
            match crate::snapshot::save_png(&composed, &path) {
                Ok(()) => self.info(format!("Saved {}", path.display())),
                Err(e) => self.error(format!("Saving snapshot failed: {e:#}")),
            }
        }
    }

    // ------------------------------------------------------------- the view

    fn central_view(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        self.last_view_rect = rect;
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        // The view takes keyboard focus by default and when clicked, so arrow keys go
        // to the centre nudge. A focused slider or number field keeps its arrows.
        self.view_id = Some(response.id);
        if response.clicked() || response.drag_started() || ui.memory(|m| m.focused().is_none()) {
            response.request_focus();
        }
        if response.has_focus() {
            let filter = egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() };
            ui.memory_mut(|m| m.set_focus_lock_filter(response.id, filter));
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(10));

        let Some(size) = self.image_size() else {
            let text = if self.devices_listed {
                "No camera open — pick one in the top bar"
            } else {
                "Looking for cameras…"
            };
            painter.text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(18.0), Color32::GRAY);
            return;
        };
        let vt = ViewTransform::new(rect, size, &self.s.view);
        self.update_texture_filter(vt.scale());

        if let Some(tex) = &self.texture {
            let (quad, uv) = vt.image_quad();
            let mut mesh = Mesh::with_texture(tex.id());
            mesh.add_rect_with_uv(quad, uv, Color32::WHITE);
            painter.add(Shape::mesh(mesh));
        }
        if self.s.show_image_center {
            let c = vt.to_screen((size / 2.0).to_pos2());
            let st = Stroke::new(1.0, Color32::from_white_alpha(110));
            painter.line_segment([c - vec2(14.0, 14.0), c + vec2(14.0, 14.0)], st);
            painter.line_segment([c - vec2(14.0, -14.0), c + vec2(14.0, -14.0)], st);
        }
        paint_overlay(&painter, &vt, &self.s.overlay);
        for l in self.s.overlay.layers.iter().filter(|l| l.visible && l.id != self.s.overlay.reference) {
            if let Some(m) = &l.measured {
                paint_dashed_circle(&painter, &vt, m, l.color.gamma_multiply(0.8));
            }
        }
        if let Some(d) = &self.preview {
            let m = Measurement { center: d.center, radius: d.radius };
            paint_dashed_circle(&painter, &vt, &m, Color32::from_rgb(255, 140, 0));
            painter.text(
                vt.to_screen(d.center) + vec2(0.0, -d.radius * vt.scale() - 14.0),
                Align2::CENTER_BOTTOM,
                format!("Detected ({}) — Accept / Reject in the left panel", d.quality()),
                FontId::proportional(14.0),
                Color32::from_rgb(255, 170, 60),
            );
        }
        // The live bias readout runs jobs back to back; it shows in its own panel.
        if self.job.as_ref().is_some_and(|j| !matches!(j.kind, JobKind::LiveBias { .. })) {
            painter.text(
                rect.left_top() + vec2(12.0, 12.0),
                Align2::LEFT_TOP,
                "Detecting…",
                FontId::proportional(16.0),
                Color32::YELLOW,
            );
        }
        if self.frozen {
            painter.text(
                rect.right_top() + vec2(-12.0, 12.0),
                Align2::RIGHT_TOP,
                "FROZEN",
                FontId::proportional(16.0),
                Color32::LIGHT_BLUE,
            );
        }
        self.view_input(ui, &response, &vt);
    }

    fn view_input(&mut self, ui: &Ui, response: &Response, vt: &ViewTransform) {
        let hover = response.hover_pos();
        self.cursor_image = hover.map(|p| vt.to_image(p));

        if response.hovered() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = pinch * (scroll * 0.0015).exp();
            if (factor - 1.0).abs() > 1e-4
                && let Some(p) = hover
            {
                self.s.view.zoom_at(vt, p, factor);
            }
        }

        let center_screen = vt.to_screen(self.s.overlay.center);
        let near_center = |p: Pos2| (p - center_screen).length() <= 10.0;
        let selected_circle = self
            .s
            .overlay
            .selected_layer()
            .filter(|l| l.visible)
            .and_then(|l| l.radius().map(|r| (l.id, vt.to_screen(self.s.overlay.layer_center(l)), r * vt.scale())));
        let near_edge = |p: Pos2| selected_circle.filter(|(_, c, r)| ((p - *c).length() - r).abs() <= 6.0).map(|s| s.0);

        if let Some(p) = hover
            && self.drag.is_none()
        {
            if near_center(p) {
                ui.ctx().set_cursor_icon(CursorIcon::Move);
            } else if near_edge(p).is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw);
            }
        }

        if response.drag_started()
            && let Some(p) = response.interact_pointer_pos()
        {
            let alt = ui.input(|i| i.modifiers.alt);
            let img_p = vt.to_image(p);
            let selected_offsettable = self
                .s
                .overlay
                .selected
                .filter(|id| *id != self.s.overlay.reference && self.s.overlay.layer(*id).is_some());
            self.drag =
                Some(if response.dragged_by(PointerButton::Middle) || response.dragged_by(PointerButton::Secondary) {
                    DragMode::Pan
                } else if let (true, Some(id)) = (alt, selected_offsettable) {
                    let l = self.s.overlay.layer(id).unwrap();
                    DragMode::LayerOffset { id, grab: self.s.overlay.layer_center(l) - img_p }
                } else if near_center(p) {
                    DragMode::Center { grab: self.s.overlay.center - img_p }
                } else if let Some(id) = near_edge(p) {
                    DragMode::Radius(id)
                } else {
                    DragMode::Pan
                });
        }
        if response.dragged()
            && let (Some(mode), Some(p)) = (self.drag, response.interact_pointer_pos())
        {
            let img_p = vt.to_image(p);
            match mode {
                DragMode::Pan => {
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                    self.s.view.pan_by(vt, response.drag_delta());
                }
                DragMode::Center { grab } => self.s.overlay.center = img_p + grab,
                DragMode::Radius(id) => {
                    let o = &mut self.s.overlay;
                    if let Some(c) = o.layer(id).map(|l| o.layer_center(l)) {
                        o.layer_mut(id).unwrap().set_radius((img_p - c).length());
                    }
                }
                DragMode::LayerOffset { id, grab } => {
                    let center = self.s.overlay.center;
                    if let Some(l) = self.s.overlay.layer_mut(id) {
                        l.offset = img_p + grab - center;
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.drag = None;
        }
        if response.double_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.s.overlay.center = vt.to_image(p);
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        // Shortcuts belong to the live view, which holds keyboard focus by default and
        // when clicked. While a text field, slider or number field is focused, its keys
        // stay with it.
        let focused = ctx.memory(|m| m.focused());
        let view_focused = focused.is_none() || focused == self.view_id;
        let (mods, keys) = ctx.input(|i| {
            let keys: Vec<Key> = [
                Key::ArrowLeft,
                Key::ArrowRight,
                Key::ArrowUp,
                Key::ArrowDown,
                Key::OpenBracket,
                Key::CloseBracket,
                Key::Plus,
                Key::Equals,
                Key::Minus,
                Key::Num0,
                Key::F11,
                Key::H,
                Key::F,
                Key::D,
            ]
            .into_iter()
            .filter(|k| i.key_pressed(*k))
            .collect();
            (i.modifiers, keys)
        });
        let step = if mods.shift {
            10.0
        } else if mods.command || mods.ctrl {
            0.1
        } else {
            1.0
        };
        for k in keys {
            match k {
                // Function keys never type text, so full screen works everywhere.
                Key::F11 => {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                }
                _ if !view_focused => {}
                Key::ArrowLeft => self.s.overlay.center.x -= step,
                Key::ArrowRight => self.s.overlay.center.x += step,
                Key::ArrowUp => self.s.overlay.center.y -= step,
                Key::ArrowDown => self.s.overlay.center.y += step,
                Key::OpenBracket | Key::CloseBracket => {
                    let sign = if k == Key::CloseBracket { 1.0 } else { -1.0 };
                    if let Some(id) = self.s.overlay.selected
                        && let Some(l) = self.s.overlay.layer_mut(id)
                        && let Some(r) = l.radius()
                    {
                        l.set_radius(r + sign * step);
                    }
                }
                Key::Plus | Key::Equals => self.zoom_step(1.25),
                Key::Minus => self.zoom_step(0.8),
                Key::Num0 => self.s.view.reset(),
                Key::H => self.s.show_panels = !self.s.show_panels,
                Key::F => self.frozen = !self.frozen,
                Key::D => self.detect_reference(),
                _ => {}
            }
        }
    }

    fn zoom_step(&mut self, factor: f32) {
        let Some(size) = self.image_size() else { return };
        let vt = ViewTransform::new(self.last_view_rect, size, &self.s.view);
        self.s.view.zoom_at(&vt, self.last_view_rect.center(), factor);
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some(f) = &self.frame {
                ui.label(format!("{}×{}  {:.1} fps", f.image.size[0], f.image.size[1], f.fps));
                ui.separator();
            }
            if let Some(p) = self.cursor_image {
                ui.label(format!("cursor x {:.1}  y {:.1}", p.x, p.y));
                ui.separator();
            }
            ui.label(format!("zoom {:.0} %", self.s.view.zoom * 100.0));
            if let Some(ic) = self.image_center() {
                let d = self.s.overlay.center - ic;
                ui.separator();
                ui.label(format!("centre vs image centre: {:+.1}, {:+.1} px", d.x, d.y))
                    .on_hover_text("Offset of the shared centre (focuser ring) from the sensor centre.\nOnly needs to be roughly 0: centre the camera precisely with\nOff-centre camera calibration → Live.");
            }
            self.messages.retain(|m| m.at.elapsed() < Duration::from_secs(if m.error { 12 } else { 6 }));
            if let Some(m) = self.messages.last() {
                ui.separator();
                let color = if m.error { Color32::from_rgb(255, 110, 100) } else { Color32::from_rgb(140, 220, 140) };
                ui.colored_label(color, &m.text);
            }
        });
    }
}

fn chrono_like_stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    secs.to_string()
}

impl eframe::App for ColliViewApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        self.poll_job();
        if self.calib.live {
            self.run_live_bias();
            ctx.request_repaint_after(LIVE_INTERVAL);
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if !self.messages.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.keyboard(&ctx);
        egui::Panel::top("top_bar").show(ui, |ui| ui::top_bar(self, ui));
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        if self.s.show_panels {
            egui::Panel::left("layers").default_size(310.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| ui::layers_panel(self, ui));
            });
            egui::Panel::right("tools").default_size(330.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| ui::tools_panel(self, ui));
            });
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.central_view(ui));
        ui::help_window(self, &ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.s);
    }
}
