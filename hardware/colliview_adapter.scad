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
part = "assembly"; // [assembly, section, exploded, top_view, barrel, head, carrier, baffle, retainer, cap, cap_test, tolerance_test, fit_test, diffuser_2d]

/* [Focuser fit] */
// 2" = 50.8 nominal; print tolerance_test first and pick the ring that fits. 1.25" = 31.6
barrel_od = 50.9;
// Insertion depth into the focuser
barrel_len = 32;
barrel_wall = 2.2;
// The flange's flat underside seats on the focuser and sets the tilt
flange_t = 4;
flange_extra = 5;

/* [Camera module] */
camera = "imx179_db05"; // [ov3660_hbv, ov9726_hbv, imx179_db05, generic_38, custom]
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
// Heat-set insert for the PCB screws, on boards with mounting holes
pcb_insert = "M2x3"; // [M2x3, M2.5x4, M3x3, M3x5.7, M3x5x4, M4x4, M4x8.1]

/* [Optics - measure on your telescope] */
// Front of the lens to the far end of the drawtube, at the usual focuser position
lens_to_rim = 115;
// Drawtube inner diameter
rim_d = 48;

/* [Centring] */
// fixed: the carrier fills the head's bore, so the lens sits on the axis by itself (this
// assumes the sensor is centred under the lens). floating: it slides in the head and is
// centred with three screws.
carrier_fit = "fixed"; // [fixed, floating]
// Total gap (head bore - carrier diameter) of the fixed carrier
fixed_gap = 0.1;
// Radial travel of the floating carrier. It also sets the head's bore, for both fits.
float = 1.5;
// Heat-set insert for the three centring screws
centring_insert = "M3x5.7"; // [M2x3, M2.5x4, M3x3, M3x5.7, M3x5x4, M4x4, M4x8.1]

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
// Diameter of the sloped wire channel from the chamber up into the head
wire_hole_d = 4;

/* [Printing] */
// Total gap (hole diameter - part diameter, half of it per side) between printed parts that slide together; print fit_test to find yours
fit_gap = 0.15;
// 45 deg chamfer where a fit starts on the first layer, against elephant's foot
bed_chamfer = 0.4;
// Interference of the retainer on the baffle tube
press_fit = 0.15;
// Interference of the cap's crush ribs in the head (on the diameter, over a fit without play); print cap_test to find yours
cap_squeeze = 0.4;
// Added to the insert maker's minimum wall around an insert hole
insert_margin = 0.3;
// Plastic left past the end of an insert hole: closes the PCB bosses' blind holes, and
// is the stop in the centring bosses
insert_skin = 1;
$fn = 128;

// ------------------------------------------------------------------ presets
// [pcb [w, h, t], holes [x, y], holder [w, h], lens offset [x, y], lens body d, lens drop]
//
// imx179_db05: IMX179 8 MP autofocus board, silkscreen "DB05-AF-8MP" (AliExpress
// 1005005334799549). Measured on the board: the PCB is 28 x 28 x 1.12 mm with four
// 2.5 mm holes on a 23 mm square (two of them open slots). The voice-coil focus can is
// 8.46 x 8.46 mm and 4.5 mm tall plus ~0.1 mm of lens, and it sits centred. The
// connector's pins stick out 2.35 mm on the lens side, under the 3 mm stand-off.
function preset(name) =
    name == "ov3660_hbv" ? [[30, 25, 1.6], [0, 0], [14, 14], [0, 0], 14, 12] :
    name == "ov9726_hbv" ? [[32, 32, 1.6], [28, 28], [14, 14], [0, 0], 14, 12] :
    name == "imx179_db05" ? [[28, 28, 1.12], [23, 23], [8.5, 8.5], [0, 0], 12, 4.6] :
    name == "generic_38" ? [[38, 38, 1.6], [34, 34], [16, 16], [0, 0], 14, 12] :
    [custom_pcb, custom_holes, custom_holder, custom_lens_offset, custom_lens_body_d, custom_lens_drop];

P = preset(camera);
pcb = P[0];
holes = P[1];
holder = P[2];
lens_off = P[3];
lens_body_d = P[4];
lens_drop = P[5];

