// ColliView 2" camera adapter with a diffused LED light ring.
//
// Units are mm. The Z axis is the optical axis; z = 0 is the bottom face of the
// barrel (the telescope side). Every module is modelled in these "world"
// coordinates; the part dispatcher at the bottom moves each part into its print
// orientation.
//
// Export one part:   openscad -o barrel.stl -D 'part="barrel"' colliview_adapter.scad
// Laser-cut ring:    openscad -o diffuser.dxf -D 'part="diffuser_2d"' -D acrylic_t=3 colliview_adapter.scad
// Or run build.ps1 / build.sh to export everything.

/* [Part] */
part = "assembly"; // [assembly, section, exploded, top_view, barrel, head, carrier, baffle, retainer, cap, tolerance_test, diffuser_2d]

/* [Focuser fit] */
// 2" = 50.8 nominal; print tolerance_test first and pick the ring that fits. 1.25" = 31.6
barrel_od = 50.6;
// Insertion depth into the focuser
barrel_len = 32;
barrel_wall = 2.2;
// The flange's flat underside seats on the focuser and sets the tilt
flange_t = 4;
flange_extra = 5;

/* [Camera module] */
camera = "ov3660_hbv"; // [ov3660_hbv, ov9726_hbv, imx179_db05, generic_38, custom]
// Used when camera = "custom": PCB width, height, thickness
custom_pcb = [30, 25, 1.6];
// Mounting hole pitch x, y (0, 0 = no holes; corner supports are used instead)
custom_holes = [0, 0];
// Lens-holder footprint on the PCB
custom_holder = [14, 14];
// Lens centre relative to the PCB centre
custom_lens_offset = [0, 0];
// Widest part of lens/holder below the PCB (enters the baffle; the diagonal for a square one)
custom_lens_body_d = 14;
// PCB front face down to the front of the lens
custom_lens_drop = 12;

/* [Optics - measure on your telescope] */
// Front of the lens to the far end of the drawtube, at the usual focuser position
lens_to_rim = 115;
// Drawtube inner diameter
rim_d = 48;

/* [Centring] */
// Radial travel of the camera carrier
float = 1.5;
// Hole for an M3 heat-set insert
insert_d = 4.0;
insert_len = 5.0;
screw_boss_d = 9;

/* [Light ring] */
// Measured acrylic sheet thickness (2 or 3 mm sheets vary by about ±0.2)
acrylic_t = 3;
// Full laser kerf width. Turn OFF the laser software's own kerf offset.
kerf = 0.15;
// Radial clearance of the acrylic on each side
acrylic_clear = 0.2;
// LED strip cut-unit length, number of units and width
led_unit = 25;
led_units = 5;
led_strip_w = 5;
// Gap left in the ring for the wires
wire_gap = 8;

/* [Printing] */
// Interference of the retainer on the baffle tube
press_fit = 0.15;
$fn = 128;

// ------------------------------------------------------------------ presets
// [pcb [w, h, t], holes [x, y], holder [w, h], lens offset [x, y], lens body d, lens drop]
//
// imx179_db05: IMX179 8 MP autofocus board, silkscreen "DB05-AF-8MP" (AliExpress
// 1005005334799549). PCB and holes from the listing's drawing: 28 x 28 mm with four
// 2.5 mm holes on a 23 mm square (two of them open slots). Its lens is the compact
// voice-coil focus can (~9 x 9 mm, ~5 mm tall), estimated from photos. The drawing
// shows the lens centred, but the photos hint at ~1 mm towards the connector edge,
// so the holder cut-out is oversized to clear either.
function preset(name) =
    name == "ov3660_hbv" ? [[30, 25, 1.6], [0, 0], [14, 14], [0, 0], 14, 12] :
    name == "ov9726_hbv" ? [[32, 32, 1.6], [28, 28], [14, 14], [0, 0], 14, 12] :
    name == "imx179_db05" ? [[28, 28, 1.6], [23, 23], [11, 11], [0, 0], 14, 5] :
    name == "generic_38" ? [[38, 38, 1.6], [34, 34], [16, 16], [0, 0], 14, 12] :
    [custom_pcb, custom_holes, custom_holder, custom_lens_offset, custom_lens_body_d, custom_lens_drop];

