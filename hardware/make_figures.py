#!/usr/bin/env python3
"""Render the README's assembly figures from colliview_adapter.scad into img/: the
exploded cutaway, the assembled half-section and the view from above.

    python make_figures.py [extra openscad -D overrides...]

Needs OpenSCAD (on PATH, or set OPENSCAD to openscad.com / openscad) and Pillow +
NumPy. OpenSCAD draws the parts; this script adds the labels and arrows at the points
the model echoes as "anchor" and "arrow", so the figures follow the model.
"""
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
SCAD = HERE / "colliview_adapter.scad"
OUT = HERE / "img"
SS = 2  # supersampling: everything is drawn at twice the size, then scaled down

INK = (31, 35, 40)
MUTED = (87, 96, 106)
ACCENT = (9, 105, 218)
AXIS = (150, 158, 167)
WHITE = (255, 255, 255)

# side, step (None = no badge), title, detail, anchors
EXPLODED_LABELS = [
    ("left", 6, "Cap", "push in, ribs hold it; zip-tie the cable", ["cap"]),
    ("left", 5, "Camera board", "lens down through the carrier", ["camera"]),
    ("left", 5, "Carrier", "notch on the wire side", ["carrier_notch"]),
    ("left", 1, "Marks", "line up head and flange", ["head_mark", "flange_mark"]),
    ("left", 2, "LED wires", "up the sloped channel", ["wires"]),
    ("left", 4, "Opal acrylic ring", "pushed into the recess", ["diffuser"]),
    ("right", 3, "Baffle", "black bore, white outside", ["baffle"]),
    ("right", 1, "Head", "glued onto the flange", ["head"]),
    ("right", 1, "Centring screw ×3", "M3×5.7 heat-set insert, M3×12", ["insert", "screw"]),
    ("right", 2, "COB LED strip", "on the white-lined wall", ["led_strip"]),
    ("right", None, "Barrel", "flange seats on the focuser", ["barrel"]),
    ("right", 4, "Retainer", "pressed onto the baffle", ["retainer"]),
]
SECTION_LABELS = [
    ("left", 6, "Cap", "crush ribs grip the head", ["sv_cap"]),
    ("left", 5, "Camera board", "lens down into the baffle", ["sv_camera"]),
    ("left", 1, "Head", "spigot glued in the groove", ["sv_spigot"]),
    ("left", 2, "LED wires", "up the channel", ["sv_wires"]),
    ("left", 4, "Opal acrylic ring", "in the recess", ["sv_diffuser"]),
    ("right", 1, "Centring screw ×3", "insert in the head, tip on the carrier", ["sv_insert", "sv_screw"]),
    ("right", 5, "Carrier", "rests on the floor, slides sideways", ["sv_carrier"]),
    ("right", 3, "Baffle", "flange in the counterbore", ["sv_baffle"]),
    ("right", 2, "COB LED strip", "on the chamber wall", ["sv_led_strip"]),
    ("right", None, "Barrel", "into the focuser up to the flange", ["sv_barrel"]),
    ("right", 4, "Retainer", "pressed onto the baffle", ["sv_retainer"]),
]
TOP_LABELS = [
    ("left", None, "Mark", "on the head's rim", ["tv_mark"]),
    ("left", None, "Carrier notch", "the LED wires come up here", ["tv_notch"]),
    ("right", None, "Camera board", "lens down, on the axis", ["tv_camera"]),
    ("right", None, "Carrier", "floats inside the head", ["tv_carrier"]),
    ("right", None, "Centring screws ×3", "push the carrier sideways", ["tv_screw_210"]),
]


def find_openscad():
    exe = os.environ.get("OPENSCAD") or shutil.which("openscad")
    for p in [exe, r"C:\Program Files\OpenSCAD\openscad.com"]:
        if p and Path(p).exists():
            # On Windows only openscad.com writes the echo output to the console.
            com = Path(p).with_suffix(".com")
            return str(com if com.exists() else p)
    sys.exit("OpenSCAD not found: put it on PATH or set OPENSCAD")


def font(bold, size):
    names = ["segoeuib.ttf" if bold else "segoeui.ttf", "DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf",
             "Arial Bold.ttf" if bold else "Arial.ttf"]
    for n in names:
        try:
            return ImageFont.truetype(n, size * SS)
        except OSError:
            pass
    return ImageFont.load_default(size * SS)


def rot(axis, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return {"x": np.array([[1, 0, 0], [0, c, -s], [0, s, c]]),
            "y": np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]]),
            "z": np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])}[axis]


