#!/bin/bash
# Bundle target/release/vizij into vizij.app and a drag-to-install .dmg
# (the app beside an Applications link). The app is ad-hoc signed only, so
# macOS quarantines a downloaded copy until it is opened once from the
# context menu, or the attribute is cleared:
#   xattr -dr com.apple.quarantine /Applications/vizij.app
#
# Usage: scripts/package_macos.sh <version> [arch-label]
# Produces: vizij-<version>-macos-<arch>.dmg in the working directory.
# Requires: a release build of the binary (cargo build -p vizij --release).
set -euo pipefail

version="${1:?usage: package_macos.sh <version> [arch-label]}"
arch="${2:-$(uname -m | sed 's/x86_64/intel/')}"
root="$(cd "$(dirname "$0")/.." && pwd)"
binary="$root/target/release/vizij"
[ -x "$binary" ] || { echo "error: build first: cargo build -p vizij --release" >&2; exit 1; }

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
app="$staging/vizij.app"

mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/vizij"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>vizij</string>
  <key>CFBundleIdentifier</key><string>ai.vizij.vizij</string>
  <key>CFBundleName</key><string>vizij</string>
  <key>CFBundleDisplayName</key><string>Vizij</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# The bundle's signature has to cover its Info.plist, not only the binary the
# linker signed.
codesign --force --sign - "$app"
ln -s /Applications "$staging/Applications"

dmg="vizij-$version-macos-$arch.dmg"
rm -f "$dmg"
hdiutil create -volname "Vizij" -srcfolder "$staging" -format UDZO -ov "$dmg"
echo "built: $dmg"
