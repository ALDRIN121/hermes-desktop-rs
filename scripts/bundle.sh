#!/usr/bin/env bash
# Assemble a real .app bundle for the Rust shell so it can be launched from
# Finder, the Dock, Spotlight — and from the `hermes-rust` command.
#
#   scripts/bundle.sh [debug|release]     # default: debug
#
# The bundle is installed to ~/Applications/Hermes Rust.app.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"
BIN="$ROOT/target/$PROFILE/hermes-desktop-rust"
APP="$HOME/Applications/Hermes Rust.app"
ICON_SRC="$ROOT/src-tauri/icons/icon.png"

if [[ ! -x "$BIN" ]]; then
  echo "bundle: $BIN not found — run 'cargo build -p hermes-desktop-rust' first" >&2
  exit 1
fi

echo "bundle: $PROFILE → $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/hermes-desktop-rust"

# .icns from the 1024px PNG: sips makes the sizes, iconutil packs them.
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Hermes Rust</string>
  <key>CFBundleDisplayName</key><string>Hermes Rust</string>
  <key>CFBundleIdentifier</key><string>com.aldrin.hermes-rust</string>
  <key>CFBundleExecutable</key><string>hermes-desktop-rust</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSRequiresAquaSystemAppearance</key><false/>
</dict>
</plist>
PLIST

# Ad-hoc signature: enough for local use, and keeps macOS from re-prompting for
# permissions on every rebuild (the bundle identity stays stable).
codesign --force --deep --sign - "$APP" 2>/dev/null || echo "bundle: ad-hoc codesign skipped"

echo "bundle: done — $(du -sh "$APP" | cut -f1)"
