#!/bin/zsh
set -euo pipefail

SCRIPT_DIR=${0:A:h}
PROJECT_DIR=${SCRIPT_DIR:h}
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"
NOTICES_DIR="$PROJECT_DIR/src-tauri/third-party"
TARGET_TRIPLE=$(rustc --print host-tuple)
JOBS=$(sysctl -n hw.logicalcpu)
MACOS_MINIMUM=${MACOSX_DEPLOYMENT_TARGET:-12.0}

WHISPER_VERSION=v1.8.6
WHISPER_COMMIT_PREFIX=23ee035
FFMPEG_VERSION=8.1.2
FFMPEG_SHA256=464beb5e7bf0c311e68b45ae2f04e9cc2af88851abb4082231742a74d97b524c
LIBASS_VERSION=0.17.5
LIBASS_SHA256=2dca25c0e0c837ddf00b52011b3f82cac1e4ddd3ad018227806b0c2288864acc
LIBUNIBREAK_VERSION=7.0
LIBUNIBREAK_SHA256=8c9a6e121736cd0d5c890ae3ae96f3f4010a19aa040f1dbded833a62a87717d3
FRIBIDI_VERSION=1.0.16
FRIBIDI_SHA256=1b1cde5b235d40479e91be2f0e88a309e3214c8ab470ec8a2744d82a5a9ea05c
FREETYPE_VERSION=2.14.3
FREETYPE_SHA256=36bc4f1cc413335368ee656c42afca65c5a3987e8768cc28cf11ba775e785a5f
HARFBUZZ_VERSION=14.3.0
HARFBUZZ_SHA256=16070d77cfc4ba1f1e7327e83bf9b3f55898081cabdb94e56a33e04fc8874eae

case "$TARGET_TRIPLE" in
  aarch64-apple-darwin|x86_64-apple-darwin) ;;
  *)
    print -u2 "unsupported macOS sidecar target: $TARGET_TRIPLE"
    exit 1
    ;;
esac

for command_name in cmake curl git make meson ninja pkg-config shasum otool; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    print -u2 "missing sidecar build dependency: $command_name"
    exit 1
  fi
done

BUILD_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/atogaki-sidecars.XXXXXX")
SOURCE_CACHE=${ATOGAKI_SIDECAR_SOURCE_CACHE:-${TMPDIR:-/tmp}/atogaki-sidecar-source-cache}
cleanup() {
  exit_code=$?
  if (( exit_code == 0 )); then
    rm -rf "$BUILD_ROOT"
  else
    print -u2 "sidecar build failed; retained diagnostics at: $BUILD_ROOT"
  fi
}
trap cleanup EXIT

mkdir -p "$BINARIES_DIR" "$NOTICES_DIR/licenses" "$SOURCE_CACHE"
DEPS_PREFIX="$BUILD_ROOT/deps-install"
mkdir -p "$DEPS_PREFIX"

download_and_verify() {
  local url=$1
  local output=$2
  local sha256=$3
  if [[ -f "$output" ]] && print "$sha256  $output" | shasum -a 256 --check >/dev/null 2>&1; then
    print "Using cached source: $output"
    return
  fi
  curl --fail --location --retry 3 --output "$output" "$url"
  print "$sha256  $output" | shasum -a 256 --check
}

export CFLAGS="-O2 -mmacosx-version-min=$MACOS_MINIMUM"
export CXXFLAGS="$CFLAGS"
export LDFLAGS="-mmacosx-version-min=$MACOS_MINIMUM"
export PKG_CONFIG_PATH="$DEPS_PREFIX/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$PKG_CONFIG_PATH"

# Build the ASS shaping/font stack from pinned sources. Homebrew bottles are
# intentionally not linked: their install names and deployment target would
# make the App depend on the build machine.
LIBUNIBREAK_ARCHIVE="$SOURCE_CACHE/libunibreak-$LIBUNIBREAK_VERSION.tar.gz"
download_and_verify \
  "https://github.com/adah1972/libunibreak/releases/download/libunibreak_7_0/libunibreak-$LIBUNIBREAK_VERSION.tar.gz" \
  "$LIBUNIBREAK_ARCHIVE" "$LIBUNIBREAK_SHA256"
tar -xf "$LIBUNIBREAK_ARCHIVE" -C "$BUILD_ROOT"
pushd "$BUILD_ROOT/libunibreak-$LIBUNIBREAK_VERSION" >/dev/null
./configure --prefix="$DEPS_PREFIX" --disable-shared --enable-static
make -j "$JOBS"
make install
popd >/dev/null

