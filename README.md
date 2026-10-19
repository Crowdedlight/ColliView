# ColliView

A live USB-camera collimation aid for Newtonian telescopes, like an electronic OCAL. Put the camera in
the focuser, and line up the drawtube, the secondary and the primary reflections with concentric
circles drawn over the live view. It runs on Windows and Linux, and is written in Rust with
[egui](https://github.com/emilk/egui).

The camera sits in a 3D-printed 2" adapter with a diffused LED light ring. See [hardware/](hardware/README.md)
for the model, the camera presets and the build.

## Features

- Live view from any UVC camera (MJPEG/YUYV/NV12), with a resolution and frame-rate picker.
- Native camera controls (focus, exposure, gain, white balance, …), each with an auto toggle.
  These use Media Foundation on Windows and V4L2 on Linux.
- Concentric circle and crosshair layers on one shared centre. They are stored in sensor pixels, so
  they stay locked to the image when you zoom, pan or flip.
- **Auto-detect** fits the reference ring and the centre to the drawtube edge with sub-pixel accuracy.
  **Measure** reports how far another feature is off centre.
- A camera centring check with a printed jig, which reads the lens's offset from the barrel axis in mm.
- Image tools (brightness, contrast, gamma, invert, frame averaging), freeze, PNG snapshots,
  per-telescope JSON profiles, and a simulated Newtonian test pattern for trying it without a camera.

## Running

Download the Windows or Linux build from [Releases](https://github.com/Crowdedlight/ColliView/releases), or build from source with
Rust ≥ 1.95:

```sh
cargo run --release
```

On Linux, building needs these packages (Debian/Ubuntu names), and camera access needs the `video` group:

```sh
sudo apt install build-essential pkg-config libclang-dev libxkbcommon-dev libwayland-dev
sudo usermod -aG video $USER   # log in again afterwards
```

Pushing a `v*` tag builds both platforms and attaches them to a GitHub release
(see [`.github/workflows/release.yml`](.github/workflows/release.yml)).

## Usage

1. Put the adapter in the focuser, light the tube and pick the camera in the top bar.
2. Press **D** to auto-detect the drawtube edge, check the dashed preview and click **Accept**.
3. Adjust the secondary until its outline is concentric with the green circle.
4. Adjust the primary until its reflection, the clips and the centre spot line up with the red,
   magenta and cyan circles.
5. Save a profile for the telescope.

The app's **Help** window lists the mouse and keyboard controls. The main keys:

| Key | Action |
|---|---|
| Arrows | Nudge the centre 1 px (Shift 10 px, Ctrl 0.1 px) |
| `[` / `]` | Shrink / grow the selected circle |
| `+` / `−` / `0` | Zoom in / out / fit |
| `D` / `F` / `H` / `F11` | Auto-detect / freeze / hide panels / full screen |

## Development

```sh
cargo test
cargo clippy --all-targets
```

| Path | What |
|---|---|
| `src/camera/` | Capture worker thread ([nokhwa](https://crates.io/crates/nokhwa)) and the test pattern. |
| `src/camera/controls/` | Native controls. nokhwa can't switch a control from auto to manual on Windows, so these open a second Media Foundation source (Windows) or a `/dev/videoN` handle (Linux). |
| `src/overlay/`, `src/view.rs` | Layers, and the sensor↔screen transform. |
| `src/detect.rs`, `src/jig.rs` | Circle detection, and the centring jig's geometry. |
| `hardware/` | The OpenSCAD adapter model, its build scripts and docs. |
