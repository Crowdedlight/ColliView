# ColliView 2" camera adapter

A 3D-printed 2" adapter that holds a small USB camera module on the focuser axis,
with a diffused LED light ring around the lens, similar to an OCAL. Everything is
parametric in [`colliview_adapter.scad`](colliview_adapter.scad). The laser-cut
acrylic diffuser is exported from the same model, so all parts always match.

```
            cable + zip tie
        ┌──────── cap ────────┐
        │  trimmer, resistor  │   head: floating camera carrier,
        │ ┌─── carrier ─────┐ │◄─ 3 radial M3 centring screws
        │ │ ▄▄▄ camera PCB  │ │
  ══════╧═╧══╤═ lens ═╤═════╧═╧══════  flange: seats on the focuser (sets tilt)
        │    │ baffle │    ▓│
        │    │ (black │    ▓│ ◄─ 5 V COB LED strip on the barrel wall
        │    │  bore) │     │    (white light chamber)
        │    │        │     │
        └▀▀▀▀┘        └▀▀▀▀▀┘ ◄─ opal acrylic ring, held by the retainer
              telescope side
```

## 1. Choose the camera

Everything collimation needs (the secondary, the primary reflection, the clips and
the centre spot) appears inside the far end of the drawtube. So the camera has to
see that rim, and should spend its pixels on it.

1. Measure, on your focuser at its usual position, the distance from where the lens
   will sit to the far end of the drawtube (`lens_to_rim`, typically 95–135 mm) and
   the drawtube's inner diameter (`rim_d`).
2. Set both in the SCAD file and look at the console: it prints
   `Required vertical FOV of the camera: >= N deg`. With the defaults that is
   about 30°, so a lens with **35–45° vertical FOV** is ideal.

| Module | Verdict |
|---|---|
| **OV3660 3 MP, 64° lens** (HBVCAM-3M2111, ~30×25 mm, preset `ov3660_hbv`) | Best of the cheap boards. Before buying, confirm it offers a 4:3 MJPEG mode (2048×1536) and that 64° is the diagonal: then the vertical FOV is ~41°. At 1920×1080 it is ~33°, which is marginal. **Avoid the 110° version**: the rim would be only ~350 px across. |
| OV9726 1 MP (HBV board, ~32×32 mm, preset `ov9726_hbv`) | Only with the **70°** lens (~38° vertical). The 42° lens crops the rim. Low resolution. |
| IMX179 8 MP autofocus (`DB05-AF-8MP` board, 28×28 mm, preset `imx179_db05`) | Compact voice-coil focus can, 10 cm–∞. Manual focus is not yet confirmed: the first thing to check is that `cargo run --example controls_spike` passes. If the stock lens is the usual ~72° diagonal one, the vertical FOV is ~47° in 4:3 modes. That is wider than ideal, but the rim would still be ~1150 px across at 3264×2448 and ~930 px at 2592×1944, if the board offers those modes (similar IMX179 boards do, at 15 fps for the largest). Use a 4:3 MJPEG mode: 1920×1080 cuts the vertical FOV to ~36° at best, and less if the mode is a centre crop. **Avoid the wide M12 lens options**: they change the preset by several mm too. |
| **Autofocus UVC module with manual focus control**, e.g. Arducam B0292 (IMX219 AF, 60° H) or an IMX179 AF board | The upgrade (~$35–60). The app can refocus from the drawtube/secondary (15–25 cm) to the primary reflections (≥ 1 m) without touching the camera. **Check that reviews show a Focus slider in AMCap or `focus_absolute` in `v4l2-ctl`.** Many cheap OV5640 AF boards run continuous AF only. |

**Fixed-focus lenses:** set the focus to about 15–20 cm, between the rim and the
secondary. The primary reflections will be soft. Symmetric blur hardly moves a
circle's fitted centre, so they still work as concentricity references.

> **Measure your board before printing.** The presets were estimated from listing
> photos, not datasheets. Check the PCB size, hole pitch, lens-holder footprint,
> lens body diameter and lens drop with calipers. Then correct the preset's line in
> `preset()`, or switch to `camera = "custom"` and use the `custom_*` values. The lens
> body diameter and drop decide the baffle size and whether the lens physically fits.
>
> `imx179_db05` takes its PCB and holes from the listing's drawing, which shows a
> fixed-focus revision of the board. The focus can's size and height come from photos.
> The photos hint that the lens sits ~1 mm off-centre towards the connector edge, so
> check the lens offset too. The connector's pins come through on the lens side: trim
> them below the 1.5 mm stand-off, or the board won't sit flat on the carrier bosses.

