#!/usr/bin/env bash
# Builds "AG Unlocker.app" and a zip of it into dist/.
#
#   macos/build.sh              this Mac's architecture
#   macos/build.sh --universal  Apple Silicon and Intel in one binary; needs
#                               rustup target add aarch64-apple-darwin x86_64-apple-darwin
#
# The bundle is signed ad hoc - there is no Developer ID here - so a copy
# downloaded from the internet is opened the first time with right-click >
# Open (or System Settings > Privacy & Security > Open Anyway). A bundle rather
# than a bare binary, because App Management is granted to an app: the user
# allows «AG Unlocker» once, not the terminal they happened to start it from.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
APP="dist/AG Unlocker.app"

if [[ "${1:-}" == "--universal" ]]; then
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    BIN="target/universal/ag_unlocker"
    mkdir -p target/universal
    lipo -create -output "$BIN" \
        target/aarch64-apple-darwin/release/ag_unlocker \
        target/x86_64-apple-darwin/release/ag_unlocker
    SUFFIX="macos"
else
    cargo build --release
    BIN="target/release/ag_unlocker"
    SUFFIX="macos_$(uname -m)"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/ag_unlocker"
cp macos/icon.icns "$APP/Contents/Resources/icon.icns"
sed "s/@VERSION@/$VERSION/g" macos/Info.plist > "$APP/Contents/Info.plist"
codesign --force --sign - "$APP"

ZIP="dist/AG_${VERSION}_${SUFFIX}.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
echo "$APP"
echo "$ZIP"
