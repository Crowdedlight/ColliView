#!/usr/bin/env sh
# Export every printed part (STL) and the laser-cut diffuser (DXF + SVG) for
# 2 mm and 3 mm acrylic into hardware/out/.
#
# Usage: ./build.sh [extra openscad -D overrides...]
#   e.g. ./build.sh -D barrel_od=50.7 -D acrylic_t=2.9
set -eu
cd "$(dirname "$0")"
OPENSCAD="${OPENSCAD:-openscad}"
mkdir -p out

for t in 2 3; do
    # The barrel's acrylic recess depends on the sheet thickness.
    "$OPENSCAD" -o "out/barrel_${t}mm.stl" -D 'part="barrel"' -D "acrylic_t=$t" "$@" colliview_adapter.scad
    "$OPENSCAD" -o "out/diffuser_${t}mm.dxf" -D 'part="diffuser_2d"' -D "acrylic_t=$t" "$@" colliview_adapter.scad
    "$OPENSCAD" -o "out/diffuser_${t}mm.svg" -D 'part="diffuser_2d"' -D "acrylic_t=$t" "$@" colliview_adapter.scad
done
for p in head carrier baffle retainer cap cap_test tolerance_test fit_test jig; do
    "$OPENSCAD" -o "out/$p.stl" -D "part=\"$p\"" "$@" colliview_adapter.scad
done
echo "Done: $(ls out | wc -l) files in $(pwd)/out"