FRIBIDI_ARCHIVE="$SOURCE_CACHE/fribidi-$FRIBIDI_VERSION.tar.xz"
download_and_verify \
  "https://github.com/fribidi/fribidi/releases/download/v$FRIBIDI_VERSION/fribidi-$FRIBIDI_VERSION.tar.xz" \
  "$FRIBIDI_ARCHIVE" "$FRIBIDI_SHA256"
tar -xf "$FRIBIDI_ARCHIVE" -C "$BUILD_ROOT"
meson setup "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/build-atogaki" \
  "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION" \
  --prefix "$DEPS_PREFIX" --libdir lib --default-library static \
  -Ddocs=false -Dbin=false -Dtests=false
meson compile -C "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/build-atogaki" -j "$JOBS"
meson install -C "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/build-atogaki"

FREETYPE_ARCHIVE="$SOURCE_CACHE/freetype-$FREETYPE_VERSION.tar.xz"
download_and_verify \
  "https://downloads.sourceforge.net/project/freetype/freetype2/$FREETYPE_VERSION/freetype-$FREETYPE_VERSION.tar.xz" \
  "$FREETYPE_ARCHIVE" "$FREETYPE_SHA256"
tar -xf "$FREETYPE_ARCHIVE" -C "$BUILD_ROOT"
cmake -S "$BUILD_ROOT/freetype-$FREETYPE_VERSION" \
  -B "$BUILD_ROOT/freetype-$FREETYPE_VERSION/build-atogaki" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$DEPS_PREFIX" \
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$MACOS_MINIMUM" \
  -DBUILD_SHARED_LIBS=OFF \
  -DFT_DISABLE_ZLIB=TRUE \
  -DFT_DISABLE_BZIP2=TRUE \
  -DFT_DISABLE_PNG=TRUE \
  -DFT_DISABLE_HARFBUZZ=TRUE \
  -DFT_DISABLE_BROTLI=TRUE
cmake --build "$BUILD_ROOT/freetype-$FREETYPE_VERSION/build-atogaki" -j "$JOBS"
cmake --install "$BUILD_ROOT/freetype-$FREETYPE_VERSION/build-atogaki"

HARFBUZZ_ARCHIVE="$SOURCE_CACHE/harfbuzz-$HARFBUZZ_VERSION.tar.xz"
download_and_verify \
  "https://github.com/harfbuzz/harfbuzz/releases/download/$HARFBUZZ_VERSION/harfbuzz-$HARFBUZZ_VERSION.tar.xz" \
  "$HARFBUZZ_ARCHIVE" "$HARFBUZZ_SHA256"
tar -xf "$HARFBUZZ_ARCHIVE" -C "$BUILD_ROOT"
meson setup "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/build-atogaki" \
  "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION" \
  --prefix "$DEPS_PREFIX" --libdir lib --default-library static \
  -Dfreetype=enabled \
  -Dglib=disabled -Dgobject=disabled -Dcairo=disabled -Dchafa=disabled \
  -Dicu=disabled -Dgraphite=disabled -Ddocs=disabled -Dtests=disabled \
  -Dutilities=disabled -Dintrospection=disabled
meson compile -C "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/build-atogaki" -j "$JOBS"
meson install -C "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/build-atogaki"

LIBASS_ARCHIVE="$SOURCE_CACHE/libass-$LIBASS_VERSION.tar.xz"
download_and_verify \
  "https://github.com/libass/libass/releases/download/$LIBASS_VERSION/libass-$LIBASS_VERSION.tar.xz" \
  "$LIBASS_ARCHIVE" "$LIBASS_SHA256"
tar -xf "$LIBASS_ARCHIVE" -C "$BUILD_ROOT"
pushd "$BUILD_ROOT/libass-$LIBASS_VERSION" >/dev/null
./configure --prefix="$DEPS_PREFIX" --disable-shared --enable-static --disable-fontconfig
make -j "$JOBS"
make install
popd >/dev/null

WHISPER_SOURCE="$BUILD_ROOT/whisper.cpp"
git clone --branch "$WHISPER_VERSION" --depth 1 https://github.com/ggml-org/whisper.cpp.git "$WHISPER_SOURCE"
WHISPER_COMMIT=$(git -C "$WHISPER_SOURCE" rev-parse HEAD)
if [[ "$WHISPER_COMMIT" != ${WHISPER_COMMIT_PREFIX}* ]]; then
  print -u2 "unexpected whisper.cpp commit for $WHISPER_VERSION: $WHISPER_COMMIT"
  exit 1
