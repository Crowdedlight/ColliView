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
> `imx179_db05` is measured on the board: a 28 × 28 × 1.12 mm PCB with 2.5 mm holes on
> a 23 mm square, and a centred 8.46 × 8.46 × 4.5 mm focus can. The connector's pins
> come through on the lens side, about 2.35 mm: they clear the 3 mm stand-off (the
> height of the insert bosses), but trim them if yours are longer, or the board won't
> sit flat on them.

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
| `cap_test.stl` | Three caps with crush ribs at `cap_squeeze` ± 0.15 mm (0.25, 0.4, 0.55 by default), each engraved with its value inside. Try each in the head and keep the one that holds firmly but still comes off; set `cap_squeeze` to its value for later builds. Print it instead of `cap.stl`. |
| `tolerance_test.stl` | Seven barrel rings from −0.3 to +0.3 mm with the diameter engraved. **Print this first.** |
| `fit_test.stl` | A peg and five rings engraved with their total gaps, `fit_gap` ± 0.2 mm (0–0.35 mm by default). **Print this first** too. |
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

**Fits.** `fit_gap` is the total gap (hole Ø − part Ø) between printed parts that slide
together, so each side gets half of it. The default is 0.15 mm. It applies to the head's
spigot in the flange groove, the baffle in the barrel floor and its counterbore, and
the camera PCB in the carrier pocket. On a board with mounting holes
the screws locate the PCB, so its pocket gets 3 × `fit_gap` (28.45 mm for the 28 mm
`imx179_db05`). The pocket has round reliefs in its corners, since printed inside corners
round off and would catch the PCB's sharp corners. Push the fit-test peg through the rings and set `fit_gap` to the ring it
passes through without play. Most of these fits have one side
on the first layer, so `bed_chamfer` (0.4 mm) bevels those edges against elephant's foot.
These are not slide fits and have their own settings:
* the retainer is a press fit (`press_fit`)
* the cap is held by eight crush ribs on its lip (`cap_squeeze`); the lip itself has the
  slide fit, so it centres the cap
* the carrier floats in the head by `float`
* the barrel is tuned to the focuser with `barrel_od`
* the insert holes follow the insert maker's table
* the laser-cut acrylic has `acrylic_clear` per side

**Heat-set inserts.** The screw holes are sized for [CNC Kitchen inserts][cnck]. From
their table, each hole is D2 wide and at least L + 1 mm deep, with at least W plus
`insert_margin` of plastic around it. `centring_insert` (default `M3x5.7`) sets the
three centring screws, and `pcb_insert` (default `M2x3`) the camera board's screws.
The table's other M2–M4 sizes can be picked as well. The console prints the screws
to buy, e.g. `Centring screws: M3x5.7 inserts, M3 x 12 screws`. The PCB bosses
have blind holes, closed `insert_skin` above the carrier's underside, so no melt can
come out where the carrier slides on the floor. On boards with mounting holes this
puts the PCB a little higher than on corner pads.

[cnck]: https://cnckitchen.store/cdn/shop/files/table_metric.png

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
goes through a centring screw.

![Section of the assembled adapter, each part's cut face in its own colour: the cap's lip
in the head, the camera board in the carrier, the head's spigot in the flange's groove,
a centring screw against the carrier, the baffle's flange in the counterbore, and the
acrylic ring and retainer at the bottom.](img/assembly_section.png)

The section shows the same parts assembled and cut in half. Each part's cut face has its
own colour, so you can see how the parts fit into each other. Again the left half is cut
on the wire side and the right half through a centring screw. The drawings use the
default parameters; set `part = "exploded"` or `part = "section"` in OpenSCAD to turn
them around, and run `python make_figures.py` (needs Pillow and NumPy) to redraw the
figures after changing the model.