// CNC Kitchen heat-set inserts: [thread, L, D1, D2, W] from their design table
// (https://cnckitchen.store/cdn/shop/files/table_metric.png). L is the insert length,
// D1 its diameter, D2 the hole diameter and W the minimum wall around the hole. The
// hole is at least L + 1 deep.
function insert_spec(name) =
    name == "M2x3" ? [2, 3.0, 3.6, 3.2, 1.3] :
    name == "M2.5x4" ? [2.5, 4.0, 4.6, 4.0, 1.6] :
    name == "M3x3" ? [3, 3.0, 4.6, 4.0, 1.6] :
    name == "M3x5.7" ? [3, 5.7, 4.6, 4.0, 1.6] :
    name == "M3x5x4" ? [3, 4.0, 5.0, 4.4, 1.6] :   // Voron
    name == "M4x4" ? [4, 4.0, 6.3, 5.7, 2.1] :
    name == "M4x8.1" ? [4, 8.1, 6.3, 5.7, 2.1] :
    assert(false, str("unknown insert '", name, "'"));

function ins_thread(i) = i[0];
function ins_len(i) = i[1];
function ins_od(i) = i[2];
function ins_hole_d(i) = i[3];
function ins_hole_depth(i) = i[1] + 1;
function ins_boss_d(i) = i[3] + 2 * (i[4] + insert_margin);

centring_ins = insert_spec(centring_insert);
pcb_ins = insert_spec(pcb_insert);

screw_lengths = [3, 4, 5, 6, 8, 10, 12, 14, 16, 20, 25, 30];
function screw_at_least(l) = min([for (s = screw_lengths) if (s >= l) s]);
function screw_at_most(l) = max([for (s = screw_lengths) if (s <= l) s]);

// ------------------------------------------------------------------ derived
bore_d = barrel_od - 2 * barrel_wall;          // light-chamber bore
recess_d = barrel_od - 2 * 1.2;                // acrylic recess; thin, but it carries no load
ledge_z = acrylic_t + 0.2;                     // the acrylic is pulled up against this ledge
head_floor_top = barrel_len + flange_t;        // the flange doubles as the head floor
head_wall = 3;
carrier_floor_t = 2;
// PCB supports. On mounting holes they are bosses with blind insert holes, tall enough
// that insert_skin closes them: melt must not reach the carrier's sliding underside.
has_holes = holes[0] > 0 && holes[1] > 0;
// The screws locate a board with mounting holes, so its pocket only has to clear it.
pcb_gap = (has_holes ? 3 : 1) * fit_gap;
boss_h = has_holes ? max(1.5, ins_hole_depth(pcb_ins) + insert_skin - carrier_floor_t) : 1.5;
pcb_boss_d = ins_boss_d(pcb_ins);
pcb_boss_xy = has_holes ? [for (sx = [-1, 1], sy = [-1, 1])
    [-lens_off[0] + sx * holes[0] / 2, -lens_off[1] + sy * holes[1] / 2]] : [];
carrier_h = carrier_floor_t + boss_h + pcb[2] + 1.0;
carrier_od = max(barrel_od + 2, norm([pcb[0], pcb[1]]) + 4);
head_id = carrier_od + 2 * float;
// The fixed carrier fills the same bore, so a head printed for the floating one fits it.
carrier_d = carrier_fit == "fixed" ? head_id - fixed_gap : carrier_od;
head_od = max(head_id + 2 * head_wall, barrel_od + 2 * flange_extra);
head_h = carrier_h + 14;                       // connector, cable, resistor and trimmer
head_top = head_floor_top + head_h;
// Centring screws. Each boss holds the insert in a pocket from its outer face, with
// insert_skin left as a stop before the head's cavity. It sits high enough not to hang
// below the head onto the flange.
screw_boss_d = ins_boss_d(centring_ins);
screw_z = head_floor_top + max(carrier_h / 2, screw_boss_d / 2);
boss_out_r = max(head_od / 2, head_id / 2 + insert_skin + ins_hole_depth(centring_ins));
// Long enough to reach the carrier at full travel.
centring_screw_min = boss_out_r - (carrier_od / 2 - float);
centring_screw_len = screw_at_least(centring_screw_min);
// Through the PCB into the insert, short of the hole's bottom.
pcb_screw_max = pcb[2] + ins_hole_depth(pcb_ins);
pcb_screw_len = screw_at_most(pcb_screw_max);
cap_t = 2.5;
cap_lip = 5;
// Crush ribs on the cap's lip hold it in the head. They fade into the lip over
// cap_rib_lead at its bottom edge, so the cap starts easily.
cap_ribs = 8;
cap_rib_d = 2;
cap_rib_lead = 1.5;
spigot_h = 2;
spigot_w = 1.2;

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
wire_in_r = bore_d / 2 - wire_hole_d / 2 - 0.5;    // inside the bore, clear of the barrel wall
wire_out_r = head_id / 2 - wire_hole_d / 2 - 0.7;  // under the carrier's wire notch, clear of the spigot
wire_notch_w = wire_hole_d + 1.4;

