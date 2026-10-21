# ColliView 2" camera adapter

This readme details the generated 3D model, how to generate the stl files and how to make your own preset for other camera adapters. 

## Camera

The default and only tested preset is `imx179_db05`: an IMX179 8 MP autofocus board bought from: https://www.aliexpress.com/item/1005005334799549.html 

Its measurements:
- A 28 × 28 × 1.12 mm PCB, with 2.5 mm holes on a 23 mm square.
- A centred voice-coil focus can, 8.46 × 8.46 × 4.5 mm.
- The connector's pins come through about 2.35 mm on the lens side.

## Camera presets

A preset is one line in `preset()`:

```openscad
// [pcb [w, h, t], holes [x, y], holder [w, h], lens offset [x, y], lens body d, lens drop]
function preset(name) =
    name == "imx179_db05" ? [[28, 28, 1.12], [23, 23], [8.5, 8.5], [0, 0], 12, 4.6] :
    name == "custom" ? [custom_pcb, custom_holes, ...] :
    assert(false, ...);
```

| Field | Meaning |
|---|---|
| `pcb` | PCB width, height and thickness. |
| `holes` | Mounting-hole pitch. `[0, 0]` means no holes: the PCB then sits on corner pads in a pocket with a `fit_gap` clearance. |
| `holder` | The lens holder's footprint on the PCB. It is cut through the carrier floor. |
| `lens offset` | The lens centre relative to the PCB centre. The pocket shifts so the lens sits on the center axis. |
| `lens body d` | The widest part of the lens or holder below the PCB (the diagonal, for a square one). It sizes the baffle. |
| `lens drop` | From the PCB's front face down to the front of the lens. Controls the lens height. |

To try a new board, set `camera = "custom"` and fill in the `custom_*` values, from the Customizer or with
`-D`. To keep it as a preset, add a line above the `custom` one and add its name to the `camera = ...; // [...]`
list. Measure the board with calipers for best results. For a board with holes, also set `pcb_insert` to suit its screws.

Asserts stop the export if the board doesn't fit: for example a lens that sticks out of the baffle, a
PCB boss that hits the lens holder, or the wire notch cutting into the pocket.

ColliView's jig defaults (16 / 44.4 / 6 / 100 mm) match the model's defaults. If the console's
`Centring jig: …` line shows other values, enter them under **Camera centring (jig) → Jig dimensions**.

## Light ring

- **Strip:** a 5V COB strip, 4000 K, 4–5 mm wide. Its length is `led_unit × led_units` (default 5 × 25 mm). The model checks that it fits the chamber.
- **Power:** Solder extra wires together with the camera boards USB cable for 5V and GND. You can add resistors to limit the current drawn by the light-ring, but for the few LEDs and my test I just went without resister as I could control the exposure and focus on camera to not get blinded. And the 500 mA available on USB port should be plenty to drive the few LEDs and the camera board.
  - Alternative you can also add a potentiometer in series and make a hole for it in the lid. That way you can adjust the brightness manually on the light-ring. 
- **Diffuser:** opal/opaque 3mm acrylic. 

## Export, print and cut
Script both for windows (ps1) and linux (sh). 

```sh
./build.sh                                   # needs openscad on PATH
.\build.ps1                                  # or set $env:OPENSCAD
.\build.ps1 -Extra '-D barrel_od=50.7'       # with overrides (./build.sh -D barrel_od=50.7)
```

These write `out/`:

| File | What |
|---|---|
| `tolerance_test.stl`, `fit_test.stl`, `cap_test.stl` | Print these first. Each test part is engraved with its value: set `barrel_od` to the ring that fits your focuser snugly, `fit_gap` to the ring the peg slides through without play, and `cap_squeeze` to the cap that holds firmly but still comes off. |
| `barrel_2mm.stl`, `barrel_3mm.stl` | Barrel, flange and head floor. Print the one that matches your acrylic. |
| `head.stl`, `carrier.stl`, `baffle.stl`, `retainer.stl`, `cap.stl` | The other parts, already in print orientation. |
| `jig.stl` | The centring jig. |
| `diffuser_{2,3}mm.{dxf,svg}` | The laser-cut ring. |

**Printing.** I recommend to use PETG with 4 perimeters. Nothing needs support technically but for a nicer finish you can add support to the screw holes cylinders on the head.stl. PLA can work too, but be mindful if you leave it around where sun can reach it, as it will deform in time. 

