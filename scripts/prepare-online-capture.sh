#!/bin/bash
# Native capture stays a separate 14.2+ arm64 helper; offline App stays 12.0+.
set -eu
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
target_output="${1:-$repo_root/local-artifacts/online/native/atogaki-audio-tap}"
mkdir -p "$(dirname "$target_output")"
xcrun swiftc -swift-version 5 -O -target arm64-apple-macos14.2 \
  -module-cache-path "${TMPDIR:-/tmp}/atogaki-online-swift-modules" \
  "$repo_root/src/infrastructure/online_capture/main.swift" \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist \
  -Xlinker "$repo_root/src/infrastructure/online_capture/Info.plist" \
  -o "$target_output"
codesign --force --sign - "$target_output"
"$target_output" --check