fi

cmake -S "$WHISPER_SOURCE" -B "$WHISPER_SOURCE/build-atogaki" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$MACOS_MINIMUM" \
  -DBUILD_SHARED_LIBS=OFF \
  -DGGML_ACCELERATE=ON \
  -DGGML_METAL=ON \
  -DGGML_NATIVE=OFF \
  -DWHISPER_BUILD_EXAMPLES=ON \
  -DWHISPER_BUILD_SERVER=OFF \
  -DWHISPER_BUILD_TESTS=OFF \
  -DWHISPER_SDL2=OFF
cmake --build "$WHISPER_SOURCE/build-atogaki" --config Release --target whisper-cli -j "$JOBS"
install -m 755 "$WHISPER_SOURCE/build-atogaki/bin/whisper-cli" "$BINARIES_DIR/whisper-cli-$TARGET_TRIPLE"

FFMPEG_ARCHIVE="$SOURCE_CACHE/ffmpeg-$FFMPEG_VERSION.tar.xz"
FFMPEG_SOURCE="$BUILD_ROOT/ffmpeg-$FFMPEG_VERSION"
download_and_verify \
  "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" \
  "$FFMPEG_ARCHIVE" "$FFMPEG_SHA256"
tar -xf "$FFMPEG_ARCHIVE" -C "$BUILD_ROOT"

FFMPEG_DESTDIR="$BUILD_ROOT/ffmpeg-install"
pushd "$FFMPEG_SOURCE" >/dev/null
CFLAGS="$CFLAGS -I$DEPS_PREFIX/include" \
LDFLAGS="$LDFLAGS -L$DEPS_PREFIX/lib" \
./configure \
  --prefix=/atogaki-sidecar \
  --cc=clang \
  --cxx=clang++ \
  --pkg-config-flags=--static \
  --extra-cflags="-mmacosx-version-min=$MACOS_MINIMUM" \
  --extra-ldflags="-mmacosx-version-min=$MACOS_MINIMUM" \
  --disable-autodetect \
  --disable-gpl \
  --disable-nonfree \
  --disable-network \
  --disable-doc \
  --disable-debug \
  --disable-ffplay \
  --disable-shared \
  --enable-static \
  --enable-bzlib \
  --enable-iconv \
  --enable-zlib \
  --enable-libass \
  --enable-videotoolbox \
  --enable-audiotoolbox
make -j "$JOBS"
make DESTDIR="$FFMPEG_DESTDIR" install
popd >/dev/null

install -m 755 "$FFMPEG_DESTDIR/atogaki-sidecar/bin/ffmpeg" "$BINARIES_DIR/ffmpeg-$TARGET_TRIPLE"
install -m 755 "$FFMPEG_DESTDIR/atogaki-sidecar/bin/ffprobe" "$BINARIES_DIR/ffprobe-$TARGET_TRIPLE"

FFMPEG_BINARY="$BINARIES_DIR/ffmpeg-$TARGET_TRIPLE"
FFPROBE_BINARY="$BINARIES_DIR/ffprobe-$TARGET_TRIPLE"
WHISPER_BINARY="$BINARIES_DIR/whisper-cli-$TARGET_TRIPLE"

FFMPEG_CONFIGURATION=$($FFMPEG_BINARY -version | sed -n '3p')
FFMPEG_LICENSE=$($FFMPEG_BINARY -L 2>&1)
FFMPEG_ENCODERS=$($FFMPEG_BINARY -hide_banner -encoders 2>/dev/null)
FFMPEG_FILTERS=$($FFMPEG_BINARY -hide_banner -filters 2>/dev/null)
if [[ "$FFMPEG_CONFIGURATION" == *"--enable-gpl"* || "$FFMPEG_CONFIGURATION" == *"--enable-nonfree"* || "$FFMPEG_CONFIGURATION" == *"--enable-libx264"* ]]; then
  print -u2 "FFmpeg sidecar contains a forbidden GPL/nonfree build option"
  exit 1
fi
if [[ "$FFMPEG_LICENSE" != *"GNU Lesser General Public"* ]]; then
  print -u2 "FFmpeg sidecar did not report an LGPL license"
  exit 1