assert(carrier_fit == "fixed" || carrier_fit == "floating", str("unknown carrier_fit '", carrier_fit, "'"));
assert(fixed_gap >= 0 && fixed_gap < 2 * float, "fixed_gap must be between 0 and the floating travel");
assert(strip_len <= strip_room,
    str("LED strip (", strip_len, " mm) does not fit the chamber (", strip_room, " mm available): use fewer units"));
assert(acrylic_od - acrylic_id >= 8, "Diffuser ring is too narrow: the baffle is too wide for this barrel");
assert(baffle_flange_d / 2 + 1 < wire_in_r - wire_hole_d / 2,
    "No room for the wire hole next to the baffle flange: use a smaller wire_hole_d");
assert(wire_out_r - wire_notch_w / 2 > max([-lens_off[0] + (pcb[0] + pcb_gap) / 2,
                                            for (c = pcb_boss_xy) c[0] + pcb_boss_d / 2]) + 1,
    "The carrier's wire notch cuts into the PCB pocket: use a smaller wire_hole_d");
assert(lens_z > baffle_bottom + 5, "lens_drop is too large: the lens would stick out of the baffle");
assert(screw_z - head_floor_top + ins_thread(centring_ins) / 2 <= carrier_h,
    "The centring screws would pass over the carrier: use a smaller centring_insert");
assert(head_top - cap_lip >= head_floor_top + carrier_h + 4,
    "The cap's lip reaches down to the camera board: use a shorter cap_lip");
for (c = pcb_boss_xy) {
    assert(max(abs(c[0]) - (holder[0] + 1) / 2, abs(c[1]) - (holder[1] + 1) / 2) >= pcb_boss_d / 2,
        "The lens holder cut-out cuts into a PCB boss: use a smaller pcb_insert");
    assert(norm(c) + pcb_boss_d / 2 <= carrier_od / 2, "A PCB boss sticks out of the carrier");
}

echo(str("ColliView adapter: camera '", camera, "'"));
echo(str("  Required vertical FOV of the camera: >= ", round(fov_needed), " deg (drawtube rim spans ",
         round(2 * half_rim * 10) / 10, " deg)"));
echo(str("  Diffuser ring (laser path): OD ", acrylic_od + kerf, "  ID ", acrylic_id - kerf,
         "  -> finished OD ", acrylic_od, "  ID ", acrylic_id, "  thickness ", acrylic_t));
echo(str("  LED strip: ", strip_len, " mm of ", strip_room, " mm available"));
echo(str("  Baffle ID ", baffle_id, "  head OD ", head_od, "  overall height ", head_top + cap_t));
echo(str("  Carrier: ", carrier_fit, ", OD ", carrier_d, " in a ", head_id, " mm head bore (gap ",
         round((head_id - carrier_d) * 100) / 100, " mm)"));
if (carrier_fit == "floating")
    echo(str("  Centring screws: ", centring_insert, " inserts, M", ins_thread(centring_ins), " x ", centring_screw_len,
             " screws (at least ", round(centring_screw_min * 10) / 10, " mm)"));
else
    echo("  Centring screws: not needed with the fixed carrier");
if (has_holes)
    echo(str("  PCB screws: ", pcb_insert, " inserts, M", ins_thread(pcb_ins), " x ", pcb_screw_len,
             " screws (at most ", round(pcb_screw_max * 10) / 10, " mm)"));

// ------------------------------------------------------------------ parts

// Elephant's-foot chamfers, to subtract at a face z0 that prints on the bed with the
// part below it (the barrel, head and baffle print upside down). mirror([0, 0, 1]) them
// for a part that prints the right way up. One widens a bore's mouth, the other bevels
// the outer edge of a round face. Both are thin rings, so they also work on a groove's walls.
module bed_hole_chamfer(d, z0) {
    translate([0, 0, z0 - bed_chamfer]) difference() {
        cylinder(d1 = d, d2 = d + 2 * bed_chamfer + 0.02, h = bed_chamfer + 0.01);
        translate([0, 0, -0.01]) cylinder(d = d - 1, h = bed_chamfer + 0.03);
    }
}
module bed_edge_chamfer(d, z0) {
    translate([0, 0, z0 - bed_chamfer]) difference() {
        cylinder(d = d + 1, h = bed_chamfer + 0.01);
        translate([0, 0, -0.01]) cylinder(d1 = d + 0.02, d2 = d - 2 * bed_chamfer - 0.02, h = bed_chamfer + 0.02);
    }
}