Colourwise I printed it all in grey, but if you want more contrast you can print the baffle in matte black. If you want more light you can coat the outer side of the baffle in something reflective or white, however I did not find that necessary. 

Print The barrel flange-down. Ensure that the barrel and the head is clean and smooth where they slot together so they sit flush and does not add tilt to the camera.   

**Carrier.** The default `carrier_fit = "fixed"` makes a carrier that fits the head with a `fixed_gap` (0.1 mm)
clearance, so the lens sits on the axis by itself, within about ±0.25 mm. With `"floating"`, the
carrier is smaller and is centred with three radial M3 screws. The head is the same for both.

**Inserts.** The screw holes are sized for [CNC Kitchen heat-set inserts][cnck] but most brands of heat inserts will fit. `pcb_insert` (default
M2x3) is for the PCB/camera board inserts into the carrier, and `centring_insert` (default M3x5.7) is for the heads centering screws. 

**Laser.** Import the DXF in millimetres. The paths already include `kerf`, so turn off your laser
software's own offset. Measure your acrylic sheet and set `acrylic_t` to it before exporting the barrel.

[cnck]: https://cnckitchen.store/cdn/shop/files/table_metric.png

## Assembly

![Exploded cutaway: cap, camera board, carrier, baffle, head, barrel with the LED strip, acrylic ring
and retainer.](img/assembly_exploded.png)

![Section of the assembled adapter.](img/assembly_section.png)

1. Glue the **head** onto the flange. Ensure that you don't place an adjustment screw where the hole is for the LED rings wires. For the floating carrier, also fit M3 inserts and nylon-tipped screws.
2. Glue the LED strip around the top of the chamber,
   and feed its wires up the sloped channel.
3. Drop the baffle from the head into the counterbore.
4. Push the **acrylic ring** into the bottom recess, and press the **retainer** onto the baffle. (I added a bit of glue to the retainer. It is press-fit, but to keep it more secure a bit of glue helps)
5. Melt M2 inserts into the carrier, screw on the board with the lens facing down, and press the
   carrier into the head with its hole for wires at where the LED wires comes up. (If you PCB board does not lie flush you might need to cut a little edge of one of the 3D printed standoffs, as the board can have resistors or caps that hit part of it)
6. Solder the LED wires to 5V and GND. If you decided to add a resistor or potentiometer then add it first.
7. Try and plug it in to ensure everything works before fitting the cap on and zip-tie the cable. To take the cap off, lever it out at its notches.

![The head from above with the cap off.](img/assembly_top.png)

Set `part = "exploded"`, `"section"` or `"top_view"` to view these in OpenSCAD. `python make_figures.py`
(needs Pillow and NumPy) redraws the figures based on default preset.

## Camera centring check

Ideally you would want the sensor of the camera sensor in the middle of your barrel so it would match your astro or DSLR camera. Minor offsets is alright with most telescopes as you set the center-point for the concentric circles based on the focuser tube anyway.

But if you want to minimize the error of the camera sitting outside the axis of the focuser and use the floating carrier with nylon-tipped screws then you can print the jig.stl and put it on a backlight screen like a phone on a white background. Looking through the live-view after clicking "to image center" will put the rings on the center of your camera sensor. You can then adjust the screws until the center is concentric in the middle of the view. 

A tenth or two of a millimetre is fine at about f/5; Use a compression-ring focuser if you can to avoid it being pushed to one side by a screw.

`jig.stl` holds the adapter by its barrel. Its two holes on the axis line up, as seen through the
camera, when the lens is centred. The model prints its dimensions as `Centring jig: …`.

1. Stand the jig on a backlight, such as a phone screen, and put the adapter in.
2. In ColliView, open **Camera centring (jig)**. Click **Find aperture**, **Accept** the preview,
   and tick **Live**.
3. Loosen the board's screws and nudge it the way the arrow points, until the offset turns green
   (< 0.05 mm). Then tighten the screws and check again. Some cameras mirror the image, so check the
   arrow's direction with a small nudge first.
4. Turn the adapter 180° and average the two readings: that cancels any print error in the jig.

<img width="1801" height="952" alt="Image" src="https://github.com/user-attachments/assets/034aab5d-d315-44b6-88b7-5ee9e891f608" />