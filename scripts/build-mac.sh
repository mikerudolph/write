#!/bin/sh
set -eu

sh scripts/build-bindings.sh
swift build -c release --package-path apps/mac

app=build/mac/Write.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp apps/mac/.build/release/Write "$app/Contents/MacOS/Write"
cp apps/mac/Resources/Info.plist "$app/Contents/Info.plist"
codesign --force --sign - "$app"
echo "Built $app"