// Barrel + flange. Printed with the head-side face of the floor on the bed, so the
// carrier seat is the first layer and the flange's seating face prints as a flat top
// surface: both tilt-critical faces are flat and parallel, with no bridges.
module barrel_part() {
    groove_id = head_id - fit_gap;
    groove_od = head_id + 2 * spigot_w + fit_gap;
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
        translate([0, 0, barrel_len - 1]) cylinder(d = baffle_od + fit_gap, h = flange_t + 2);
        translate([0, 0, head_floor_top - baffle_flange_t - 0.2])
            cylinder(d = baffle_flange_d + fit_gap, h = baffle_flange_t + 1);
        bed_hole_chamfer(baffle_flange_d + fit_gap, head_floor_top);
        // Sloped wire channel from the chamber up into the head.
        hull() {
            translate([wire_in_r, 0, barrel_len - 0.5]) cylinder(d = wire_hole_d, h = 0.1, $fn = 32);
            translate([wire_out_r, 0, head_floor_top]) cylinder(d = wire_hole_d, h = 0.5, $fn = 32);
        }
        // Groove for the head's spigot.
        translate([0, 0, head_floor_top - spigot_h - 0.3]) difference() {
            cylinder(d = groove_od, h = spigot_h + 1);
            translate([0, 0, -1]) cylinder(d = groove_id, h = spigot_h + 3);
        }
        bed_hole_chamfer(groove_od, head_floor_top);
        bed_edge_chamfer(groove_id, head_floor_top);
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
            for (a = [90, 210, 330]) rotate([0, 0, a]) translate([0, 0, screw_z]) rotate([0, 90, 0])
                translate([0, 0, head_od / 2 - 1]) cylinder(d = screw_boss_d, h = boss_out_r - head_od / 2 + 1);
        }
        translate([0, 0, head_floor_top - spigot_h - 1]) cylinder(d = head_id, h = head_h + spigot_h + 2);
        bed_hole_chamfer(head_id, head_top);    // the cap lip goes in here
        // Radial centring screws: the insert goes in from outside, the screw passes into
        // the cavity through a clearance hole.
        for (a = [90, 210, 330]) rotate([0, 0, a]) translate([0, 0, screw_z]) rotate([0, 90, 0]) {
            translate([0, 0, boss_out_r - ins_hole_depth(centring_ins)])
                cylinder(d = ins_hole_d(centring_ins), h = 20, $fn = 32);
            translate([0, 0, head_id / 2 - 1]) cylinder(d = ins_thread(centring_ins) + 0.4, h = 20, $fn = 32);
        }
        // Line this mark up with the one on the flange (wire channel side).
        translate([head_od / 2, 0, head_top - 3]) rotate([0, 0, 45]) cube([1.5, 1.5, 7], center = true);
    }
}

// The camera carrier. It prints floor-down, so its bottom edge is the first layer.
module carrier() {
    pocket = [pcb[0] + pcb_gap, pcb[1] + pcb_gap];
    translate([0, 0, head_floor_top]) difference() {
        union() {
            difference() {
                cylinder(d = carrier_d, h = carrier_h);
                // PCB pocket, placed so the lens sits on the optical axis.
                translate([-lens_off[0] - pocket[0] / 2, -lens_off[1] - pocket[1] / 2, carrier_floor_t])
                    cube([pocket[0], pocket[1], carrier_h]);
                // Round reliefs in its corners: printed inside corners round off, and the
                // PCB's sharp corners would catch on them.
                for (sx = [-1, 1], sy = [-1, 1])
                    translate([-lens_off[0] + sx * pocket[0] / 2, -lens_off[1] + sy * pocket[1] / 2, carrier_floor_t])
                        cylinder(d = 1.5, h = carrier_h, $fn = 16);
            }
            // PCB supports: bosses on the mounting holes, or corner pads.
            for (c = pcb_boss_xy) translate(c) cylinder(d = pcb_boss_d, h = carrier_floor_t + boss_h, $fn = 32);
            if (!has_holes)
                for (sx = [-1, 1], sy = [-1, 1])
                    translate([-lens_off[0] + sx * (pocket[0] / 2 - 1.5) - 1.5, -lens_off[1] + sy * (pocket[1] / 2 - 1.5) - 1.5, 0])
                        cube([3, 3, carrier_floor_t + boss_h]);
        }
        // Blind holes for the heat-set inserts, from the top of the bosses.
        for (c = pcb_boss_xy) translate([c[0], c[1], carrier_floor_t + boss_h - ins_hole_depth(pcb_ins)])
            cylinder(d = ins_hole_d(pcb_ins), h = carrier_h, $fn = 32);
        // Lens holder through the floor (the lens is on the axis).
        translate([-(holder[0] + 1) / 2, -(holder[1] + 1) / 2, -1]) cube([holder[0] + 1, holder[1] + 1, carrier_h + 2]);
        // Notch for the LED wires over the wire channel, lined up with the mark on the head.
        translate([wire_out_r - wire_notch_w / 2, -wire_notch_w / 2, -1]) cube([carrier_d / 2, wire_notch_w, carrier_h + 2]);
        // Elephant's foot would make the fixed carrier bind in the head.
        mirror([0, 0, 1]) bed_edge_chamfer(carrier_d, 0);
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
        bed_edge_chamfer(baffle_flange_d, h);
    }
}