P = preset(camera);
pcb = P[0];
holes = P[1];
holder = P[2];
lens_off = P[3];
lens_body_d = P[4];
lens_drop = P[5];

// ------------------------------------------------------------------ derived
bore_d = barrel_od - 2 * barrel_wall;          // light-chamber bore
recess_d = barrel_od - 2 * 1.2;                // acrylic recess; thin, but it carries no load
ledge_z = acrylic_t + 0.2;                     // the acrylic is pulled up against this ledge
head_floor_top = barrel_len + flange_t;        // the flange doubles as the head floor
head_wall = 3;
carrier_floor_t = 2;
boss_h = 1.5;
carrier_h = carrier_floor_t + boss_h + pcb[2] + 1.0;
carrier_od = max(barrel_od + 2, norm([pcb[0], pcb[1]]) + 4);
head_id = carrier_od + 2 * float;
head_od = max(head_id + 2 * head_wall, barrel_od + 2 * flange_extra);
head_h = carrier_h + 14;                       // connector, cable, resistor and trimmer
head_top = head_floor_top + head_h;
cap_t = 2.5;
cap_lip = 3;
spigot_h = 2;
spigot_w = 1.2;
spigot_clear = 0.15;

lens_z = head_floor_top + carrier_floor_t + boss_h - lens_drop;
retainer_t = 1.6;
baffle_bottom = 0.2 - retainer_t;              // the baffle protrudes past the acrylic
half_rim = atan((rim_d / 2) / lens_to_rim);
fov_needed = 2 * half_rim + 6;
baffle_id = max(lens_body_d + 2 * float + 1, 2 * (lens_z - baffle_bottom) * tan(half_rim + 1) + 2 * float);
baffle_wall = 1.6;
baffle_od = baffle_id + 2 * baffle_wall;
baffle_flange_d = baffle_od + 8;
baffle_flange_t = 2;

acrylic_od = recess_d - 2 * acrylic_clear;
acrylic_id = baffle_od + 2 * acrylic_clear;
strip_len = led_unit * led_units;
strip_room = PI * bore_d - wire_gap;
wire_in_r = bore_d / 2 - 1.8;
wire_out_r = head_id / 2 - 2;     // under the carrier's wire notch, clear of the spigot

assert(strip_len <= strip_room,
    str("LED strip (", strip_len, " mm) does not fit the chamber (", strip_room, " mm available): use fewer units"));
assert(acrylic_od - acrylic_id >= 8, "Diffuser ring is too narrow: the baffle is too wide for this barrel");
assert(baffle_flange_d / 2 + 1 < wire_in_r, "No room for the wire hole next to the baffle flange");
assert(lens_z > baffle_bottom + 5, "lens_drop is too large: the lens would stick out of the baffle");

echo(str("ColliView adapter: camera '", camera, "'"));
echo(str("  Required vertical FOV of the camera: >= ", round(fov_needed), " deg (drawtube rim spans ",
         round(2 * half_rim * 10) / 10, " deg)"));
echo(str("  Diffuser ring (laser path): OD ", acrylic_od + kerf, "  ID ", acrylic_id - kerf,
         "  -> finished OD ", acrylic_od, "  ID ", acrylic_id, "  thickness ", acrylic_t));
echo(str("  LED strip: ", strip_len, " mm of ", strip_room, " mm available"));
echo(str("  Baffle ID ", baffle_id, "  head OD ", head_od, "  overall height ", head_top + cap_t));

// ------------------------------------------------------------------ parts