For a board not in the presets, set `camera = "custom"` and fill in `custom_pcb`,
`custom_holes`, `custom_holder`, `custom_lens_offset`, `custom_lens_body_d` and
`custom_lens_drop`. The lens isn't always in the middle of the PCB; the offset puts
the lens on the axis.

## 2. Light ring

* **LED strip:** a 5 V **COB** strip in **4000 K neutral white**, 4–5 mm wide,
  ≥ 320 chips/m, CRI ≥ 90 if you can get it, with a short cut unit (10–25 mm).
  COB gives a continuous line of light, so a thin diffuser looks even. 4000 K adds
  no colour cast; avoid ≥ 6000 K (blue, glary on mirrors). A 620 nm red strip is an
  option for keeping night vision.
* **Length:** `led_unit × led_units`. The model checks that it fits the chamber and
  leaves `wire_gap` for the wires. The default is 5 × 25 mm = 125 mm.
* **Power:** the strip runs from the camera board's **5V and GND pads**. These boards
  expose a `5V D- D+ GND` header; alternatively, splice into the red/black wires of
  the USB cable. The light is on whenever USB has power: no switch, no driver.
* **Current:** about 5 W/m strip at full brightness draws ~125 mA for 125 mm. That
  is far too bright and eats into the 500 mA a USB 2 port gives (the camera itself
  takes 150–250 mA). Put **10 Ω (¼ W) + a 100 Ω trimmer (3296-type, 0.5 W)** in
  series and set it once to **30–50 mA**. The trimmer sits in the head, reachable
  with the cap off. Once tuned you may replace it with a fixed resistor.

```
 camera board            head                         light chamber
 5V  ●──────── 10 Ω ──── 100 Ω trimmer ──┐
                                         ├── JST ──── + COB strip −
 GND ●───────────────────────────────────┘                     │
      └────────────────────────────────────────────────────────┘
```

* **Diffuser:** opal (milky white) cast acrylic. 2 mm is brighter (roughly 60–70 %
  transmission); 3 mm diffuses more evenly (roughly 40–60 %), so set the trimmer a
  little higher.

## 3. Export, print and cut

```sh
./build.sh                    # Linux/macOS, needs openscad on PATH
.\build.ps1                   # Windows PowerShell ($env:OPENSCAD = path\to\openscad.com if not on PATH)
.\build.ps1 -Extra '-D camera=\"ov9726_hbv\" -D barrel_od=50.7'
```

This writes `out/`:

| File | What |
|---|---|
| `barrel_2mm.stl`, `barrel_3mm.stl` | Barrel with flange and head floor. The acrylic recess depth matches the sheet thickness, so print the one for your acrylic. |
| `head.stl` | The tube around the camera carrier, with the three centring-screw bosses. |
| `carrier.stl`, `baffle.stl`, `retainer.stl`, `cap.stl` | The other printed parts, already in print orientation. |
| `tolerance_test.stl` | Seven barrel rings from −0.3 to +0.3 mm with the diameter engraved. **Print this first.** |
| `diffuser_2mm.dxf/.svg`, `diffuser_3mm.dxf/.svg` | The laser-cut ring. |

**Printing.** PETG or ASA, matte black. Use 4 perimeters for a round, stiff barrel.
Every STL is oriented to print without supports:

* The **barrel** prints flange-down. The carrier seat (the head floor) is the first
  layer, and the flange face that seats on the focuser is a flat top surface parallel
  to it. These two faces set the camera's tilt, which is why the barrel and head are
  separate parts: printed as one, one of the faces would be a sagging bridge.
  Remove any elephant's foot from the first layer, so the carrier sits flat.
* The **head** prints rim-down. Its three screw bosses overhang a little; add supports
  from the bed if your printer struggles.

Try the tolerance rings in your focuser and set `barrel_od` to the snug one.

