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
- **Off-centre camera calibration**: measure at 0° and 180° adapter rotation. ColliView separates the
  camera's own bias (off-centre sensor, tilt, parallax) from the telescope's real misalignment and stores
  the bias as a per-layer offset. It warns when the adapter didn't rotate about the focuser axis.
  A **live bias readout** guides the adapter's carrier screws until the lens sits on the focuser axis.
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
5. If your camera isn't perfectly centred, run **Off-centre camera calibration** on the secondary and
   tick **Live** to centre the camera with the adapter's carrier screws. Then calibrate once per layer
   and save a **profile** for the telescope.

| Input | Action |
|---|---|
| Mouse wheel | Zoom around the cursor |
| Drag on empty space, or right/middle drag | Pan |
| Drag the white centre cross / double-click | Move / set the shared centre |
| Drag the selected circle's edge | Resize it |
| Alt + drag | Move the selected layer's offset |
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
| `src/detect.rs`, `src/calibrate.rs` | Circle detection and the rotation calibration. |
| `src/camera/test_pattern.rs` | The simulated Newtonian, also used by the tests. |
| `examples/controls_spike.rs` | The Windows check that manual focus works while streaming. |

```sh
cargo test     # transforms, circle fitting, detection, calibration, processing, profiles
cargo clippy --all-targets
```