// Barrel + flange. Printed with the head-side face of the floor on the bed, so the
// carrier seat is the first layer and the flange's seating face prints as a flat top
// surface: both tilt-critical faces are flat and parallel, with no bridges.
module barrel_part() {
    difference() {
        union() {
            cylinder(d = barrel_od, h = barrel_len);
            translate([0, 0, barrel_len]) cylinder(d = head_od, h = flange_t);
        }
        // Light chamber and the acrylic recess (which forms the ledge).
        translate([0, 0, -1]) cylinder(d = bore_d, h = barrel_len + 1);
        translate([0, 0, -1]) cylinder(d = recess_d, h = ledge_z + 1);
        // Baffle hole with a counterbore. The counterbore is 0.2 mm deeper than the
        // baffle's flange so the carrier rests only on the floor.
        translate([0, 0, barrel_len - 1]) cylinder(d = baffle_od + 0.4, h = flange_t + 2);
        translate([0, 0, head_floor_top - baffle_flange_t - 0.2])
            cylinder(d = baffle_flange_d + 0.4, h = baffle_flange_t + 1);
        // Sloped wire channel from the chamber up into the head.
        hull() {
            translate([wire_in_r, 0, barrel_len - 0.5]) cylinder(d = 2.6, h = 0.1, $fn = 24);
            translate([wire_out_r, 0, head_floor_top]) cylinder(d = 2.6, h = 0.5, $fn = 24);
        }
        // Groove for the head's spigot.
        translate([0, 0, head_floor_top - spigot_h - 0.3]) difference() {
            cylinder(d = head_id + 2 * (spigot_w + spigot_clear), h = spigot_h + 1);
            translate([0, 0, -1]) cylinder(d = head_id - 2 * spigot_clear, h = spigot_h + 3);
        }
        // Mark the wire channel on the flange rim.
        translate([head_od / 2, 0, head_floor_top - 1]) rotate([0, 0, 45]) cube([1.5, 1.5, 4], center = true);
    }
}

// The tube around the floating carrier, with the three centring screws. Located on
// the flange by a spigot and glued.
module head() {
    difference() {
        union() {
            translate([0, 0, head_floor_top]) cylinder(d = head_od, h = head_h);
            translate([0, 0, head_floor_top - spigot_h]) cylinder(d = head_id + 2 * spigot_w, h = spigot_h);
            for (a = [90, 210, 330]) rotate([0, 0, a])
                translate([head_od / 2 - 1, 0, head_floor_top + carrier_h / 2])
                    rotate([0, 90, 0]) cylinder(d = screw_boss_d, h = insert_len - head_wall + 2);
        }
        translate([0, 0, head_floor_top - spigot_h - 1]) cylinder(d = head_id, h = head_h + spigot_h + 2);
        // Radial centring screws (M3 into heat-set inserts from outside).
        for (a = [90, 210, 330]) rotate([0, 0, a])
            translate([head_id / 2 - 1, 0, head_floor_top + carrier_h / 2])
                rotate([0, 90, 0]) cylinder(d = insert_d, h = 20, $fn = 32);
        // Line this mark up with the one on the flange (wire channel side).
        translate([head_od / 2, 0, head_top - 3]) rotate([0, 0, 45]) cube([1.5, 1.5, 7], center = true);
    }
}

module carrier() {
    pocket = [pcb[0] + 0.4, pcb[1] + 0.4];
    translate([0, 0, head_floor_top]) difference() {
        union() {
            difference() {
                cylinder(d = carrier_od, h = carrier_h);
                // PCB pocket, placed so the lens sits on the optical axis.
                translate([-lens_off[0] - pocket[0] / 2, -lens_off[1] - pocket[1] / 2, carrier_floor_t])
                    cube([pocket[0], pocket[1], carrier_h]);
            }
            // PCB supports: bosses on the mounting holes, or corner pads.
            translate([-lens_off[0], -lens_off[1], 0]) {
                if (holes[0] > 0 && holes[1] > 0) {
                    for (sx = [-1, 1], sy = [-1, 1]) translate([sx * holes[0] / 2, sy * holes[1] / 2, 0])
                        cylinder(d = 4.5, h = carrier_floor_t + boss_h, $fn = 32);
                } else {
                    for (sx = [-1, 1], sy = [-1, 1])
                        translate([sx * (pocket[0] / 2 - 1.5) - 1.5, sy * (pocket[1] / 2 - 1.5) - 1.5, 0])
                            cube([3, 3, carrier_floor_t + boss_h]);
                }
            }
        }
        // Self-tapping M2 holes in the bosses.
        if (holes[0] > 0 && holes[1] > 0)
            for (sx = [-1, 1], sy = [-1, 1])
                translate([-lens_off[0] + sx * holes[0] / 2, -lens_off[1] + sy * holes[1] / 2, -1])
                    cylinder(d = 1.8, h = carrier_h + 2, $fn = 16);
        // Lens holder through the floor (the lens is on the axis).
        translate([-(holder[0] + 1) / 2, -(holder[1] + 1) / 2, -1]) cube([holder[0] + 1, holder[1] + 1, carrier_h + 2]);
        // Notch for the LED wires, lined up with the mark on the head.
        translate([carrier_od / 2 - 2, -2, -1]) cube([4, 4, carrier_h + 2]);
    }
}

