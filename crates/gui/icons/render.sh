#!/bin/sh
# Renders statscraft.svg to the PNG sizes Linux uses and to statscraft.ico for Windows.
# statscraft.svg is the logo's icon as the brand files give it: replace it, never redraw it.
# All sizes, 16 px included, are the same full-colour icon.
# Needs rsvg-convert (librsvg) and ImageMagick. Run it after changing the SVG and commit the output.
set -eu
cd "$(dirname "$0")"
for size in 16 32 48 64 128 256; do
    rsvg-convert -w $size -h $size statscraft.svg -o statscraft-$size.png
done
convert statscraft-16.png statscraft-32.png statscraft-48.png statscraft-64.png statscraft-128.png statscraft-256.png statscraft.ico
