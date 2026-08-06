#!/bin/zsh
set -euo pipefail

SCRIPT_DIR=${0:A:h}
PROJECT_DIR=${SCRIPT_DIR:h}
SOURCE_CACHE=${ATOGAKI_SIDECAR_SOURCE_CACHE:-${TMPDIR:-/tmp}/atogaki-sidecar-source-cache}
OUTPUT_DIR=${1:-$PROJECT_DIR/src-tauri/target/release/bundle/sources}
MANIFEST="$PROJECT_DIR/src-tauri/third-party/build-manifest.txt"
source "$SCRIPT_DIR/sidecar-versions.zsh"

for command_name in curl node shasum tar; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    print -u2 "missing source archive dependency: $command_name"
    exit 1
  fi
done

if [[ ! -f "$MANIFEST" ]]; then
  print -u2 "missing sidecar build manifest: $MANIFEST"
  exit 1
fi

require_manifest_value() {
  local key=$1
  local expected=$2
  local actual=$(sed -n "s/^$key=//p" "$MANIFEST")
  if [[ "$actual" != "$expected" ]]; then
    print -u2 "sidecar manifest mismatch for $key: expected $expected, found ${actual:-missing}"
    exit 1
  fi
}

require_manifest_value whisper_version "$WHISPER_VERSION"
require_manifest_value whisper_commit "$WHISPER_COMMIT"
require_manifest_value whisper_source_sha256 "$WHISPER_SOURCE_SHA256"
require_manifest_value ffmpeg_version "$FFMPEG_VERSION"
require_manifest_value ffmpeg_source_sha256 "$FFMPEG_SHA256"
require_manifest_value libass_version "$LIBASS_VERSION"
require_manifest_value libass_source_sha256 "$LIBASS_SHA256"
require_manifest_value libunibreak_version "$LIBUNIBREAK_VERSION"
require_manifest_value libunibreak_source_sha256 "$LIBUNIBREAK_SHA256"
require_manifest_value fribidi_version "$FRIBIDI_VERSION"
require_manifest_value fribidi_source_sha256 "$FRIBIDI_SHA256"
require_manifest_value freetype_version "$FREETYPE_VERSION"
require_manifest_value freetype_source_sha256 "$FREETYPE_SHA256"
require_manifest_value harfbuzz_version "$HARFBUZZ_VERSION"
require_manifest_value harfbuzz_source_sha256 "$HARFBUZZ_SHA256"

TARGET_TRIPLE=$(sed -n 's/^target=//p' "$MANIFEST")
if [[ "$TARGET_TRIPLE" != "aarch64-apple-darwin" && "$TARGET_TRIPLE" != "x86_64-apple-darwin" ]]; then
  print -u2 "unsupported sidecar manifest target: ${TARGET_TRIPLE:-missing}"
  exit 1
fi

require_binary_sha256() {
  local binary_name=$1
  local manifest_key=$2
  local binary_path="$PROJECT_DIR/src-tauri/binaries/$binary_name-$TARGET_TRIPLE"
  local expected=$(sed -n "s/^$manifest_key=//p" "$MANIFEST")
  if [[ ! -x "$binary_path" ]]; then
    print -u2 "missing executable sidecar: $binary_path"
    exit 1
  fi
  print "$expected  $binary_path" | shasum -a 256 --check
}

require_binary_sha256 ffmpeg ffmpeg_binary_sha256
require_binary_sha256 ffprobe ffprobe_binary_sha256
require_binary_sha256 whisper-cli whisper_binary_sha256

FFMPEG_CONFIGURATION=$(sed -n 's/^ffmpeg_configuration=//p' "$MANIFEST")
if [[ "$FFMPEG_CONFIGURATION" != *"--disable-gpl"* || "$FFMPEG_CONFIGURATION" != *"--disable-nonfree"* ]]; then
  print -u2 "FFmpeg manifest is missing the required LGPL distribution flags"
  exit 1
fi
if [[ "$FFMPEG_CONFIGURATION" == *"--enable-gpl"* || "$FFMPEG_CONFIGURATION" == *"--enable-nonfree"* || "$FFMPEG_CONFIGURATION" == *"libx264"* ]]; then
  print -u2 "FFmpeg manifest contains a forbidden GPL/nonfree component"
  exit 1
fi

mkdir -p "$SOURCE_CACHE" "$OUTPUT_DIR"
STAGING_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/atogaki-release-sources.XXXXXX")
cleanup() {
  exit_code=$?
  if (( exit_code == 0 )); then
    rm -rf "$STAGING_ROOT"
  else
    print -u2 "source packaging failed; retained diagnostics at: $STAGING_ROOT"
  fi
}
trap cleanup EXIT