module baffle() {
    h = head_floor_top - baffle_bottom;
    translate([0, 0, baffle_bottom]) difference() {
        union() {
            cylinder(d = baffle_od, h = h);
            translate([0, 0, h - baffle_flange_t]) cylinder(d = baffle_flange_d, h = baffle_flange_t);
        }
        translate([0, 0, -1]) cylinder(d = baffle_id, h = h + 2);
    }
}

// Clamps the acrylic's inner edge up against the ledge. Axial load only.
module retainer() {
    translate([0, 0, baffle_bottom]) difference() {
        cylinder(d = acrylic_id + 5, h = retainer_t);
        translate([0, 0, -1]) cylinder(d = baffle_od - press_fit, h = retainer_t + 2);
    }
}

module cap() {
    cable_d = 5;
    translate([0, 0, head_top]) difference() {
        union() {
            cylinder(d = head_od, h = cap_t);
            translate([0, 0, -cap_lip]) cylinder(d = head_id - 0.3, h = cap_lip);
        }
        translate([0, 0, -cap_lip - 1]) cylinder(d = head_id - 0.3 - 4, h = cap_lip + 1);
        translate([0, 0, -cap_lip - 1]) cylinder(d = cable_d, h = cap_lip + cap_t + 2, $fn = 32);
        // Zip-tie slots for strain relief.
        for (s = [-1, 1]) translate([-2, s * (cable_d / 2 + 3) - 0.75, -1]) cube([4, 1.5, cap_t + 2]);
    }
}

module tolerance_test() {
    steps = [-0.3, -0.2, -0.1, 0, 0.1, 0.2, 0.3];
    pitch = barrel_od + 6;
    for (i = [0 : len(steps) - 1]) {
        d = barrel_od + steps[i];
        translate([(i % 4) * pitch, floor(i / 4) * pitch, 0]) {
            difference() {
                cylinder(d = d, h = 10);
                translate([0, 0, -1]) cylinder(d = d - 4, h = 12);
            }
            // A bar across the ring carries the engraved diameter.
            difference() {
                translate([-(d - 2) / 2, -5, 0]) cube([d - 2, 10, 1.6]);
                translate([0, 0, 0.8]) linear_extrude(1) text(str(d), size = 5, halign = "center", valign = "center");
            }
        }
    }
}

// The laser-cut diffuser. Cut paths are offset by half the kerf on each side.
module diffuser_2d() {
    difference() {
        circle(d = acrylic_od + kerf, $fn = 360);
        circle(d = acrylic_id - kerf, $fn = 360);
    }
}

module assembly() {
    color("DimGray") barrel_part();
    color("DarkSlateGray") head();
    color("WhiteSmoke") baffle();
    color("SlateGray") carrier();
    color("Gray") retainer();
    color("DimGray", 0.5) cap();
    // Acrylic ring and LED strip (not printed).
    color("White", 0.7) translate([0, 0, 0.2]) linear_extrude(acrylic_t) diffuser_2d();
    color("Gold") translate([0, 0, barrel_len - led_strip_w - 3]) difference() {
        cylinder(d = bore_d, h = led_strip_w);
        translate([0, 0, -1]) cylinder(d = bore_d - 3, h = led_strip_w + 2);
    }
    // Camera PCB and lens (for reference).
    color("ForestGreen") translate([-lens_off[0] - pcb[0] / 2, -lens_off[1] - pcb[1] / 2, head_floor_top + carrier_floor_t + boss_h])
        cube([pcb[0], pcb[1], pcb[2]]);
    color("Black") translate([0, 0, lens_z]) cylinder(d = lens_body_d - 2, h = head_floor_top + carrier_floor_t + boss_h - lens_z);
}

