#!/bin/zsh

set -euo pipefail

SCRIPT_DIR=${0:A:h}
PROJECT_DIR=${SCRIPT_DIR:h}
TAURI_DIR="$PROJECT_DIR/src-tauri"
SOURCE_APP="$TAURI_DIR/target/release/bundle/macos/Atogaki.app"
DESTINATION_APP="/Applications/Atogaki.app"

if [[ -n "$(git -C "$PROJECT_DIR" status --porcelain)" ]]; then
  print -u2 "Refusing to install the current App from a dirty worktree."
  exit 1
fi

if [[ "$(rustc --print host-tuple)" != "aarch64-apple-darwin" ]]; then
  print -u2 "The current formal local App is defined for Apple Silicon macOS."
  exit 1
fi

if pgrep -x atogaki-desktop >/dev/null; then
  print -u2 "Quit Atogaki before replacing /Applications/Atogaki.app."
  exit 1
fi

cd "$TAURI_DIR"
cargo tauri build --config tauri.macos-complete.conf.json --bundles app

for SIDECAR in ffmpeg ffprobe whisper-cli llama-server; do
  if [[ ! -x "$SOURCE_APP/Contents/MacOS/$SIDECAR" ]]; then
    print -u2 "Packaged App is missing sidecar: $SIDECAR"
    exit 1
  fi
done
codesign --verify --deep --strict "$SOURCE_APP"

STAGED_APP="/Applications/.Atogaki.install.$$"
BACKUP_APP="/Applications/.Atogaki.backup.$$"
cleanup() {
  [[ ! -e "$STAGED_APP" ]] || rm -rf "$STAGED_APP"
  if [[ -e "$BACKUP_APP" && ! -e "$DESTINATION_APP" ]]; then
    mv "$BACKUP_APP" "$DESTINATION_APP"
  fi
}
trap cleanup EXIT

ditto "$SOURCE_APP" "$STAGED_APP"
codesign --verify --deep --strict "$STAGED_APP"
if [[ -e "$DESTINATION_APP" ]]; then
  mv "$DESTINATION_APP" "$BACKUP_APP"
fi
mv "$STAGED_APP" "$DESTINATION_APP"
[[ ! -e "$BACKUP_APP" ]] || rm -rf "$BACKUP_APP"
trap - EXIT

print "Installed current Atogaki: $DESTINATION_APP"
