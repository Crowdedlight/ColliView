use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use egui::{
    Align2, Color32, ColorImage, CursorIcon, FontId, Key, Mesh, PointerButton, Pos2, Rect, Response, Sense, Shape,
    Stroke, TextureHandle, TextureOptions, Ui, Vec2, vec2,
};
use serde::{Deserialize, Serialize};

use crate::camera::test_pattern::Scene;
use crate::camera::{CameraWorker, Command, ControlDesc, DeviceInfo, DeviceKind, Event, Frame, VideoMode};
use crate::detect::{DetectParams, Detection, GrayImage, Polarity, detect_circle};
use crate::jig::JigGeometry;
use crate::overlay::draw::{paint_dashed_circle, paint_overlay};
use crate::overlay::{LayerId, Measurement, Overlay, Rescale};
use crate::processing::Processing;
use crate::ui;
use crate::ui::endless::EndlessFields;
use crate::view::{EndlessDrag, ViewState, ViewTransform};

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
    pub jig: JigGeometry,
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
            jig: JigGeometry::default(),
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
    /// Resize a circle. The drag goes on past the screen edge, with the pointer hidden.
    Radius {
        id: LayerId,
        drag: EndlessDrag,
    },
}

/// What a background detection job is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JobKind {
    /// Fit the reference ring; result is shown as a preview to accept/reject.
    Reference,
    /// Fit another layer's feature; stored as that layer's measurement.
    Measure(LayerId),
    /// Live jig readout: the near aperture, then the far hole.
    Jig,
}

/// Time between live jig measurements.
const LIVE_INTERVAL: Duration = Duration::from_millis(250);

pub struct Job {
    pub kind: JobKind,
    rx: Receiver<Vec<Option<Detection>>>,
}

/// The live jig readout.
#[derive(Default)]
pub struct JigState {
    pub live: bool,
    /// The last fits of the near aperture and the far hole.
    pub fits: Option<(Detection, Detection)>,
    /// Measurements that missed a hole since the last good one.
    pub misses: u32,
    next: Option<Instant>,
}