download_and_verify() {
  local url=$1
  local output=$2
  local sha256=$3
  if [[ ! -f "$output" ]] || ! print "$sha256  $output" | shasum -a 256 --check >/dev/null 2>&1; then
    curl --fail --location --retry 3 --output "$output" "$url"
  fi
  print "$sha256  $output" | shasum -a 256 --check
}

APP_VERSION=$(node -e 'console.log(require(process.argv[1]).version)' "$PROJECT_DIR/src-tauri/tauri.conf.json")
PACKAGE_NAME="Atogaki-$APP_VERSION-third-party-sources"
PACKAGE_ROOT="$STAGING_ROOT/$PACKAGE_NAME"
SOURCES_DIR="$PACKAGE_ROOT/sources"
mkdir -p "$SOURCES_DIR" "$PACKAGE_ROOT/build" "$PACKAGE_ROOT/licenses"

collect_source() {
  local file_name=$1
  local url=$2
  local sha256=$3
  local cached="$SOURCE_CACHE/$file_name"
  download_and_verify "$url" "$cached" "$sha256"
  cp "$cached" "$SOURCES_DIR/$file_name"
}

collect_source "whisper.cpp-$WHISPER_COMMIT.tar.gz" "$WHISPER_SOURCE_URL" "$WHISPER_SOURCE_SHA256"
collect_source "ffmpeg-$FFMPEG_VERSION.tar.xz" "$FFMPEG_SOURCE_URL" "$FFMPEG_SHA256"
collect_source "libass-$LIBASS_VERSION.tar.xz" "$LIBASS_SOURCE_URL" "$LIBASS_SHA256"
collect_source "libunibreak-$LIBUNIBREAK_VERSION.tar.gz" "$LIBUNIBREAK_SOURCE_URL" "$LIBUNIBREAK_SHA256"
collect_source "fribidi-$FRIBIDI_VERSION.tar.xz" "$FRIBIDI_SOURCE_URL" "$FRIBIDI_SHA256"
collect_source "freetype-$FREETYPE_VERSION.tar.xz" "$FREETYPE_SOURCE_URL" "$FREETYPE_SHA256"
collect_source "harfbuzz-$HARFBUZZ_VERSION.tar.xz" "$HARFBUZZ_SOURCE_URL" "$HARFBUZZ_SHA256"

cp "$PROJECT_DIR/LICENSE" "$PACKAGE_ROOT/ATOGAKI-LICENSE"
cp "$SCRIPT_DIR/build-sidecars-macos.sh" "$PACKAGE_ROOT/build/"
cp "$SCRIPT_DIR/sidecar-versions.zsh" "$PACKAGE_ROOT/build/"
cp "$MANIFEST" "$PACKAGE_ROOT/build/"
cp "$PROJECT_DIR/src-tauri/third-party/README.md" "$PACKAGE_ROOT/README.md"
cp -R "$PROJECT_DIR/src-tauri/third-party/licenses/." "$PACKAGE_ROOT/licenses/"

(
  cd "$SOURCES_DIR"
  shasum -a 256 * > SHA256SUMS
)

cat > "$PACKAGE_ROOT/SOURCES.md" <<EOF
# Atogaki $APP_VERSION corresponding sidecar sources

This archive contains the exact upstream source archives used for the macOS
Atogaki sidecars, together with the build script, pinned version file, binary
build manifest, checksums, and license texts.

The FFmpeg sidecar is an LGPL v2.1-or-later build without GPL, nonfree, or
libx264 components. Its statically linked subtitle stack sources are included:
libass, libunibreak, FriBidi, FreeType, and HarfBuzz. whisper.cpp is included
for the separately distributed MIT-licensed whisper-cli sidecar.

Run build/build-sidecars-macos.sh from the root of the Atogaki source tree to
rebuild the target recorded in build/build-manifest.txt. The script expects the
normal macOS build dependencies documented by the repository.
EOF

OUTPUT_ARCHIVE="$OUTPUT_DIR/$PACKAGE_NAME.tar.xz"
tar -cJf "$OUTPUT_ARCHIVE" -C "$STAGING_ROOT" "$PACKAGE_NAME"
(
  cd "$OUTPUT_DIR"
  shasum -a 256 "$(basename "$OUTPUT_ARCHIVE")" > "$(basename "$OUTPUT_ARCHIVE").sha256"
)

print "Packaged corresponding sidecar sources:"
print "  $OUTPUT_ARCHIVE"
print "  $OUTPUT_ARCHIVE.sha256"
