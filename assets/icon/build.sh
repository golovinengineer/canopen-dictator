#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Regenerates every raster icon from the SVG sources in this directory.
# Needs rsvg-convert (librsvg) and ImageMagick 7 (`magick`); the .icns also
# needs iconutil, so run it on macOS.
#
# Sources:
#   dictator.svg              main icon, light
#   dictator-dark.svg         the same on a deep green backdrop, for dark contexts
#   dictator-small*.svg       simplified for 16-32 px (no text lines, no badge wave)
#   dictator-macos*.svg       the light icon on Apple's 1024/824 grid with shadow
set -eu
cd "$(dirname "$0")"
font=../../ui/fonts/JetBrainsMonoNL-Regular.ttf

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

render() { rsvg-convert -w "$2" -h "$2" "$1" -o "$3"; }

mkdir -p png
for s in 16 24 32; do
    render dictator-small.svg "$s" "png/dictator-$s.png"
    render dictator-small-dark.svg "$s" "png/dictator-dark-$s.png"
done
for s in 48 64 128 256 512 1024; do
    render dictator.svg "$s" "png/dictator-$s.png"
    render dictator-dark.svg "$s" "png/dictator-dark-$s.png"
done

# Windows
magick png/dictator-16.png png/dictator-24.png png/dictator-32.png \
    png/dictator-48.png png/dictator-64.png png/dictator-128.png \
    png/dictator-256.png dictator.ico

# macOS
set_dir=$tmp/dictator.iconset
mkdir -p "$set_dir"
for s in 16 32 128 256 512; do
    d=$((s * 2))
    if [ "$s" -le 32 ]; then src=dictator-macos-small.svg; else src=dictator-macos.svg; fi
    render "$src" "$s" "$set_dir/icon_${s}x${s}.png"
    if [ "$d" -le 32 ]; then src=dictator-macos-small.svg; else src=dictator-macos.svg; fi
    render "$src" "$d" "$set_dir/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$set_dir" -o dictator.icns

# README banners: icon and wordmark on a transparent background, one per theme.
banner() { # $1 icon svg, $2 text colour, $3 muted colour, $4 output
    render "$1" 280 "$tmp/banner-icon.png"
    magick -size 1280x320 xc:none \
        "$tmp/banner-icon.png" -geometry +24+20 -composite \
        -font "$font" -fill "$2" -stroke "$2" -strokewidth 2.5 -pointsize 112 \
        -annotate +336+176 'Dictator' \
        -stroke none -fill "$3" -pointsize 34 \
        -annotate +342+236 'CANopen object dictionary editor' \
        -trim +repage -bordercolor none -border 20 "$4"
}
banner dictator.svg '#2C4A3E' '#5E7A6C' banner.png
banner dictator-dark.svg '#DCEDE2' '#94B3A2' banner-dark.png

# GitHub social preview (Settings > General > Social preview), 1280x640.
render dictator.svg 400 "$tmp/social-icon.png"
magick -size 1280x640 xc:'#EEF5F0' \
    "$tmp/social-icon.png" -geometry +96+120 -composite \
    -font "$font" -fill '#2C4A3E' -stroke '#2C4A3E' -strokewidth 3 -pointsize 124 \
    -annotate +540+318 'Dictator' \
    -stroke none -fill '#5E7A6C' -pointsize 36 \
    -annotate +546+384 'CANopen object dictionary editor' \
    -annotate +546+432 'EDS · C code · Markdown docs' \
    social-preview.png
