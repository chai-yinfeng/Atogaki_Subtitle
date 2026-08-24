#!/bin/zsh

set -euo pipefail

SCRIPT_DIR=${0:A:h}
PROJECT_DIR=${SCRIPT_DIR:h}
TAURI_DIR="$PROJECT_DIR/src-tauri"
ARTIFACT_DIR="$PROJECT_DIR/local-artifacts"

if [[ -n "$(git -C "$PROJECT_DIR" status --porcelain)" ]]; then
  print -u2 "Refusing to package a local test DMG from a dirty worktree."
  exit 1
fi

HOST_TARGET=$(rustc --print host-tuple)
if [[ "$HOST_TARGET" != "aarch64-apple-darwin" ]]; then
  print -u2 "Local macOS test packaging currently supports Apple Silicon only; found $HOST_TARGET."
  exit 1
fi

COMMIT=$(git -C "$PROJECT_DIR" rev-parse --short HEAD)
VERSION=$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' "$TAURI_DIR/tauri.conf.json" | head -1)
if [[ -z "$VERSION" ]]; then
  print -u2 "Could not read the Tauri product version."
  exit 1
fi

cd "$TAURI_DIR"
cargo tauri build --bundles dmg

SOURCE_DMG="$TAURI_DIR/target/release/bundle/dmg/Atogaki_${VERSION}_aarch64.dmg"
if [[ ! -f "$SOURCE_DMG" ]]; then
  print -u2 "Expected DMG was not created: $SOURCE_DMG"
  exit 1
fi

hdiutil verify "$SOURCE_DMG"

INSPECTION_MOUNT=$(mktemp -d /private/tmp/atogaki-dmg-inspection.XXXXXX)
cleanup_inspection_mount() {
  hdiutil detach "$INSPECTION_MOUNT" >/dev/null 2>&1 || true
  rmdir "$INSPECTION_MOUNT" >/dev/null 2>&1 || true
}
trap cleanup_inspection_mount EXIT
hdiutil attach "$SOURCE_DMG" -nobrowse -readonly -mountpoint "$INSPECTION_MOUNT" >/dev/null
APP_BUNDLE="$INSPECTION_MOUNT/Atogaki.app"
ICON_FILE=$(plutil -extract CFBundleIconFile raw "$APP_BUNDLE/Contents/Info.plist")
if [[ -z "$ICON_FILE" || "$ICON_FILE" != *.icns || ! -f "$APP_BUNDLE/Contents/Resources/$ICON_FILE" ]]; then
  print -u2 "Packaged App is missing its declared macOS icon metadata."
  exit 1
fi
hdiutil detach "$INSPECTION_MOUNT" >/dev/null
rmdir "$INSPECTION_MOUNT"
trap - EXIT

mkdir -p "$ARTIFACT_DIR"

DESTINATION="$ARTIFACT_DIR/Atogaki-${VERSION}-${COMMIT}-macos-arm64.dmg"
cp "$SOURCE_DMG" "$DESTINATION"
shasum -a 256 "$DESTINATION" > "$DESTINATION.sha256"

print "Local test DMG: $DESTINATION"
print "SHA-256: $(cut -d ' ' -f 1 "$DESTINATION.sha256")"
