# ColliView

Live USB-camera collimation aid for Newtonian telescopes, in the spirit of the OCAL.
Put a camera in the focuser and line up the drawtube, the secondary and the primary
reflections with adjustable concentric circles drawn over the live view.
Runs on Windows and Linux; written in Rust with [egui](https://github.com/emilk/egui).

## Features

- **Live view** from any UVC webcam or camera module (MJPEG/YUYV/NV12), with a resolution and frame-rate picker.
- **Camera controls**, generated from what the camera offers: focus, exposure, gain, white balance,
  brightness, contrast and more, each with an *auto* toggle. **Manual focus** has its own section with
  fine steps (Windows through `IAMCameraControl`, Linux through V4L2). It is hidden when the camera has no focus motor.
- **Layered circles** sharing one centre, like layers in an image editor: per-layer colour, line width,
  radius and visibility; add, duplicate, reorder and delete layers. A rotatable **crosshair** layer helps
  check spider vanes and secondary rotation.
- The **shared centre** moves with x/y fields, arrow keys (1 / 10 / 0.1 px) or by dragging the centre cross.
  Drag a circle's edge to resize it.
- Overlays are stored in **sensor-pixel coordinates**, so they stay locked to the image when the window
  is resized, zoomed, panned or flipped.
- **Zoom** with the mouse wheel around the cursor, Fit / 1:1 / 2× / 4× / 8× presets, *zoom on centre*,
  and sharp nearest-neighbour pixels when zoomed in.
- **Auto-detect** fits the focuser ring and the shared centre to the drawtube edge with sub-pixel accuracy
  (radial edge search, RANSAC, geometric least squares), shown as a preview you accept or reject.
  **Measure** fits any other layer's feature and reports how far off it is.
- All layers share the reference ring's centre, so lining features up on one centre is all there is to it.
- **Camera centring check** with a printable two-depth **jig**: a live readout shows how far the lens sits
  off the barrel's axis, in mm, and which way to push the camera board. A sensor a few pixels off-centre or
  a tilted camera doesn't matter; the circles follow the drawtube. See [hardware/ §5](hardware/README.md#5-check-the-camera-centring).
- **Software image tools**: brightness, contrast, gamma, invert, greyscale, frame averaging for a dim tube,
  and display flip / rotate 180°.
- Freeze frame, PNG **snapshots** at sensor resolution (with or without overlays), **per-telescope profiles**
  (JSON), and automatic restore of the last session.
- A built-in **simulated Newtonian** test pattern, to try everything without a camera.
- A parametric **3D-printable 2" adapter** with a laser-cut diffuser light ring: see [hardware/](hardware/README.md).

## Building

Requires Rust **1.95 or newer** (egui 0.36): `rustup update stable`.

```sh
cargo run --release
```

**Linux** needs a few system packages (Debian/Ubuntu names):

```sh
sudo apt install build-essential pkg-config libclang-dev libxkbcommon-dev libwayland-dev
sudo usermod -aG video $USER   # access to /dev/video*; log in again afterwards
```

`libclang-dev` is for the V4L2 bindings (bindgen). File dialogs use the XDG desktop portal.

## Using it

1. Insert the camera adapter, light the tube and pick the camera in the top bar.
2. Press **D** (Auto-detect). Check that the dashed orange circle sits on the drawtube's far edge and click **Accept**.
   The yellow *Focuser drawtube* ring (★, the reference) and the shared centre now follow the drawtube.
3. **Secondary:** adjust the secondary until its outline is concentric with the green circle.
   Select the layer and click **Measure** to get the remaining offset in pixels.
4. **Primary:** adjust the primary until its reflection, clips, the light ring's reflection and the centre spot
   line up with the red, magenta and cyan circles.
5. Save a **profile** for the telescope. Check the camera's centring once with the
   [centring jig](hardware/README.md#5-check-the-camera-centring) (**Camera centring (jig)**).

| Input | Action |
|---|---|
| Mouse wheel | Zoom around the cursor |
| Drag on empty space, or right/middle drag | Pan |
| Drag the white centre cross / double-click | Move / set the shared centre |
| Drag the selected circle's edge | Resize it (keeps going past the screen edge) |
| Arrow keys | Nudge the centre 1 px (Shift 10 px, Ctrl 0.1 px) |
| `[` / `]` | Shrink / grow the selected circle |
| `+` / `−` / `0` | Zoom in / out / fit |
| `D` / `F` / `H` / `F11` | Auto-detect / freeze / hide panels / full screen |

## How it works

| Path | What it does |
|---|---|
| `src/camera/` | Worker thread that owns capture and controls. It sends frames through a single-slot mailbox. |
| `src/camera/nokhwa_capture.rs` | Capture through [nokhwa](https://crates.io/crates/nokhwa). |
| `src/camera/controls/` | Native controls. nokhwa can't switch a control from auto to manual on Windows, so ColliView opens a second Media Foundation source (Windows) or `/dev/videoN` handle (Linux) just for controls. |
| `src/view.rs` | The sensor↔screen transform shared by the texture and the overlays. |
| `src/overlay/` | Layers, presets, resolution changes. |
| `src/detect.rs` | Circle detection. |
| `src/jig.rs` | The centring jig's geometry: lens offset in mm from its two fitted holes. |
| `src/camera/test_pattern.rs` | The simulated Newtonian (or the view into the jig), also used by the tests. |
| `examples/controls_spike.rs` | The Windows check that manual focus works while streaming. |

```sh
cargo test     # transforms, circle fitting, detection, the jig readout, processing, profiles
cargo clippy --all-targets
```