// ------------------------------------------------------------------ figures
// The README's assembly drawing: an exploded cutaway and the head seen from above with
// the cap off. make_figures.py renders them and labels the points that exploded() and
// top_view() echo as "anchor". The bought parts are simple stand-ins.
fig_gap = 10;
fig_cut = "Bisque";                                    // colour of the cut faces
pcb_z = head_floor_top + carrier_floor_t + boss_h;     // underside of the camera PCB
header_h = 2.5;
boss_out_r = head_od / 2 - 1 + insert_len - head_wall + 2;
screw_z = head_floor_top + carrier_h / 2;
strip_z = barrel_len - led_strip_w - 3;

fig_dz_acrylic = -fig_gap - 0.2 - acrylic_t;
fig_dz_retainer = fig_dz_acrylic - fig_gap;
fig_dz_head = spigot_h + fig_gap;
fig_dz_baffle = head_top + fig_dz_head + fig_gap - baffle_bottom;
fig_dz_carrier = fig_dz_baffle + fig_gap;
fig_dz_camera = head_floor_top + carrier_h + fig_dz_carrier + fig_gap - lens_z;
fig_dz_cap = pcb_z + pcb[2] + header_h + fig_dz_camera + fig_gap - (head_top - cap_lip);

// Cuts away the quarter facing the viewer (0..90 deg): the cut at 0 deg shows the wire
// channel and the marks, the cut at 90 deg a centring screw.
module cutaway(c) {
    difference() {
        color(c) children();
        color(fig_cut) translate([0, 0, -500]) cube([500, 500, 1000]);
    }
}

module anchor(name, p) echo("anchor", name, p);
module arrow(from, to) echo("arrow", from, to);
function polar(a, r, z) = [r * cos(a), r * sin(a), z];

// A chain of rods through the points: wires and cables.
module rod(pts, d) {
    for (i = [0 : len(pts) - 2]) hull() {
        translate(pts[i]) sphere(d = d, $fn = 12);
        translate(pts[i + 1]) sphere(d = d, $fn = 12);
    }
}

// Along +z, tip at z = 0: M3 x 10 socket-head screw with a nylon tip.
module m3_screw() {
    color("White") cylinder(d = 2.8, h = 1.2, $fn = 24);
    color("Silver") difference() {
        union() {
            translate([0, 0, 1.2]) cylinder(d = 3, h = 8.8, $fn = 24);
            translate([0, 0, 10]) cylinder(d = 5.5, h = 3, $fn = 32);
        }
        translate([0, 0, 11]) cylinder(d = 2.9, h = 3, $fn = 6);
    }
}

// Along +z from z = 0: M3 heat-set insert.
module m3_insert() {
    color("Goldenrod") difference() {
        union() {
            cylinder(d = insert_d + 0.4, h = insert_len, $fn = 32);
            for (z = [1, insert_len - 2]) translate([0, 0, z]) cylinder(d = insert_d + 0.8, h = 1, $fn = 32);
        }
        translate([0, 0, -1]) cylinder(d = 2.6, h = insert_len + 2, $fn = 24);
    }
}

// Camera PCB, lens holder and lens in their assembled position, with the USB header on
// the wire side.
module camera_board() {
    holder_h = min(5, lens_drop);
    translate([-lens_off[0], -lens_off[1], 0]) {
        color("ForestGreen") translate([-pcb[0] / 2, -pcb[1] / 2, pcb_z]) cube(pcb);
        color("WhiteSmoke") translate([pcb[0] / 2 - 4, -5, pcb_z + pcb[2]]) cube([2.5, 10, header_h]);
    }
    color("#222") {
        translate([-holder[0] / 2, -holder[1] / 2, pcb_z - holder_h]) cube([holder[0], holder[1], holder_h]);
        if (lens_drop > holder_h)
            translate([0, 0, lens_z]) cylinder(d = min(holder[0], holder[1]) * 0.8, h = lens_drop - holder_h);
    }
}

// LED strip on the chamber wall, with its gap centred on the wire channel (0 deg). Only
// the part outside the cut-away quarter: OpenCSG draws a cut rotate_extrude poorly.
module led_strip() {
    a = strip_len / (PI * bore_d) * 360;
    color("Gold") translate([0, 0, strip_z]) rotate([0, 0, 91])
        rotate_extrude(angle = 89 + a / 2) translate([bore_d / 2 - 1.6, 0]) square([1.2, led_strip_w]);
}