**Laser cutting.** Import the DXF **in millimetres** (DXF has no units). `kerf` is
the full kerf width: the outer path is already offset outward by kerf/2 and the inner
path inward by kerf/2. **Turn off your laser software's own kerf/offset**, or it gets
compensated twice. Cast acrylic thickness varies by about ±0.2 mm: measure your sheet
and set `acrylic_t` to it before exporting the barrel.

## 4. Assembly

![Exploded cutaway of the adapter. From the top: cap, camera board, carrier, baffle, head
with a centring screw, barrel with the LED strip, acrylic ring and retainer.](img/assembly_exploded.png)

The blue numbers are the steps below; the arrows show which way each part goes in.
The quarter facing you is cut away. The left cut is on the wire side, and the right cut
goes through a centring screw. The drawing uses the default parameters; set
`part = "exploded"` in OpenSCAD to turn it around, and run `python make_figures.py`
(needs Pillow and NumPy) to redraw the figures after changing the model.

1. Glue the **head** onto the barrel's flange (CA or epoxy). The spigot locates it;
   line up the two marks (they show the wire-channel side). Melt M3 heat-set inserts
   into the three bosses. Fit M3×10 screws with nylon tips, or cap them with a drop of
   hot glue, so they don't dig into the carrier.
2. **Light chamber:** line the barrel's inside wall with white tape or paint it white,
   to mix the light. Stick the COB strip around the top of the chamber, leaving the gap
   at the sloped wire channel. Feed the two wires up through the channel into the head.
3. **Baffle:** paint or flock its bore matte black, and make its outside white
   (it is the inner wall of the light chamber). Drop it in from the head until its
   flange sits in the counterbore, just below the floor so the carrier rests on the
   floor only.
4. **Diffuser:** push the acrylic ring into the recess at the barrel bottom, around the
   baffle. Press the retainer onto the baffle's end until it clamps the ring against
   the ledge. The clamping force is axial only; the precision barrel never gets
   pushed out. Use a drop of glue if the press fit is loose.
5. **Camera:** fit the PCB into the carrier with the lens down through the floor
   (M2 self-tappers where the board has holes). Put the carrier in the head with its
   notch at the marks. Solder the LED wires through the resistor and
   trimmer to the board's 5V and GND pads.

   ![The head from above after step 5, with the cap off: the mark and the carrier notch
   on the wire side, and the three centring screws.](img/assembly_top.png)
6. Plug in USB, set the trimmer to 30–50 mA (measure with a multimeter), close the cap
   and zip-tie the cable for strain relief.

## 5. Centre the camera with ColliView

1. Insert the adapter. The flange must sit flat on the focuser. A compression-ring
   focuser is best; thumbscrews push the barrel to one side.
2. In ColliView, **Auto-detect** the drawtube rim.
3. Turn on **Image & view → Mark the sensor centre**, then turn the three carrier screws
   until the status bar's *centre vs image centre* reads close to 0, 0.
4. Run **Off-centre camera calibration** for the secondary and primary layers. It
   measures at 0° and 180° adapter rotation and stores the remaining camera bias as
   layer offsets. If it warns that the rim moved between the captures, the adapter
   isn't rotating about the focuser axis: re-seat it and tighten the same way both times.

## Sources

* OV9726 module: <https://www.aliexpress.com/item/1005004029465825.html>
* OV3660 module: <https://www.aliexpress.com/item/1005006543674079.html>
* IMX179 AF module (`DB05-AF-8MP`): <https://www.aliexpress.com/item/1005005334799549.html>
* Typical IMX179 AF lens (3.5 mm, F2.2, ~72° diagonal, 10 cm–∞), from DFRobot's FIT1022, a similar board: <https://littlebirdelectronics.com.au/products/imx179-8mp-autofocus-usb-camera-module-with-microphone-for-raspberry-pi-nvidia-lattepanda>
* Waveshare OV5640 USB (A), autofocus with no documented manual focus: <https://www.waveshare.com/wiki/OV5640_5MP_USB_Camera_(A)>
* Arducam UVC selection guide: <https://docs.arducam.com/UVC-Camera/USB2-UVC-Camera-Kit/Specs-and-Selection-Guide/>
* Arducam AMCap focus-mode notes: <https://docs.arducam.com/UVC-Camera/Appilcation-Note/Software-Instruction/Windows-AMCap/>
* Opal acrylic: <https://www.perspex.co.uk/materials/perspex-cast-acrylic/opal-white/>