// Clamps the acrylic's inner edge up against the ledge. Axial load only.
module retainer() {
    translate([0, 0, baffle_bottom]) difference() {
        cylinder(d = acrylic_id + 5, h = retainer_t);
        translate([0, 0, -1]) cylinder(d = baffle_od - press_fit, h = retainer_t + 2);
    }
}

// Held in the head by friction: the lip has the slide fit, so it centres the cap, and
// its crush ribs reach `squeeze` (on the diameter) past it. `label` is engraved on the
// underside, inside the lip.
module cap(squeeze = cap_squeeze, label = "") {
    cable_d = 5;
    lip_r = (head_id - fit_gap) / 2;
    rib_r = lip_r + squeeze / 2 - cap_rib_d / 2;    // rib centre: its outer edge at lip_r + squeeze / 2
    translate([0, 0, head_top]) difference() {
        union() {
            cylinder(d = head_od, h = cap_t);
            translate([0, 0, -cap_lip]) cylinder(d = 2 * lip_r, h = cap_lip);
            for (i = [0 : cap_ribs - 1]) rotate([0, 0, i * 360 / cap_ribs]) hull() {
                translate([rib_r, 0, cap_rib_lead - cap_lip]) cylinder(d = cap_rib_d, h = cap_lip - cap_rib_lead, $fn = 24);
                translate([lip_r - 0.5, 0, -cap_lip]) cylinder(d = 0.1, h = 0.01, $fn = 8);
            }
        }
        translate([0, 0, -cap_lip - 1]) cylinder(d = head_id - fit_gap - 4, h = cap_lip + 1);
        translate([0, 0, -cap_lip - 1]) cylinder(d = cable_d, h = cap_lip + cap_t + 2, $fn = 32);
        // Zip-tie slots for strain relief.
        for (s = [-1, 1]) translate([-2, s * (cable_d / 2 + 3) - 0.75, -1]) cube([4, 1.5, cap_t + 2]);
        // Notches in the underside, to lever the cap off the head's rim with a small screwdriver.
        for (a = [90, 270]) rotate([0, 0, a]) translate([head_id / 2 + 0.5, -3, -1]) cube([head_od, 6, 2]);
        // Mirrored, so that it reads the right way round on the underside.
        if (label != "")
            translate([0, -15, -0.01]) linear_extrude(0.61) mirror([0, 1, 0])
                text(label, size = 5, halign = "center", valign = "center");
    }
}

// Three caps with crush ribs at cap_squeeze - 0.15, + 0 and + 0.15, each engraved with
// its value inside. Keep the one that holds firmly in the head but still comes off.
module cap_test() {
    for (i = [0 : 2]) {
        s = max(0, cap_squeeze + (i - 1) * 0.15);
        translate([i * (head_od + 6), 0, 0]) flipped(head_top + cap_t) cap(s, str(s));
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
                // Without elephant's foot, like the real barrel's outside.
                mirror([0, 0, 1]) bed_edge_chamfer(d, 0);
            }
            // A bar across the ring carries the engraved diameter.
            difference() {
                translate([-(d - 2) / 2, -5, 0]) cube([d - 2, 10, 1.6]);
                translate([0, 0, 0.8]) linear_extrude(1) text(str(d), size = 5, halign = "center", valign = "center");
            }
        }
    }
}