// The strip's two wires, up the sloped channel to a JST plug above the flange.
module led_wires(top) {
    a = 180 + strip_len / (PI * bore_d) * 180;
    r = bore_d / 2 - 1;
    for (s = [-1, 1]) color(s > 0 ? "Red" : "#111")
        rod([[r * cos(a), r * sin(a), strip_z + led_strip_w / 2 + s * 0.6],
             [r * cos(a / 2 + 180), r * sin(a / 2 + 180), barrel_len - 2],
             [wire_in_r, s * 0.5, barrel_len - 0.5],
             [wire_out_r, s * 0.5, head_floor_top],
             [wire_out_r, s * 0.5, top]], 1);
    color("WhiteSmoke") translate([wire_out_r - 2, -3, top]) cube([4, 6, 5]);
}

module exploded() {
    // Barrel, white lining, LED strip and wires.
    cutaway("DimGray") barrel_part();
    cutaway("White") translate([0, 0, ledge_z]) difference() {
        cylinder(d = bore_d, h = barrel_len - ledge_z);
        translate([0, 0, -1]) cylinder(d = bore_d - 0.8, h = barrel_len);
    }
    led_strip();
    led_wires(head_floor_top + fig_gap * 0.4);
    // Diffuser and retainer, from below.
    translate([0, 0, fig_dz_acrylic]) cutaway("GhostWhite") translate([0, 0, 0.2]) linear_extrude(acrylic_t) diffuser_2d();
    translate([0, 0, fig_dz_retainer]) cutaway("Gray") retainer();
    // Head with one insert and screw pulled out along the boss at 90 deg.
    translate([0, 0, fig_dz_head]) {
        cutaway("DarkSlateGray") head();
        rotate([0, 0, 90]) translate([0, 0, screw_z]) rotate([0, 90, 0]) {
            translate([0, 0, boss_out_r + 5]) m3_insert();
            translate([0, 0, boss_out_r + insert_len + 12]) m3_screw();
        }
    }
    // Baffle: black bore, white outside.
    translate([0, 0, fig_dz_baffle]) {
        cutaway("#333") baffle();
        cutaway("White") translate([0, 0, baffle_bottom]) difference() {
            cylinder(d = baffle_od + 0.4, h = head_floor_top - baffle_flange_t - baffle_bottom);
            translate([0, 0, -1]) cylinder(d = baffle_od, h = head_floor_top);
        }
    }
    translate([0, 0, fig_dz_carrier]) cutaway("SlateGray") carrier();
    translate([0, 0, fig_dz_camera]) camera_board();
    translate([0, 0, fig_dz_cap]) cutaway("DimGray") cap();
    // USB cable from the header, out through the cap.
    hx = pcb[0] / 2 - 2.75 - lens_off[0];
    hz = pcb_z + pcb[2] + header_h + fig_dz_camera;
    color("#1a1a1a") rod([[hx, 0, hz], [hx, 0, hz + 3], [0, 0, hz + 8], [0, 0, head_top + cap_t + fig_dz_cap + 12]], 4);

    // Label points on visible surfaces: 330 deg is the left edge, 120 deg the right edge.
    anchor("cap", polar(330, head_od / 2, head_top + cap_t / 2 + fig_dz_cap));
    anchor("cable", [0, 0, head_top + cap_t + fig_dz_cap + 8]);
    anchor("camera", [pcb[0] / 2 - 3 - lens_off[0], -pcb[1] / 2 + 3 - lens_off[1], pcb_z + pcb[2] + fig_dz_camera]);
    anchor("carrier", polar(120, carrier_od / 2, head_floor_top + carrier_h / 2 + fig_dz_carrier));
    anchor("carrier_notch", [carrier_od / 2 - 2, 0, head_floor_top + carrier_h + fig_dz_carrier]);
    anchor("baffle", polar(120, baffle_od / 2, (baffle_bottom + head_floor_top) / 2 + fig_dz_baffle));
    anchor("head", polar(120, head_od / 2, head_floor_top + head_h * 0.6 + fig_dz_head));
    anchor("head_mark", [head_od / 2, 0, head_top - 2 + fig_dz_head]);
    anchor("insert", polar(90, boss_out_r + 5 + insert_len / 2, screw_z + fig_dz_head));
    anchor("screw", polar(90, boss_out_r + insert_len + 18, screw_z + fig_dz_head));
    anchor("flange_mark", [head_od / 2, 0, head_floor_top - 1.5]);
    anchor("wires", [(wire_in_r + wire_out_r) / 2, 0, (barrel_len + head_floor_top) / 2]);
    anchor("led_strip", polar(90, bore_d / 2 - 1, strip_z + 1));
    anchor("barrel", polar(120, barrel_od / 2, barrel_len * 0.4));
    anchor("diffuser", polar(330, acrylic_od / 2, 0.2 + acrylic_t / 2 + fig_dz_acrylic));
    anchor("retainer", polar(120, (acrylic_id + 5) / 2, baffle_bottom + retainer_t / 2 + fig_dz_retainer));

