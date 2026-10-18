# ColliView 2" camera adapter

A 3D-printed 2" adapter that holds a small USB camera module on the focuser axis,
with a diffused LED light ring around the lens, similar to an OCAL. Everything is
parametric in [`colliview_adapter.scad`](colliview_adapter.scad). The laser-cut
acrylic diffuser is exported from the same model, so all parts always match.

```
            cable + zip tie
        ┌──────── cap ────────┐
        │  trimmer, resistor  │   head: camera carrier pressed into the bore
        │ ┌─── carrier ─────┐ │◄─ (or floating, on 3 radial M3 centring screws)
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
| `carrier.stl` | The camera carrier, fixed by default (see *Fixed or floating carrier* below). |
| `baffle.stl`, `retainer.stl`, `cap.stl` | The other printed parts, already in print orientation. |
| `cap_test.stl` | Three caps with crush ribs at `cap_squeeze` ± 0.15 mm (0.25, 0.4, 0.55 by default), each engraved with its value inside. Try each in the head and keep the one that holds firmly but still comes off; set `cap_squeeze` to its value for later builds. Print it instead of `cap.stl`. |
| `tolerance_test.stl` | Seven barrel rings from −0.3 to +0.3 mm with the diameter engraved. **Print this first.** |
| `fit_test.stl` | A peg and five rings engraved with their total gaps, `fit_gap` ± 0.2 mm (0–0.35 mm by default). **Print this first** too. |
| `jig.stl` | The centring jig (§5). It stands on a backlight with the adapter in it. |
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
* the fixed carrier is 0.1 mm (`fixed_gap`, on the diameter) smaller than the head's bore;
  the floating one floats in it by `float`
* the barrel is tuned to the focuser with `barrel_od`
* the insert holes follow the insert maker's table
* the laser-cut acrylic has `acrylic_clear` per side

**Fixed or floating carrier.** By default (`carrier_fit = "fixed"`) the carrier fills the
head's bore, `fixed_gap` (0.1 mm) smaller than it, so the lens sits on the axis by itself.
Its bottom edge is bevelled against elephant's foot, which would otherwise make it bind.
The board's screws locate it within about ±0.25 mm. The centring jig (§5) checks the lens
position, and lets you trim it within that play. The head is
the same for both fits, so a head printed for the floating carrier takes the fixed one;
leave its centring screws out. With `carrier_fit = "floating"` the carrier is 3 mm smaller
and is centred with the three screws, but their tips can dent the print, which makes fine
adjustment hard.

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
   line up the two marks (they show the wire-channel side). The fixed carrier needs
   nothing else here. For the floating carrier, melt M3×5.7 heat-set inserts into the three
   bosses from outside, flush with the boss faces, and fit M3×12 screws with nylon tips, or
   cap them with a drop of hot glue, so they don't dig into the carrier.
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
   down through the floor, and fix it with M2×5 screws (or M2×4). Press the carrier into
   the head, notch at the marks, down onto the floor. Solder the LED wires through the resistor and
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

## 5. Check the camera centring

**How much it matters.** ColliView fits the shared centre to the drawtube rim. A sensor
that sits a few pixels off the lens, or a slightly tilted camera, shifts every feature by
the same amount, so neither makes any difference. What counts is how far the *lens* sits
off the focuser axis, in millimetres: then nearer and farther features shift by different
amounts. Through the usual steps that works out as follows:

* **Centring the secondary** under the drawtube: off by less than the lens offset, which
  doesn't matter.
* **Aiming the secondary** (centre spot on the rim's centre): off by about 8 × the lens offset on
  the primary. That only tilts the eyepiece's view by hundredths of a degree.
* **Tilting the primary**, the adjustment that matters: off by about **half the lens offset**
  at the focal plane, like a Cheshire whose peephole is off-centre.

Play of the barrel in the focuser counts in full, though, because the light ring moves
with it. So the primary ends up off by about lens offset / 2 + barrel play. The usual
tolerance shrinks with the cube of the f-ratio. A tenth or two of a millimetre is fine
around f/5, while f/4 and faster leave much less.

The fixed carrier holds the lens within about ±0.25 mm. That covers the play of the M2
screws in the board's holes, and the lens's own placement on the board. So:

1. **Seat the barrel well.** A compression-ring focuser is best; thumbscrews push the
   barrel to one side. Take `barrel_od` from `tolerance_test.stl`.
2. **Check, and if needed trim, the lens position with the jig** below.

**The centring jig** (`jig.stl`) holds the adapter by its barrel, like the focuser does. It
has two holes on its axis: a Ø16 mm aperture just below the barrel, and a Ø6 mm hole in its
floor. Seen through the camera, their centres line up exactly when the lens is on the
barrel's axis, whatever the tilt or the sensor's offset. A lens offset shows up at about
30 px per mm at full resolution.

1. Print `jig.stl` floor-down; it needs no supports. The model's console prints the jig's
   dimensions (`Centring jig: …`). With non-default settings, enter them in ColliView under
   **Camera centring (jig) → Jig dimensions**.
2. Stand the jig on a backlight, such as a phone showing a white screen or a light pad. Put
   the adapter in, with the flange on the rim.
3. In ColliView, open **Camera centring (jig)**, click **Find aperture** and Accept the
   preview, so the yellow ring goes on the aperture. Plain Auto-detect can lock onto the
   baffle's edge just outside it instead. If the aperture's edge is too dark, dim the
   backlight or set the exposure by hand.
4. Tick **Live**. It reads the lens offset in mm, and an arrow in the view shows which way
   to push the board. Check the arrow with a small nudge first, as some cameras mirror the
   image.
5. Loosen the board's M2 screws a little. Nudge the board until the offset reads under
   about 0.05 mm (it turns green), then tighten the screws. Tightening can move it, so check
   again.
6. Turn the adapter 180° in the jig and check once more. A hole printed slightly off-centre
   flips with the turn, but the lens offset doesn't, so the average of the two readings is
   the true offset. Seat the adapter the same way each time: play in the socket doesn't
   average out.

With the floating carrier, turn its centring screws instead. Give them nylon tips, or cap
them with hot glue.

**Focuser checks**

* **Drawtube wobble.** With the scope collimated, rack the focuser in and out, and tighten
  and loosen its lock, while watching the centre spot against the shared centre. If the spot
  jumps, the drawtube moves sideways in the focuser. That counts in full, like barrel play.
* **Focuser tilt.** The secondary's tilt makes up for a slightly tilted focuser: it aims
  the focuser axis at the primary's centre whatever the focuser's angle. So the tilt rarely
  needs fixing. A strongly tilted focuser shows up as a secondary that can't be centred in the
  drawtube.

  To check it, remove the secondary or turn it out of view. Tape a cross on the inside of the
  tube wall, directly opposite the focuser's centre: the same distance from the tube's front
  end, and half the circumference round. Turn on the crosshair layer; the cross should sit
  on the shared centre. To get the tilt angle, take the cross's offset as a share of the
  yellow ring's radius, and multiply by about 12°. That is the ring's angular radius with
  the default `lens_to_rim` and `rim_d`.

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
