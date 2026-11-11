#!/bin/sh
# Builds the macOS package folder from the release builds of both architectures: Statscraft.app
# (the window) and the statscraft command line, each one universal program for Apple silicon and
# Intel. From the repository root, after
#   cargo build --release -p statscraft -p statscraft-gui --target aarch64-apple-darwin
#   cargo build --release -p statscraft -p statscraft-gui --target x86_64-apple-darwin
#   packaging/macos/build.sh 0.1.0
# Needs lipo, iconutil, plutil and codesign (Xcode command line tools). Output:
# target/packages/statscraft-VERSION-macos-universal/, which the release workflow signs (once
# there is a Developer ID) and packs as a .tar.gz.
set -eu
version=$1
out=target/packages
tree=$out/statscraft-$version-macos-universal
app=$tree/Statscraft.app

rm -rf "$out"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$tree/statscraft" \
    target/aarch64-apple-darwin/release/statscraft target/x86_64-apple-darwin/release/statscraft
lipo -create -output "$app/Contents/MacOS/statscraft-gui" \
    target/aarch64-apple-darwin/release/statscraft-gui target/x86_64-apple-darwin/release/statscraft-gui
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

# the icon from the PNGs that Linux uses; Finder scales up the 256 px one where it needs more
iconset=$out/Statscraft.iconset
mkdir "$iconset"
icons=crates/gui/icons
cp $icons/statscraft-16.png "$iconset/icon_16x16.png"
cp $icons/statscraft-32.png "$iconset/icon_16x16@2x.png"
cp $icons/statscraft-32.png "$iconset/icon_32x32.png"
cp $icons/statscraft-64.png "$iconset/icon_32x32@2x.png"
cp $icons/statscraft-128.png "$iconset/icon_128x128.png"
cp $icons/statscraft-256.png "$iconset/icon_128x128@2x.png"
cp $icons/statscraft-256.png "$iconset/icon_256x256.png"
iconutil -c icns -o "$app/Contents/Resources/Statscraft.icns" "$iconset"
rm -r "$iconset"

cp README.md "$tree/README.md"
cp LICENSE "$tree/LICENSE"

# An ad-hoc signature, no identity: Apple silicon runs only signed code, and it seals the bundle
# so that macOS offers Open Anyway rather than calling it damaged. The Developer ID signature
# replaces it in the release workflow once there is one.
codesign --force --sign - "$tree/statscraft"
codesign --force --sign - "$app"