class View:
    """OpenSCAD's orthographic camera for --camera=tx,ty,tz,rx,ry,rz,dist, with a scale
    given in pixels per mm. Its view is dist * tan(fov / 2) high, with fov = 22.5 deg."""

    def __init__(self, centre, rotation, ppm, size_mm):
        self.ppm = ppm * SS
        self.size = [round(s * self.ppm) for s in size_mm]
        self.dist = self.size[1] / 2 / (self.ppm * math.tan(math.radians(22.5 / 2)))
        self.centre = np.array(centre, float)
        self.rotation = rotation
        rx, ry, rz = rotation
        self.m = rot("x", 90 - rx) @ rot("y", -ry) @ rot("z", -rz)

    def camera(self):
        return ",".join(f"{v:g}" for v in [*self.centre, *self.rotation, self.dist])

    def __call__(self, p):
        v = self.m @ (np.asarray(p, float) - self.centre)
        return np.array([self.size[0] / 2 + v[0] * self.ppm, self.size[1] / 2 - v[2] * self.ppm])


def render(part, view, extra):
    with tempfile.TemporaryDirectory() as tmp:
        png = Path(tmp) / f"{part}.png"
        # The drawings and their labels show the floating carrier with its centring screws.
        cmd = [find_openscad(), "-o", str(png), "-D", f'part="{part}"', "-D", 'carrier_fit="floating"', *extra,
               f"--imgsize={view.size[0]},{view.size[1]}", "--projection=o", "--colorscheme=Tomorrow",
               f"--camera={view.camera()}", str(SCAD)]
        r = subprocess.run(cmd, capture_output=True, text=True)
        log = r.stdout + r.stderr
        if r.returncode or not png.exists():
            sys.exit(f"openscad failed:\n{log}")
        img = Image.open(png).convert("RGB")
    vec = r"\[([-\d.e, ]+)\]"
    parse = lambda s: [float(v) for v in s.split(",")]
    anchors = {m[1]: parse(m[2]) for m in re.finditer(r'ECHO: "anchor", "(\w+)", ' + vec, log)}
    arrows = [(parse(m[1]), parse(m[2])) for m in re.finditer(r'ECHO: "arrow", ' + vec + ", " + vec, log)]
    return img, anchors, arrows


class Label:
    def __init__(self, side, step, title, detail, points):
        self.side, self.step, self.title, self.detail, self.points = side, step, title, detail, points
        self.f_title, self.f_detail, self.f_badge = font(True, 19), font(False, 16), font(True, 15)
        self.r = 12 * SS if step else 0
        self.w = max(self.f_title.getlength(title), self.f_detail.getlength(detail))
        self.h = 52 * SS
        self.y = float(points[0][1])  # level with the first point

    def draw(self, d, x_text):
        """x_text: the text's edge on the model side (right edge for left labels)."""
        s = 1 if self.side == "right" else -1
        x_badge = x_text - s * (self.r + 6 * SS) if self.step else x_text
        # Leader lines to every point, from the badge (or the text edge).
        x0 = x_badge - s * self.r - s * 4 * SS if self.step else x_text - s * 6 * SS
        for p in self.points:
            d.line([x0, self.y, *p], fill=MUTED, width=round(1.5 * SS))
            d.ellipse([p[0] - 4 * SS, p[1] - 4 * SS, p[0] + 4 * SS, p[1] + 4 * SS],
                      fill=INK, outline=WHITE, width=round(1.5 * SS))
        if self.step:
            d.ellipse([x_badge - self.r, self.y - self.r, x_badge + self.r, self.y + self.r], fill=ACCENT)
            d.text((x_badge, self.y + SS), str(self.step), font=self.f_badge, fill=WHITE, anchor="mm")
        anchor = "l" if s > 0 else "r"
        d.text((x_text, self.y), self.title, font=self.f_title, fill=INK, anchor=anchor + "m")
        d.text((x_text, self.y + 21 * SS), self.detail, font=self.f_detail, fill=MUTED, anchor=anchor + "m")


def spread(labels, lo, hi):
    """Move labels apart vertically so that they don't overlap, staying in [lo, hi]."""
    labels.sort(key=lambda l: l.y)
    for a, b in zip(labels, labels[1:]):
        b.y = max(b.y, a.y + a.h)
    if labels and labels[-1].y > hi:
        labels[-1].y = hi
        for a, b in reversed(list(zip(labels, labels[1:]))):
            a.y = min(a.y, b.y - a.h)
    for l in labels:
        l.y = max(l.y, lo)