1. Glue the **head** onto the barrel's flange (CA or epoxy). The spigot locates it;
   line up the two marks (they show the wire-channel side). Melt M3×5.7 heat-set inserts
   into the three bosses from outside, flush with the boss faces. Fit M3×12 screws with
   nylon tips, or cap them with a drop of hot glue, so they don't dig into the carrier.
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
5. **Camera:** if the board has mounting holes, melt M2×3 heat-set inserts into the
   carrier's bosses, flush with their tops. Fit the PCB into the carrier with the lens
   down through the floor, and fix it with M2×5 screws (or M2×4). Put the carrier in
   the head with its notch at the marks. Solder the LED wires through the resistor and
   trimmer to the board's 5V and GND pads. Keep the trimmer and the JST plug away from
   the head's wall, or lay the trimmer flat: the cap's lip reaches 5 mm down inside the
   wall.

   ![The head from above after step 5, with the cap off: the mark and the carrier notch
   on the wire side, and the three centring screws.](img/assembly_top.png)
6. Plug in USB, set the trimmer to 30–50 mA (measure with a multimeter), push the cap in
   until it sits on the head's rim, and zip-tie the cable for strain relief. The crush
   ribs on its lip hold it without glue. To open it, twist a small flat screwdriver in
   one of the two notches at the rim. If it gets loose after many openings, reprint it
   with a larger `cap_squeeze`. A hard tug on the cable pops the cap off rather than
   pulling on the board's header.

## 5. Centre the camera with ColliView

1. Insert the adapter. The flange must sit flat on the focuser. A compression-ring
   focuser is best; thumbscrews push the barrel to one side.
2. In ColliView, **Auto-detect** the drawtube rim.
3. **Rough:** turn on **Image & view → Mark the sensor centre**, then turn the three
   carrier screws until the status bar's *centre vs image centre* is within a few % of
   the frame. It doesn't need to be closer.
4. **Fine:** in **Off-centre camera calibration**, pick the **secondary** layer, capture at
   0°, turn the adapter 180° in the focuser and capture again. Leave the adapter there,
   tick **Live**, and turn the carrier screws until *Camera bias now* reads about 0 (it
   turns green). The lens now sits on the focuser axis.
5. Untick **Live** (this clears the old captures) and calibrate each layer (secondary,
   then primary) at 0° and 180°. Apply the small leftover bias as the layer's offset. If it warns that the rim
   moved between the captures, the adapter isn't rotating about the focuser axis:
   re-seat it and tighten the same way both times.

**Why not just centre the rim on the sensor?** The carrier screws only move the camera
sideways; how much it tilts is set by the print. A slightly tilted camera with the rim
on the sensor centre ends up off the axis, so features deeper in the tube look shifted
by a different amount than the rim (parallax). With the lens on the axis, like the
peephole of a Cheshire, the tilt only shifts the whole picture evenly, and the circles
follow the detected rim anyway. The camera bias that the 0°/180° calibration measures is
exactly this off-axis error, so it is the right thing to null.

**The telescope's own tilt can't get into the calibration.** The rim is the far end of
the drawtube, which the adapter sits in, so it tilts with the focuser. And between the
0° and 180° captures everything fixed to the telescope (secondary, primary and focuser
tilt) flips sign and cancels; only what turns with the camera is kept.

## Sources

* OV9726 module: <https://www.aliexpress.com/item/1005004029465825.html>
* OV3660 module: <https://www.aliexpress.com/item/1005006543674079.html>
* IMX179 AF module (`DB05-AF-8MP`): <https://www.aliexpress.com/item/1005005334799549.html>
* Typical IMX179 AF lens (3.5 mm, F2.2, ~72° diagonal, 10 cm–∞), from DFRobot's FIT1022, a similar board: <https://littlebirdelectronics.com.au/products/imx179-8mp-autofocus-usb-camera-module-with-microphone-for-raspberry-pi-nvidia-lattepanda>
* Waveshare OV5640 USB (A), autofocus with no documented manual focus: <https://www.waveshare.com/wiki/OV5640_5MP_USB_Camera_(A)>
* Arducam UVC selection guide: <https://docs.arducam.com/UVC-Camera/USB2-UVC-Camera-Kit/Specs-and-Selection-Guide/>
* Arducam AMCap focus-mode notes: <https://docs.arducam.com/UVC-Camera/Appilcation-Note/Software-Instruction/Windows-AMCap/>
* Opal acrylic: <https://www.perspex.co.uk/materials/perspex-cast-acrylic/opal-white/>
* CNC Kitchen heat-set insert dimensions: <https://cnckitchen.store/cdn/shop/files/table_metric.png>
