#!/usr/bin/env bash
# Assemble a real .app bundle for the Rust shell so it can be launched from
# Finder, the Dock, Spotlight — and from the `hermes-rust` command.
#
#   scripts/bundle.sh [debug|release] [output.app] [bundle-id] [display-name]
#
# The bundle is installed to ~/Applications/Hermes Rust.app.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"
BIN="$ROOT/target/$PROFILE/hermes-desktop-rust"
APP="${2:-$HOME/Applications/Hermes Rust.app}"
BUNDLE_ID="${3:-ai.nous.hermes.rust-shell}"
BUNDLE_NAME="${4:-Hermes}"
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
ICON_TEMP="$(mktemp -d)"
trap 'rm -rf "$ICON_TEMP"' EXIT
ICONSET="$ICON_TEMP/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

python3 - "$APP/Contents/Info.plist" "$BUNDLE_ID" "$BUNDLE_NAME" <<'PY'
import plistlib, sys
with open(sys.argv[1], 'wb') as output:
    plistlib.dump({
        'CFBundleName': sys.argv[3], 'CFBundleDisplayName': sys.argv[3],
        'CFBundleIdentifier': sys.argv[2], 'CFBundleExecutable': 'hermes-desktop-rust',
        'CFBundleIconFile': 'AppIcon', 'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': '0.1.0', 'CFBundleVersion': '1',
        'LSMinimumSystemVersion': '13.0',
        'LSApplicationCategoryType': 'public.app-category.developer-tools',
        'NSHighResolutionCapable': True, 'NSRequiresAquaSystemAppearance': False,
        'NSMicrophoneUsageDescription': 'Hermes uses the microphone for dictation and voice conversations when you start them.',
    }, output)
PY

# Ad-hoc signature: enough for local use, and keeps macOS from re-prompting for
# permissions on every rebuild (the bundle identity stays stable).
codesign --force --deep --sign - "$APP" 2>/dev/null || echo "bundle: ad-hoc codesign skipped"

echo "bundle: done — $(du -sh "$APP" | cut -f1)"
