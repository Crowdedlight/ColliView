# Paper target for centring the camera

[`centring_target.pdf`](centring_target.pdf) is an A4 page of black and white squares that
never repeats. ColliView's **Off-centre camera calibration → Paper target** matches
it between a capture at 0° and one at 180° adapter rotation, to centre the lens on the
focuser axis with the adapter's carrier screws. The squares are 5 mm, grouped in blocks
that are mostly black or mostly white, so the pattern has structure at every scale.

## Using it

1. Print the PDF on matte paper, not glossy, at any scale. Neither the size nor the
   print scale matters.
2. Tape it flat inside the tube, in front of the focuser, facing it. Put it as far down
   the tube as you can: the further away it is, the more an off-axis lens shows. Taping
   it to the secondary's face, or seeing it in the secondary, works too.
3. Light it with the adapter's light ring and check that the camera sees the squares
   through the drawtube.
4. In ColliView, fit the yellow focuser ring to the drawtube: **Auto-detect** (D) and
   Accept, or drag it there. Roughly is enough; it only masks out the drawtube. Then open
   **Off-centre camera calibration → Paper target** and follow the steps there.
5. When Live reads about 0, remove the paper and Auto-detect the drawtube again: the carrier
   screws have moved the picture.

Don't touch the paper between the captures. If it slides, the app can't tell: the two
views still match, just about the wrong point. The same goes for the adapter: seat it the
same way at 0° and 180°, since a sideways shift goes unnoticed too.

## Squared paper instead

Plain squared paper (graph paper) or a checkerboard looks the same after half a turn,
shifted by half a square, so the match is ambiguous and ColliView refuses it. Draw
5–10 bold, irregular marks with a thick felt pen, of different sizes and spread over
the area the camera sees, and it works like the printed target.

## What it measures, and what it assumes

Turning the adapter 180° turns the paper's image about one point: where the focuser
axis meets the paper. The bias is that point minus the sensor centre. It is zero when the
lens sits on the axis, provided the camera looks straight down the focuser: no tilt, and
the lens centred over the sensor. Neither can be told apart from an off-axis lens with
the paper alone. **Layer circle** on the secondary afterwards shows whatever is left over.
See the [hardware guide](../hardware/README.md#5-centre-the-camera-with-colliview).

## Regenerating the PDF

The pattern is defined in [`src/target.rs`](../src/target.rs), which the simulator also
renders. After changing it, run:

```sh
cargo run --example make_target
```
