#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Set version and sha256 in the Homebrew cask (golovinengineer/homebrew-tap,
# Casks/dictator.rb). Run by the release workflow after a release.
#
#   scripts/update-cask.sh CASK_FILE VERSION SHA256

set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 CASK_FILE VERSION SHA256" >&2
    exit 2
fi
cask=$1 version=$2 sha=$3

[[ $version =~ ^[0-9]+(\.[0-9]+)+(-[0-9A-Za-z.]+)?$ ]] || { echo "bad version: $version" >&2; exit 1; }
[[ $sha =~ ^[0-9a-f]{64}$ ]] || { echo "bad sha256: $sha" >&2; exit 1; }
grep -q '^  version "' "$cask" && grep -q '^  sha256 "' "$cask" ||
    { echo "$cask has no version/sha256 line" >&2; exit 1; }

# perl -i works the same with GNU (CI) and BSD (macOS) tools, unlike sed -i.
VERSION=$version SHA=$sha perl -pi -e '
    s/^  version ".*"/  version "$ENV{VERSION}"/;
    s/^  sha256 ".*"/  sha256 "$ENV{SHA}"/;
' "$cask"
