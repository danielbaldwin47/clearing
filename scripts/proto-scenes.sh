#!/usr/bin/env bash
# PROTOTYPE (ticket #3): capture every judged scene of one variant in headless Kitty.
# usage: scripts/proto-scenes.sh <variant letter> <output dir> [binary]
# Each scene lands as <scene>.png (with the variant marker) and blind/<scene>.png
# (last terminal row cropped off, so a judge cannot read which variant it is).
set -euo pipefail
variant=$1
out=$2
bin=${3:-./target/release/clearing}
mkdir -p "$out/blind"
scene() { # name columns rows keys
  python3 scripts/proto-capture.py --output "$out/$1.png" --columns "$2" --rows "$3" --keys "$4" \
    -- "$bin" --sample --variant "$variant" >/dev/null
  h=$(identify -format %h "$out/$1.png")
  w=$(identify -format %w "$out/$1.png")
  magick "$out/$1.png" -crop "${w}x$((h - h / $3))+0+0" +repage "$out/blind/$1.png"
}
scene root-140x44 140 44 ""
scene root-100x30 100 30 ""
scene select-local-140x44 140 44 "Down"
scene projects-140x44 140 44 "Enter"
scene cache-140x44 140 44 "Down,Down,Down,Enter"
scene cache-100x30 100 30 "Down,Down,Down,Enter"
echo "captured 6 scenes of variant $variant into $out"
