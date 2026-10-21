# ColliView

***Full disclaimer, most of this projects code and 3D model is AI assisted as I wanted to test the capabilities for a hobby
 project like this with both code and 3D models. As well as help me build it in shorter time.*** 

<img width="1895" height="1182" alt="Image" src="https://github.com/user-attachments/assets/1de6d236-18e5-4d32-9706-beb13c804066" />

This is my DIY camera collimation project. I was inspired by OCAL but wanted to make my own. Last year I took a simple logitech
 webcam and 3D printed a mount and used that to try and collimate with various success. This season I wanted to upgrade it
 and make it nicer for myself to use. 

It consist of an adjustable 3D printed model for mounting an autofocus IMX179 camera board on with support for lighting through LEDs. The software part control and makes it possible to set the circles used for collimation.

The camera sits in a 3D-printed 2" adapter with a diffused LED light ring. See [hardware/](hardware/README.md)
for the model, the camera presets and the build.

## Features
- control any usb camera supporting UVC protocol
- Concentric circle layers with shared center position
- Auto-detect feature trying to estimate and auto-detect the focuser-tube for easy 1-click fit of the center
- A camera centering "jig" used to calibrate the camera housing to be in center. (For telescopes around F/5 you can just print the static non-adjustable carrier as the minor offset from center with the PCB should be good enough for collimation)
- parametric 3D models. (Read below how to make a preset that fit to another camera board)
- a test-pattern to try out the features without a camera connected

## Equipment used
- 3D printer and PETG filament
- opaque acrylic cut with a lasercutter
- Soldering iron and some wires + JST connector

## Materials used
- PETG filament
- 3mm opaque acrylic
- LED COB strip 5mm - https://www.aliexpress.com/item/1005008168129046.html
- micro lens f4.5 IMX179 with autofocus - https://www.aliexpress.com/item/1005005334799549.html


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

## Usage

1. Put the adapter in the focuser and pick the camera in the top bar of the gui.
2. Press **D** or click on the auto-detect button to attempt auto-fitting the drawtube edge, check the dashed preview and click **Accept** if its good enough. (Ensure your focus is for the drawtube or as close it can be for better performance. Alternatively just adjust the ring yourself with the center coordinates top-left, with hotkeys or by dragging with mouse)
3. Enable the secondary and primary reflection ring, change focus and exposure to be optimal for the secondary mirror. 
4. Adjust the secondary mirror until it is concentric within the circle and you can see the entire primary mirror in its reflection. (Adjust the ring size to easier see the fit) its concentric with the focuser edge  until its outline is concentric with the green circle.
4. Adjust the primary until its center reflection is concentric within the small circle. Ideally at this point the primary mirrors center-marking should be aligned with your camera lens reflection.
5. Save a profile for the telescope if you want to save the circle sizes for another time.

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

## Generate 3D models and how to assemble it
See [hardware/](hardware/README.md)