fi
if [[ "$FFMPEG_ENCODERS" == *" libx264 "* ]]; then
  print -u2 "FFmpeg sidecar unexpectedly contains libx264"
  exit 1
fi
for codec_name in h264_videotoolbox mpeg4; do
  if [[ "$FFMPEG_ENCODERS" != *" $codec_name "* ]]; then
    print -u2 "FFmpeg sidecar is missing encoder: $codec_name"
    exit 1
  fi
done
if [[ "$FFMPEG_FILTERS" != *" ass "* ]]; then
  print -u2 "FFmpeg sidecar is missing the libass filter"
  exit 1
fi

for binary in "$FFMPEG_BINARY" "$FFPROBE_BINARY" "$WHISPER_BINARY"; do
  if otool -L "$binary" | grep -E '/opt/homebrew|/usr/local|atogaki-sidecars' >/dev/null; then
    print -u2 "sidecar has a non-system build-machine dependency: $binary"
    otool -L "$binary" >&2
    exit 1
  fi
done

rm -rf "$NOTICES_DIR/licenses/ffmpeg" "$NOTICES_DIR/licenses/whisper.cpp" \
  "$NOTICES_DIR/licenses/libass" "$NOTICES_DIR/licenses/libunibreak" \
  "$NOTICES_DIR/licenses/fribidi" "$NOTICES_DIR/licenses/freetype" \
  "$NOTICES_DIR/licenses/harfbuzz"
mkdir -p "$NOTICES_DIR/licenses/ffmpeg" "$NOTICES_DIR/licenses/whisper.cpp" \
  "$NOTICES_DIR/licenses/libass" "$NOTICES_DIR/licenses/libunibreak" \
  "$NOTICES_DIR/licenses/fribidi" "$NOTICES_DIR/licenses/freetype" \
  "$NOTICES_DIR/licenses/harfbuzz"
cp "$FFMPEG_SOURCE/COPYING.LGPLv2.1" "$NOTICES_DIR/licenses/ffmpeg/"
cp "$WHISPER_SOURCE/LICENSE" "$NOTICES_DIR/licenses/whisper.cpp/"
cp "$BUILD_ROOT/libass-$LIBASS_VERSION/COPYING" "$NOTICES_DIR/licenses/libass/"
cp "$BUILD_ROOT/libunibreak-$LIBUNIBREAK_VERSION/LICENCE" "$NOTICES_DIR/licenses/libunibreak/"
cp "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/COPYING" "$NOTICES_DIR/licenses/fribidi/"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/LICENSE.TXT" "$NOTICES_DIR/licenses/freetype/"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/docs/FTL.TXT" "$NOTICES_DIR/licenses/freetype/"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/src/bdf/README" "$NOTICES_DIR/licenses/freetype/BDF-README.txt"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/src/pcf/README" "$NOTICES_DIR/licenses/freetype/PCF-README.txt"
cp "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/COPYING" "$NOTICES_DIR/licenses/harfbuzz/"
cp "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/src/ms-use/COPYING" "$NOTICES_DIR/licenses/harfbuzz/ms-use-COPYING"

{
  print "target=$TARGET_TRIPLE"
  print "macos_minimum=$MACOS_MINIMUM"
  print "whisper_version=$WHISPER_VERSION"
  print "whisper_commit=$WHISPER_COMMIT"
  print "ffmpeg_version=$FFMPEG_VERSION"
  print "ffmpeg_source_sha256=$FFMPEG_SHA256"
  print "libass_version=$LIBASS_VERSION"
  print "libunibreak_version=$LIBUNIBREAK_VERSION"
  print "fribidi_version=$FRIBIDI_VERSION"
  print "freetype_version=$FREETYPE_VERSION"
  print "harfbuzz_version=$HARFBUZZ_VERSION"
  print "ffmpeg_configuration=$FFMPEG_CONFIGURATION"
  print "ffmpeg_binary_sha256=$(shasum -a 256 "$FFMPEG_BINARY" | awk '{print $1}')"
  print "ffprobe_binary_sha256=$(shasum -a 256 "$FFPROBE_BINARY" | awk '{print $1}')"
  print "whisper_binary_sha256=$(shasum -a 256 "$WHISPER_BINARY" | awk '{print $1}')"
} > "$NOTICES_DIR/build-manifest.txt"

print "Built and verified Atogaki sidecars for $TARGET_TRIPLE"
print "  $FFMPEG_BINARY"
print "  $FFPROBE_BINARY"
print "  $WHISPER_BINARY"
