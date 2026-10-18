#!/usr/bin/env bash
# Draw a social preview card with the project's own renderer.
#
#   scripts/og-image.sh [out.png] [width] [height]
#
# Needs rsvg-convert (brew install librsvg). The PNGs are committed, so CI
# does not need it.
set -euo pipefail

cd "$(dirname "$0")/.."

out=${1:-assets/og.png}
W=${2:-1200}
H=${3:-630}

SEQ=${SEQ:-"R U R' U'"}
PRESET=${PRESET:-"U F R"}

# Layout: the cube fills the left square, the text starts to the right of it.
pad=$(( H / 16 ))
side=$(( H - 2 * pad ))
tx=$(( pad + side + pad * 3 / 2 ))

title_y=$(( H * 37 / 100 ))
rule_y=$(( H * 41 / 100 ))
line1_y=$(( H * 50 / 100 ))
line2_y=$(( H * 59 / 100 ))
seq_y=$(( H * 72 / 100 ))
url_y=$(( H * 83 / 100 ))

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cargo run --release -q -p cube-export -- \
    -s "$SEQ" --preset "$PRESET" --size-px "$side" > "$tmp/cube.svg"

# Nest the cube in the card: drop its own size, give it a place on the canvas.
sed -e 's| width="[0-9.]*"||; s| height="[0-9.]*"||' \
    -e "s|^<svg |<svg x=\"$pad\" y=\"$pad\" width=\"$side\" height=\"$side\" |" \
    "$tmp/cube.svg" > "$tmp/inner.svg"

{
  cat <<HEAD
<svg xmlns="http://www.w3.org/2000/svg" width="$W" height="$H" viewBox="0 0 $W $H">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#14141b"/>
      <stop offset="1" stop-color="#0b0b10"/>
    </linearGradient>
  </defs>
  <rect width="$W" height="$H" fill="url(#bg)"/>
  <g font-family="Helvetica, Arial, sans-serif" fill="#ecedf3">
    <text x="$tx" y="$title_y" font-size="62" font-weight="700">Rubik&#8217;s cube</text>
    <rect x="$tx" y="$rule_y" width="104" height="3" fill="#2a78d6"/>
    <text x="$tx" y="$line1_y" font-size="36" fill="#9a9aa8">3D, net and cycle views</text>
    <text x="$tx" y="$line2_y" font-size="36" fill="#9a9aa8">Kociemba two-phase solver</text>
    <text x="$tx" y="$seq_y" font-size="26" fill="#5a9bff"
          font-family="Menlo, DejaVu Sans Mono, monospace">$SEQ</text>
    <text x="$tx" y="$url_y" font-size="24" fill="#65656f">twaclaw.github.io/cube</text>
  </g>
HEAD
  cat "$tmp/inner.svg"
  echo '</svg>'
} > "$tmp/card.svg"

rsvg-convert -w "$W" -h "$H" -o "$out" "$tmp/card.svg"
printf '%s  %sx%s  %s\n' "$out" "$W" "$H" "$(du -h "$out" | cut -f1)"