impl JigState {
    pub fn stop(&mut self) {
        *self = Self::default();
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
    endless: EndlessFields,
    pub cursor_image: Option<Pos2>,
    pub job: Option<Job>,
    pub preview: Option<Detection>,
    pub jig: JigState,
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
            endless: EndlessFields::default(),
            cursor_image: None,
            job: None,
            preview: None,
            jig: JigState::default(),
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
        self.jig.stop();
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
                // Searches after the first start from the previous result.
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

    /// Search 8–55 % of the frame from the image centre.
    fn wide_search(size: Vec2) -> DetectParams {
        let m = size.x.min(size.y);
        DetectParams {
            radius_min: 0.08 * m,
            radius_max: 0.55 * m,
            ..DetectParams::around((size / 2.0).to_pos2(), 0.3 * m, 0.0)
        }
    }

    pub fn detect_reference(&mut self) {
        let Some(size) = self.image_size() else { return };
        let o = &self.s.overlay;
        let params = if self.s.detect_wide {
            Self::wide_search(size)
        } else {
            DetectParams::around(o.center, o.reference_radius().unwrap_or(0.4 * size.y), self.s.detect_tolerance)
        };
        self.preview = None;
        self.spawn_job(JobKind::Reference, vec![params]);
    }

    /// Find the centring jig's aperture for the reference ring, shown as a preview to
    /// accept. Only edges that are dark inside count: the baffle's edge, just outside the
    /// aperture, is bright inside and would otherwise win.
    pub fn find_jig_aperture(&mut self) {
        let Some(size) = self.image_size() else { return };
        self.preview = None;
        self.spawn_job(JobKind::Reference, vec![Self::wide_search(size).with_polarity(Polarity::DarkInside)]);
    }

    pub fn measure_layer(&mut self, id: LayerId) {
        let o = &self.s.overlay;
        let Some(layer) = o.layer(id) else { return };
        let Some(r) = layer.radius() else { return };
        let params = DetectParams::around(o.center, r, 0.15);
        self.spawn_job(JobKind::Measure(id), vec![params]);
    }

    /// One live jig measurement: the near aperture around the reference ring, then the
    /// far hole around the aperture's centre.
    fn jig_measure(&mut self) {
        let o = &self.s.overlay;
        let Some(r) = o.reference_radius() else { return };
        let near = DetectParams::around(o.center, r, 0.12).with_polarity(Polarity::DarkInside);
        // Wide band: a lens offset moves the hole off the aperture's centre.
        let far = DetectParams::around(o.center, r * self.s.jig.far_radius_ratio(), 0.5)
            .with_polarity(Polarity::BrightInside);
        self.jig.next = Some(Instant::now() + LIVE_INTERVAL);
        self.spawn_job(JobKind::Jig, vec![near, far]);
    }

    fn run_jig(&mut self) {
        let ready = self.job.is_none()
            && !self.frozen
            && self.frame.is_some()
            && self.jig.next.is_none_or(|t| Instant::now() >= t);
        if ready {
            self.jig_measure();
        }
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
            // Dropped if the readout was turned off meanwhile. Misses only count, so a
            // bad frame doesn't flood the status bar.
            JobKind::Jig if self.jig.live => {
                match (results.first().copied().flatten(), results.get(1).copied().flatten()) {
                    (Some(near), Some(far)) => {
                        // Keep the circles on the aperture as the board moves.
                        self.s.overlay.fit_reference(near.center, near.radius);
                        self.jig.fits = Some((near, far));
                        self.jig.misses = 0;
                    }
                    _ => self.jig.misses += 1,
                }
            }
            JobKind::Jig => {}
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
        if self.jig.live
            && let Some((near, far)) = &self.jig.fits
        {
            paint_jig(&painter, &vt, near, far);
        }
        // The live jig readout runs jobs back to back; it shows in its own panel.
        if self.job.as_ref().is_some_and(|j| j.kind != JobKind::Jig) {
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
            .and_then(|l| l.radius().map(|r| (l.id, center_screen, r * vt.scale())));
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
            let img_p = vt.to_image(p);
            self.drag =
                Some(if response.dragged_by(PointerButton::Middle) || response.dragged_by(PointerButton::Secondary) {
                    DragMode::Pan
                } else if near_center(p) {
                    DragMode::Center { grab: self.s.overlay.center - img_p }
                } else if let Some(id) = near_edge(p) {
                    DragMode::Radius { id, drag: EndlessDrag::new(p) }
                } else {
                    DragMode::Pan
                });
        }
        if response.dragged()
            && let (Some(mode), Some(p)) = (&mut self.drag, response.interact_pointer_pos())
        {
            let img_p = vt.to_image(p);
            match mode {
                DragMode::Pan => {
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                    self.s.view.pan_by(vt, response.drag_delta());
                }
                DragMode::Center { grab } => self.s.overlay.center = img_p + *grab,
                DragMode::Radius { id, drag } => {
                    // Hidden while resizing: it gets moved back from the edges.
                    ui.ctx().set_cursor_icon(CursorIcon::None);
                    if let Some(to) = drag.update(p, response.rect) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::CursorPosition(to));
                    }
                    let o = &mut self.s.overlay;
                    let c = o.center;
                    if let Some(l) = o.layer_mut(*id) {
                        l.set_radius((vt.to_image(drag.virt) - c).length());
                    }
                }
            }
        }
        if response.drag_stopped() {
            if let Some(DragMode::Radius { id, drag }) = self.drag {
                self.pointer_back_to_ring(ui, vt, response.rect, id, drag.virt);
            }
            self.drag = None;
        }
        if response.double_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.s.overlay.center = vt.to_image(p);
        }
    }

    /// After a radius drag, show the pointer on the ring: towards where the drag ended,
    /// or else nearest to where the pointer is. If neither is in the view, it stays put.
    fn pointer_back_to_ring(&self, ui: &Ui, vt: &ViewTransform, view: Rect, id: LayerId, toward: Pos2) {
        let o = &self.s.overlay;
        let Some((c, r)) = o.layer(id).and_then(|l| l.radius().map(|r| (vt.to_screen(o.center), r * vt.scale())))
        else {
            return;
        };
        let on_ring = |p: Pos2| {
            let d = p - c;
            (d.length() > 1e-3).then(|| c + d.normalized() * r)
        };
        let now = ui.input(|i| i.pointer.latest_pos());
        if let Some(q) = [Some(toward), now].into_iter().flatten().filter_map(on_ring).find(|q| view.contains(*q)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::CursorPosition(q));
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
                    .on_hover_text("Offset of the shared centre (focuser ring) from the sensor centre.\nIt doesn't affect collimation: the circles follow the ring. Check the lens\nposition with the centring jig instead.");
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

/// The jig's two fits, and an arrow from the far hole to the aperture's centre: the way
/// to push the camera board (with the view unflipped).
fn paint_jig(painter: &egui::Painter, vt: &ViewTransform, near: &Detection, far: &Detection) {
    let color = Color32::from_rgb(80, 200, 255);
    for d in [near, far] {
        paint_dashed_circle(painter, vt, &Measurement { center: d.center, radius: d.radius }, color);
    }
    let (from, to) = (vt.to_screen(far.center), vt.to_screen(near.center));
    if (to - from).length() < 3.0 {
        return;
    }
    painter.arrow(from, to - from, Stroke::new(2.0, color));
    painter.text(
        from + vec2(10.0, 8.0),
        Align2::LEFT_TOP,
        "push the board this way",
        FontId::proportional(13.0),
        color,
    );
}

fn chrono_like_stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    secs.to_string()
}

impl eframe::App for ColliViewApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        self.poll_job();
        if self.jig.live {
            self.run_jig();
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
        self.endless.output(&ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let window = raw_input.screen_rect.unwrap_or_else(|| ctx.viewport_rect());
        self.endless.input(raw_input, window);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.s);
    }
}
