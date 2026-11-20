#!/bin/sh
# Renders statscraft.svg to the PNG sizes Linux uses and to statscraft.ico for Windows, and the
# file type icons file-data.svg, file-syntax.svg and file-output.svg to their .ico files.
# statscraft.svg is the logo's icon as the brand files give it: replace it, never redraw it.
# All sizes, 16 px included, are the same full-colour icon.
# Needs rsvg-convert (librsvg) and ImageMagick. Run it after changing the SVG and commit the output.
set -eu
cd "$(dirname "$0")"
for size in 16 32 48 64 128 256; do
    rsvg-convert -w $size -h $size statscraft.svg -o statscraft-$size.png
done
convert statscraft-16.png statscraft-32.png statscraft-48.png statscraft-64.png statscraft-128.png statscraft-256.png statscraft.ico
for kind in data syntax output; do
    for size in 16 32 48 64 128 256; do
        rsvg-convert -w $size -h $size file-$kind.svg -o file-$kind-$size.png
    done
    convert file-$kind-16.png file-$kind-32.png file-$kind-48.png file-$kind-64.png \
        file-$kind-128.png file-$kind-256.png file-$kind.ico
    rm file-$kind-*.png
done