    // Which way each part goes in, through the gaps along the axis.
    function gap_arrow(top, bottom, down) = down ? [top - 2, bottom + 2] : [bottom + 2, top - 2];
    a_cam = gap_arrow(lens_z + fig_dz_camera, head_floor_top + carrier_h + fig_dz_carrier, true);
    a_car = gap_arrow(head_floor_top + fig_dz_carrier, head_floor_top + fig_dz_baffle, true);
    a_baf = gap_arrow(baffle_bottom + fig_dz_baffle, head_top + fig_dz_head, true);
    a_head = gap_arrow(head_floor_top - spigot_h + fig_dz_head, head_floor_top, true);
    a_acr = gap_arrow(0, 0.2 + acrylic_t + fig_dz_acrylic, false);
    a_ret = gap_arrow(0.2 + fig_dz_acrylic, 0.2 + fig_dz_retainer, false);
    for (a = [a_cam, a_car, a_baf, a_head, a_acr, a_ret]) arrow([0, 0, a[0]], [0, 0, a[1]]);
    arrow(polar(90, boss_out_r + insert_len + 10, screw_z + fig_dz_head), polar(90, boss_out_r + 1, screw_z + fig_dz_head));
}

// After step 5, seen from above with the cap off.
module top_view() {
    color("DimGray") barrel_part();
    color("DarkSlateGray") head();
    color("SlateGray") carrier();
    camera_board();
    for (a = [90, 210, 330]) rotate([0, 0, a]) translate([0, 0, screw_z]) rotate([0, 90, 0]) {
        translate([0, 0, boss_out_r - insert_len]) m3_insert();
        translate([0, 0, carrier_od / 2]) m3_screw();
    }
    for (s = [-1, 1]) color(s > 0 ? "Red" : "#111")
        rod([[wire_out_r, s * 0.6, head_floor_top], [wire_out_r, s * 0.6, head_floor_top + carrier_h + 2]], 1);

    anchor("tv_mark", [head_od / 2 - 0.5, 0, head_top]);
    anchor("tv_notch", [wire_out_r, 1.5, head_floor_top + carrier_h]);
    for (a = [90, 210, 330]) anchor(str("tv_screw_", a), polar(a, boss_out_r + 2.5, screw_z));
    anchor("tv_camera", [-pcb[0] / 4 - lens_off[0], pcb[1] / 4 - lens_off[1], pcb_z + pcb[2]]);
    anchor("tv_carrier", polar(160, carrier_od / 2 - 3, head_floor_top + carrier_h));
}

// ------------------------------------------------------------------ dispatch
// Flip a part upside down so that world height `top` ends up on the print bed.
module flipped(top) { translate([0, 0, top]) rotate([180, 0, 0]) children(); }

if (part == "assembly") assembly();
else if (part == "section") difference() { assembly(); translate([-100, 0, -50]) cube([200, 100, 300]); }
else if (part == "exploded") exploded();
else if (part == "top_view") top_view();
else if (part == "barrel") flipped(head_floor_top) barrel_part();   // carrier seat on the bed
else if (part == "head") flipped(head_top) head();                   // rim on the bed
else if (part == "carrier") translate([0, 0, -head_floor_top]) carrier();
else if (part == "baffle") flipped(head_floor_top) baffle();       // flange on the bed
else if (part == "retainer") translate([0, 0, -baffle_bottom]) retainer();
else if (part == "cap") flipped(head_top + cap_t) cap();           // flat top on the bed
else if (part == "tolerance_test") tolerance_test();
else if (part == "diffuser_2d") diffuser_2d();
else assert(false, str("unknown part '", part, "'"));
