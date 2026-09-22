#!/usr/bin/env bash
# PROTOTYPE (ticket #3, round 6): capture one variant's journey in headless Kitty.
# usage: scripts/proto-journey.sh <variant letter> <output dir> [binary]
# Reads prototype-notes/journey-<letter>.txt: one scene per line,
#   <name> <columns> <rows> <keys, comma-separated tmux key names, or - for none>
# Blank lines and lines starting with # are skipped. Scenes run in parallel.
set -euo pipefail
variant=$1
out=$2
bin=${3:-./target/release/clearing}
spec=prototype-notes/journey-${variant,,}.txt
mkdir -p "$out"
while read -r name cols rows keys; do
  [[ -z ${name:-} || $name == \#* ]] && continue
  [[ $keys == - ]] && keys=""
  python3 scripts/proto-capture.py --output "$out/$name.png" --columns "$cols" --rows "$rows" --keys "$keys" \
    -- "$bin" --sample --variant "$variant" >/dev/null &
done <"$spec"
wait
echo "captured $(grep -cv '^\s*\(#\|$\)' "$spec") scenes of variant $variant into $out"