// A peg and five rings with total gaps around fit_gap, engraved on their tabs. Set
// fit_gap to the ring the peg slides through without play. Both have the bed chamfers.
fit_test_d = 30;
module fit_test() {
    gaps = [for (s = [-0.2, -0.1, 0, 0.1, 0.2]) max(0, fit_gap + s)];
    pitch = [fit_test_d + 22, fit_test_d + 10];
    difference() {
        cylinder(d = fit_test_d, h = 10);
        translate([0, 0, -1]) cylinder(d = fit_test_d - 6, h = 12);
        mirror([0, 0, 1]) bed_edge_chamfer(fit_test_d, 0);
    }
    for (i = [0 : len(gaps) - 1]) {
        d = fit_test_d + gaps[i];
        translate([((i + 1) % 3) * pitch[0], floor((i + 1) / 3) * pitch[1], 0]) difference() {
            union() {
                cylinder(d = d + 4.8, h = 6);
                translate([d / 2, -4, 0]) cube([16.4, 8, 2]);
            }
            translate([0, 0, -1]) cylinder(d = d, h = 8);
            mirror([0, 0, 1]) bed_hole_chamfer(d, 0);
            translate([d / 2 + 9.4, 0, 1.2]) linear_extrude(1)
                text(str(gaps[i]), size = 3.5, halign = "center", valign = "center");
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
// The README's assembly drawings: an exploded cutaway, a section of the assembled adapter,
// and the head seen from above with the cap off. make_figures.py renders them and labels the
// points that exploded(), section_view() and top_view() echo as "anchor". The bought
// parts are simple stand-ins.
fig_gap = 10;
fig_cut = "Bisque";                                    // colour of the cut faces
pcb_z = head_floor_top + carrier_floor_t + boss_h;     // underside of the camera PCB
header_h = 2.5;
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

// Cuts away the half y > 0, for section_view(): the cut at 0 deg shows the wire channel.
// Each part gets its own `cut` colour, so you can see where the parts meet. OpenCSG draws
// cut faces that lie in one plane in a single colour, so each part also gets its own
// `layer`: a cut 0.01 mm further out per layer. `turn` turns the part's right half (x < 0)
// about the axis, to bring a centring screw into the cut (an aligned section).
module half_cut(c, cut, layer, turn = 0) {
    module cut_y() color(cut) translate([-500, layer * 0.01, -500]) cube(1000);
    if (turn == 0) difference() { color(c) children(); cut_y(); }
    else for (s = [0, 1]) difference() {
        color(c) rotate([0, 0, s * turn]) children();
        cut_y();
        translate([s == 0 ? -1000 : 0, -500, -500]) cube(1000);
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

// Along +z, tip at z = 0: centring screw, socket head with a nylon tip.
module centring_screw() {
    m = ins_thread(centring_ins);
    len = centring_screw_len;
    color("White") cylinder(d = m - 0.2, h = 1.2, $fn = 24);
    color("Silver") difference() {
        union() {
            translate([0, 0, 1.2]) cylinder(d = m, h = len - 1.2, $fn = 24);
            translate([0, 0, len]) cylinder(d = 1.8 * m, h = m, $fn = 32);
        }
        translate([0, 0, len + 1]) cylinder(d = m - 0.1, h = m, $fn = 6);
    }
}

// Along +z from z = 0: heat-set insert.
module heat_insert(i) {
    color("Goldenrod") difference() {
        union() {
            cylinder(d = ins_od(i) - 0.4, h = ins_len(i), $fn = 32);
            for (z = [1, ins_len(i) - 2]) translate([0, 0, z]) cylinder(d = ins_od(i), h = 1, $fn = 32);
        }
        translate([0, 0, -1]) cylinder(d = ins_thread(i) - 0.4, h = ins_len(i) + 2, $fn = 24);
    }
}

// The centring screws' positions: along +z at each boss, from the axis outward.
module at_centring_screws() {
    for (a = [90, 210, 330]) rotate([0, 0, a]) translate([0, 0, screw_z]) rotate([0, 90, 0]) children();
}

// Camera PCB, lens holder and lens in their assembled position, with the USB header on
// the wire side. With cut = true each piece is cut like the printed parts in section_view().
module camera_board(cut = false) {
    holder_h = min(5, lens_drop);
    module piece(c, cut_c, layer) if (cut) half_cut(c, cut_c, layer) children(); else color(c) children();
    piece("ForestGreen", "#9fd49f", 20) translate([-lens_off[0] - pcb[0] / 2, -lens_off[1] - pcb[1] / 2, pcb_z]) cube(pcb);
    piece("WhiteSmoke", "White", 21) translate([pcb[0] / 2 - 4 - lens_off[0], -5 - lens_off[1], pcb_z + pcb[2]])
        cube([2.5, 10, header_h]);
    piece("#222", "#777", 22) {
        translate([-holder[0] / 2, -holder[1] / 2, pcb_z - holder_h]) cube([holder[0], holder[1], holder_h]);
        if (lens_drop > holder_h)
            translate([0, 0, lens_z]) cylinder(d = min(holder[0], holder[1]) * 0.8, h = lens_drop - holder_h);
    }
}

// The barrel's white lining and the baffle's white outside (paint or tape).
module lining() {
    translate([0, 0, ledge_z]) difference() {
        cylinder(d = bore_d, h = barrel_len - ledge_z);
        translate([0, 0, -1]) cylinder(d = bore_d - 0.8, h = barrel_len);
    }
}
module baffle_skin() {
    translate([0, 0, baffle_bottom]) difference() {
        cylinder(d = baffle_od + 0.4, h = head_floor_top - baffle_flange_t - baffle_bottom);
        translate([0, 0, -1]) cylinder(d = baffle_od, h = head_floor_top);
    }
}

// LED strip on the chamber wall, with its gap centred on the wire channel (0 deg). Only
// the part from `from` deg on, outside the cut: OpenCSG draws a cut rotate_extrude poorly.
module led_strip(from = 91) {
    a = strip_len / (PI * bore_d) * 360;
    color("Gold") translate([0, 0, strip_z]) rotate([0, 0, from])
        rotate_extrude(angle = 180 + a / 2 - from) translate([bore_d / 2 - 1.6, 0]) square([1.2, led_strip_w]);
}

// The strip's two wires, up the sloped channel to `top`, with a JST plug there.
module led_wires(top, plug = true) {
    a = 180 + strip_len / (PI * bore_d) * 180;
    r = bore_d / 2 - 1;
    for (s = [-1, 1]) color(s > 0 ? "Red" : "#111")
        rod([[r * cos(a), r * sin(a), strip_z + led_strip_w / 2 + s * 0.6],
             [r * cos(a / 2 + 180), r * sin(a / 2 + 180), barrel_len - 2],
             [wire_in_r, s * 0.5, barrel_len - 0.5],
             [wire_out_r, s * 0.5, head_floor_top],
             [wire_out_r, s * 0.5, top]], 1);
    if (plug) color("WhiteSmoke") translate([wire_out_r - 2, -3, top]) cube([4, 6, 5]);
}

module exploded() {
    // Barrel, white lining, LED strip and wires.
    cutaway("DimGray") barrel_part();
    cutaway("White") lining();
    led_strip();
    led_wires(head_floor_top + fig_gap * 0.4);
    // Diffuser and retainer, from below.
    translate([0, 0, fig_dz_acrylic]) cutaway("GhostWhite") translate([0, 0, 0.2]) linear_extrude(acrylic_t) diffuser_2d();
    translate([0, 0, fig_dz_retainer]) cutaway("Gray") retainer();
    // Head with one insert and screw pulled out along the boss at 90 deg.
    translate([0, 0, fig_dz_head]) {
        cutaway("DarkSlateGray") head();
        rotate([0, 0, 90]) translate([0, 0, screw_z]) rotate([0, 90, 0]) {
            translate([0, 0, boss_out_r + 5]) heat_insert(centring_ins);
            translate([0, 0, boss_out_r + ins_len(centring_ins) + 12]) centring_screw();
        }
    }
    // Baffle: black bore, white outside.
    translate([0, 0, fig_dz_baffle]) {
        cutaway("#333") baffle();
        cutaway("White") baffle_skin();
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
    anchor("insert", polar(90, boss_out_r + 5 + ins_len(centring_ins) / 2, screw_z + fig_dz_head));
    anchor("screw", polar(90, boss_out_r + ins_len(centring_ins) + 12 + centring_screw_len / 2, screw_z + fig_dz_head));
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
    arrow(polar(90, boss_out_r + ins_len(centring_ins) + 10, screw_z + fig_dz_head), polar(90, boss_out_r + 1, screw_z + fig_dz_head));
}

// Assembled, cut in half and seen straight at the cut. The left half is cut on the wire
// side; in the right half the head and screws are turned so the cut goes through a
// centring screw.
sv_turn = 180 - 210;
module section_view() {
    half_cut("DimGray", "Bisque", 1) barrel_part();
    half_cut("White", "White", 2) lining();
    led_strip(180);
    led_wires(head_floor_top + carrier_h + 1, plug = false);
    half_cut("GhostWhite", "AliceBlue", 3) translate([0, 0, 0.2]) linear_extrude(acrylic_t) diffuser_2d();
    half_cut("Gray", "Khaki", 4) retainer();
    half_cut("DarkSlateGray", "#9fd0c6", 5, sv_turn) head();
    half_cut("Goldenrod", "#f2cf6b", 6, sv_turn)
        at_centring_screws() translate([0, 0, boss_out_r - ins_len(centring_ins)]) heat_insert(centring_ins);
    half_cut("Silver", "Gainsboro", 7, sv_turn) at_centring_screws() translate([0, 0, carrier_od / 2]) centring_screw();
    half_cut("#333", "Plum", 8) baffle();
    half_cut("White", "White", 9) baffle_skin();
    half_cut("SlateGray", "LightSteelBlue", 10) carrier();
    camera_board(cut = true);
    half_cut("DimGray", "#f2c2cb", 11) cap();
    // USB cable from the header, out through the cap.
    hx = pcb[0] / 2 - 2.75 - lens_off[0];
    hz = pcb_z + pcb[2] + header_h;
    color("#1a1a1a") rod([[hx, 0, hz], [hx, 0, hz + 3], [0, 0, hz + 8], [0, 0, head_top + cap_t + 8]], 4);

    // Label points on the cut: x > 0 is the left half, x < 0 the right one.
    anchor("sv_cap", [(head_id - fit_gap) / 2 - 1, 0, head_top - cap_lip / 2]);
    anchor("sv_camera", [pcb[0] / 2 - lens_off[0] - 7, 0, pcb_z + pcb[2] / 2]);
    anchor("sv_spigot", [head_id / 2 + spigot_w / 2, 0, head_floor_top - spigot_h / 2]);
    anchor("sv_wires", [(wire_in_r + wire_out_r) / 2, 0.5, (barrel_len + head_floor_top) / 2]);
    anchor("sv_diffuser", [(acrylic_od + acrylic_id) / 4, 0, 0.2 + acrylic_t / 2]);
    anchor("sv_insert", [-boss_out_r + ins_len(centring_ins) / 2, 0, screw_z]);
    anchor("sv_screw", [-carrier_od / 2 - 0.6, 0, screw_z]);
    anchor("sv_carrier", [-carrier_od / 2 + 3, 0, head_floor_top + 1]);
    anchor("sv_baffle", [-(baffle_od + baffle_flange_d) / 4, 0, head_floor_top - baffle_flange_t / 2]);
    anchor("sv_led_strip", polar(200, bore_d / 2 - 1, strip_z + led_strip_w / 2));
    anchor("sv_barrel", [-barrel_od / 2 + 1, 0, barrel_len * 0.4]);
    anchor("sv_retainer", [-(acrylic_id + 5) / 2 + 1, 0, baffle_bottom + retainer_t / 2]);
}

// After step 5, seen from above with the cap off.
module top_view() {
    color("DimGray") barrel_part();
    color("DarkSlateGray") head();
    color("SlateGray") carrier();
    camera_board();
    at_centring_screws() {
        translate([0, 0, boss_out_r - ins_len(centring_ins)]) heat_insert(centring_ins);
        translate([0, 0, carrier_od / 2]) centring_screw();
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
else if (part == "section") section_view();
else if (part == "exploded") exploded();
else if (part == "top_view") top_view();
else if (part == "barrel") flipped(head_floor_top) barrel_part();   // carrier seat on the bed
else if (part == "head") flipped(head_top) head();                   // rim on the bed
else if (part == "carrier") translate([0, 0, -head_floor_top]) carrier();
else if (part == "baffle") flipped(head_floor_top) baffle();       // flange on the bed
else if (part == "retainer") translate([0, 0, -baffle_bottom]) retainer();
else if (part == "cap") flipped(head_top + cap_t) cap();           // flat top on the bed
else if (part == "cap_test") cap_test();                           // flips the caps itself
else if (part == "tolerance_test") tolerance_test();
else if (part == "fit_test") fit_test();
else if (part == "diffuser_2d") diffuser_2d();
else assert(false, str("unknown part '", part, "'"));