def arrow(d, a, b, colour, width):
    a, b = np.asarray(a, float), np.asarray(b, float)
    u = (b - a) / np.linalg.norm(b - a)
    n = np.array([-u[1], u[0]])
    head = 5 * width
    d.line([*a, *(b - u * head * 0.8)], fill=colour, width=round(width))
    d.polygon([tuple(b), tuple(b - u * head + n * head * 0.55), tuple(b - u * head - n * head * 0.55)], fill=colour)


def figure(part, view, specs, out, extra, axis=False):
    img, anchors, arrows = render(part, view, extra)
    a = np.asarray(img)
    model = ~np.all(a == a[0, 0], axis=2)
    ys, xs = np.nonzero(model)
    pad = 24 * SS
    crop = [xs.min() - pad, ys.min() - pad, xs.max() + pad, ys.max() + pad]
    img = Image.fromarray(np.where(model[..., None], a, np.array(WHITE, np.uint8))).crop(crop)
    model = Image.fromarray(model).crop(crop)
    offset = np.array(crop[:2])

    labels = []
    for side, step, title, detail, names in specs:
        missing = [n for n in names if n not in anchors]
        if missing:
            sys.exit(f"{part}: the model echoes no anchor {missing}")
        labels.append(Label(side, step, title, detail, [view(anchors[n]) - offset for n in names]))
    col = {s: math.ceil(max([l.w + 2 * l.r + 6 * SS for l in labels if l.side == s] + [0])) for s in ("left", "right")}
    lead = {s: (40 * SS if col[s] else 0) for s in col}
    margin = 20 * SS
    left = margin + col["left"] + lead["left"]
    width = left + img.width + lead["right"] + col["right"] + margin
    canvas = Image.new("RGB", (int(width), img.height + 2 * margin), WHITE)
    origin = np.array([left, margin])

    # The axis, behind the parts: drawn on a layer that only shows where there's no model.
    if axis:
        layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
        dl = ImageDraw.Draw(layer)
        x = view([0, 0, 0])[0] - offset[0] + origin[0]
        y, dash = 0, [18 * SS, 6 * SS, 3 * SS, 6 * SS]
        while y < canvas.height:
            for i, seg in enumerate(dash):
                if i % 2 == 0:
                    dl.line([x, y, x, y + seg], fill=AXIS + (255,), width=round(1.5 * SS))
                y += seg
        hide = Image.new("L", canvas.size, 255)
        hide.paste(Image.eval(model.convert("L"), lambda v: 0 if v else 255), tuple(origin))
        layer.putalpha(Image.fromarray(np.minimum(np.asarray(layer.getchannel("A")), np.asarray(hide))))
        canvas.paste(img, tuple(origin))
        canvas = Image.alpha_composite(canvas.convert("RGBA"), layer).convert("RGB")
    else:
        canvas.paste(img, tuple(origin))

    d = ImageDraw.Draw(canvas)
    for a_, b_ in arrows:
        arrow(d, view(a_) - offset + origin, view(b_) - offset + origin, ACCENT, 3.5 * SS)
    for l in labels:
        l.points = [p + origin for p in l.points]
        l.y += origin[1]
    for side in ("left", "right"):
        group = [l for l in labels if l.side == side]
        spread(group, margin + 12 * SS, canvas.height - margin - 30 * SS)
        for l in group:
            badge = 2 * l.r + 6 * SS if l.step else 0
            if side == "left":
                l.draw(d, margin + col["left"] - badge)
            else:
                l.draw(d, left + img.width + lead["right"] + badge)

    canvas = canvas.resize((canvas.width // SS, canvas.height // SS), Image.LANCZOS)
    OUT.mkdir(exist_ok=True)
    canvas.save(out, optimize=True)
    print(f"{out.relative_to(HERE)}: {canvas.width} x {canvas.height}")


def main():
    extra = sys.argv[1:]
    # Exploded cutaway, seen from the cut-away quarter (45 deg) and 20 deg above.
    figure("exploded", View([0, 0, 82], [70, 0, 135], 4.6, [140, 290]), EXPLODED_LABELS,
           OUT / "assembly_exploded.png", extra, axis=True)
    # Assembled and cut in half, seen at the cut from 10 deg above, wire side on the left.
    figure("section", View([0, 0, 30], [80, 0, 180], 9, [110, 110]), SECTION_LABELS,
           OUT / "assembly_section.png", extra)
    # From above, turned so that the wire side is on the left as in the exploded view.
    figure("top_view", View([0, 0, 40], [0, 0, 180], 4.6, [110, 110]), TOP_LABELS,
           OUT / "assembly_top.png", extra)


if __name__ == "__main__":
    main()
