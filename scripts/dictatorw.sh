#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Make dictatorw.exe next to dictator.exe: a copy marked as a windows-subsystem
# program, so Windows starts it without a console and it opens the window.
# Shortcuts and file associations run it; dictator.exe stays the command line.
#
#   scripts/dictatorw.sh target/release/dictator.exe

set -euo pipefail

exe=$1
out="$(dirname "$exe")/dictatorw.exe"
# The PE header's offset is at 0x3C. Subsystem follows the signature (4 bytes),
# the file header (20) and 68 bytes of the optional header, PE32 or PE32+.
pe=$(od -An -tu4 -j60 -N4 "$exe" | tr -d ' ')
subsystem=$((pe + 92))
if [[ $(od -An -tx4 -j"$pe" -N4 "$exe" | tr -d ' ') != 00004550 ||
    $(od -An -tu2 -j"$subsystem" -N2 "$exe" | tr -d ' ') != 3 ]]; then
    echo "$exe is not a console-subsystem PE executable" >&2
    exit 1
fi
cp "$exe" "$out"
printf '\x02' | dd of="$out" bs=1 seek="$subsystem" conv=notrunc status=none
echo "wrote $out"
