#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Build Dictator.app for macOS as a universal binary (Apple Silicon + Intel),
# sign it ad hoc, and pack it as a .dmg (drag to Applications) and a .zip.
#
#   scripts/bundle-macos.sh
#
# Needs the Rust targets aarch64-apple-darwin and x86_64-apple-darwin
# (`rustup target add ...`). Output in target/bundle/:
#   Dictator.app
#   Dictator-<version>-macos-universal.dmg (+ .sha256)
#   Dictator-<version>-macos-universal.zip (+ .sha256)

set -euo pipefail

cd "$(dirname "$0")/.."

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
MIN_MACOS=11.0
BUNDLE_ID=io.github.golovinengineer.dictator
OUT=target/bundle
APP="$OUT/Dictator.app"

version=$(cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "canopen-dictator"))')
# CFBundleVersion takes only digits and dots: 0.1.0-alpha.1 -> 0.1.0
short_version=${version%%-*}

export MACOSX_DEPLOYMENT_TARGET=$MIN_MACOS

binaries=()
for target in "${TARGETS[@]}"; do
    cargo build --release --locked --target "$target"
    binaries+=("target/$target/release/dictator")
done

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/dictator" "${binaries[@]}"

# The Liquid Glass icon (macOS 26 and later) comes from the Icon Composer file
# assets/icon/Dictator.icon; actool compiles it into Assets.car and also
# writes Dictator.icns for older macOS. actool ships with Xcode, not with the
# Command Line Tools: without it, a local build falls back to the flat icon,
# a CI build fails rather than ship the wrong one.
icon_name_key=""
if xcrun --find actool >/dev/null 2>&1; then
    xcrun actool assets/icon/Dictator.icon \
        --compile "$APP/Contents/Resources" \
        --output-partial-info-plist "$OUT/icon-info.plist" \
        --app-icon Dictator --include-all-app-icons \
        --enable-on-demand-resources NO --development-region en \
        --target-device mac --platform macosx \
        --minimum-deployment-target "$MIN_MACOS" \
        --errors --warnings --output-format human-readable-text
    rm -f "$OUT/icon-info.plist"
    [[ -f "$APP/Contents/Resources/Assets.car" && -f "$APP/Contents/Resources/Dictator.icns" ]] ||
        { echo "actool did not write Assets.car and Dictator.icns" >&2; exit 1; }
    icon_file=Dictator
    icon_name_key="<key>CFBundleIconName</key>
    <string>Dictator</string>"
elif [[ -n ${CI:-} ]]; then
    echo "actool not found (needs Xcode); refusing to build without the Liquid Glass icon" >&2
    exit 1
else
    echo "warning: actool not found (needs Xcode); using the flat icon" >&2
    cp assets/icon/dictator.icns "$APP/Contents/Resources/dictator.icns"
    icon_file=dictator
fi

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Dictator</string>
    <key>CFBundleDisplayName</key>
    <string>Dictator</string>
    <key>CFBundleExecutable</key>
    <string>dictator</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleIconFile</key>
    <string>$icon_file</string>
    $icon_name_key
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$short_version</string>
    <key>LSMinimumSystemVersion</key>
    <string>$MIN_MACOS</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
plutil -lint "$APP/Contents/Info.plist"

# lipo drops the linker's ad-hoc signature, and Apple Silicon refuses to run
# unsigned code. Without a Developer ID, sign ad hoc.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

name="Dictator-$version-macos-universal"

zip="$name.zip"
rm -f "$OUT/$zip" "$OUT/$zip.sha256"
ditto -c -k --keepParent "$APP" "$OUT/$zip"
(cd "$OUT" && shasum -a 256 "$zip" > "$zip.sha256")

# The disk image shows the app next to a link to /Applications, so opening
# it says how to install: drag one onto the other.
dmg="$name.dmg"
staging="$OUT/dmg"
rm -rf "$staging" "$OUT/$dmg" "$OUT/$dmg.sha256"
mkdir -p "$staging"
ditto "$APP" "$staging/Dictator.app"
ln -s /Applications "$staging/Applications"
# hdiutil now and then fails with "Resource busy" on CI runners; retry.
for attempt in 1 2 3; do
    if hdiutil create -volname "Dictator $version" -srcfolder "$staging" \
        -format UDZO -ov "$OUT/$dmg"; then
        break
    fi
    [[ $attempt == 3 ]] && exit 1
    sleep 5
done
rm -rf "$staging"
hdiutil verify "$OUT/$dmg"
(cd "$OUT" && shasum -a 256 "$dmg" > "$dmg.sha256")

lipo -info "$APP/Contents/MacOS/dictator"
echo "$OUT/$dmg"
echo "$OUT/$zip"
