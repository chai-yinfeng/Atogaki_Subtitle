#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"
NOTICES_DIR="$PROJECT_DIR/src-tauri/third-party"
BUILD_ROOT="$PROJECT_DIR/target/windows-sidecars"
SOURCE_CACHE="$PROJECT_DIR/target/sidecar-source-cache"
DEPS_PREFIX="$BUILD_ROOT/ucrt64-static"
TARGET_TRIPLE=x86_64-pc-windows-msvc
JOBS=${NUMBER_OF_PROCESSORS:-2}
source "$SCRIPT_DIR/sidecar-versions.zsh"

if [[ ${MSYSTEM:-} != UCRT64 ]]; then
  printf 'This script must run in an MSYS2 UCRT64 shell.\n' >&2
  exit 1
fi

for command_name in cmake curl make meson ninja pkg-config sha256sum tar objdump; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Missing Windows sidecar build dependency: %s\n' "$command_name" >&2
    exit 1
  fi
done

mkdir -p "$BINARIES_DIR" "$NOTICES_DIR/licenses" "$SOURCE_CACHE" "$DEPS_PREFIX"

download_and_verify() {
  local url=$1
  local output=$2
  local sha256=$3
  if [[ -f $output ]] && printf '%s  %s\n' "$sha256" "$output" | sha256sum --check --status; then
    printf 'Using cached source: %s\n' "$output"
    return
  fi
  curl --fail --location --retry 3 --output "$output" "$url"
  printf '%s  %s\n' "$sha256" "$output" | sha256sum --check
}

export PATH="/ucrt64/bin:/usr/bin:$PATH"
export PKG_CONFIG_PATH="$DEPS_PREFIX/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$PKG_CONFIG_PATH"
export CFLAGS="-O2"
export CXXFLAGS="-O2"

LIBUNIBREAK_ARCHIVE="$SOURCE_CACHE/libunibreak-$LIBUNIBREAK_VERSION.tar.gz"
download_and_verify "$LIBUNIBREAK_SOURCE_URL" "$LIBUNIBREAK_ARCHIVE" "$LIBUNIBREAK_SHA256"
rm -rf "$BUILD_ROOT/libunibreak-$LIBUNIBREAK_VERSION"
tar -xf "$LIBUNIBREAK_ARCHIVE" -C "$BUILD_ROOT"
pushd "$BUILD_ROOT/libunibreak-$LIBUNIBREAK_VERSION" >/dev/null
./configure --prefix="$DEPS_PREFIX" --disable-shared --enable-static
make -j "$JOBS"
make install
popd >/dev/null

FRIBIDI_ARCHIVE="$SOURCE_CACHE/fribidi-$FRIBIDI_VERSION.tar.xz"
download_and_verify "$FRIBIDI_SOURCE_URL" "$FRIBIDI_ARCHIVE" "$FRIBIDI_SHA256"
rm -rf "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION"
tar -xf "$FRIBIDI_ARCHIVE" -C "$BUILD_ROOT"
meson setup "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/build-atogaki" \
  "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION" \
  --prefix "$DEPS_PREFIX" --libdir lib --default-library static \
  -Ddocs=false -Dbin=false -Dtests=false
meson compile -C "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/build-atogaki" -j "$JOBS"
meson install -C "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/build-atogaki"

FREETYPE_ARCHIVE="$SOURCE_CACHE/freetype-$FREETYPE_VERSION.tar.xz"
download_and_verify "$FREETYPE_SOURCE_URL" "$FREETYPE_ARCHIVE" "$FREETYPE_SHA256"
rm -rf "$BUILD_ROOT/freetype-$FREETYPE_VERSION"
tar -xf "$FREETYPE_ARCHIVE" -C "$BUILD_ROOT"
cmake -S "$BUILD_ROOT/freetype-$FREETYPE_VERSION" \
  -B "$BUILD_ROOT/freetype-$FREETYPE_VERSION/build-atogaki" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$DEPS_PREFIX" \
  -DBUILD_SHARED_LIBS=OFF \
  -DFT_DISABLE_ZLIB=TRUE -DFT_DISABLE_BZIP2=TRUE -DFT_DISABLE_PNG=TRUE \
  -DFT_DISABLE_HARFBUZZ=TRUE -DFT_DISABLE_BROTLI=TRUE
cmake --build "$BUILD_ROOT/freetype-$FREETYPE_VERSION/build-atogaki" -j "$JOBS"
cmake --install "$BUILD_ROOT/freetype-$FREETYPE_VERSION/build-atogaki"

HARFBUZZ_ARCHIVE="$SOURCE_CACHE/harfbuzz-$HARFBUZZ_VERSION.tar.xz"
download_and_verify "$HARFBUZZ_SOURCE_URL" "$HARFBUZZ_ARCHIVE" "$HARFBUZZ_SHA256"
rm -rf "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION"
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
download_and_verify "$LIBASS_SOURCE_URL" "$LIBASS_ARCHIVE" "$LIBASS_SHA256"
rm -rf "$BUILD_ROOT/libass-$LIBASS_VERSION"
tar -xf "$LIBASS_ARCHIVE" -C "$BUILD_ROOT"
pushd "$BUILD_ROOT/libass-$LIBASS_VERSION" >/dev/null
./configure --prefix="$DEPS_PREFIX" --disable-shared --enable-static \
  --disable-fontconfig --enable-directwrite --enable-libunibreak
make -j "$JOBS"
make install
popd >/dev/null

FFMPEG_ARCHIVE="$SOURCE_CACHE/ffmpeg-$FFMPEG_VERSION.tar.xz"
FFMPEG_SOURCE="$BUILD_ROOT/ffmpeg-$FFMPEG_VERSION"
download_and_verify "$FFMPEG_SOURCE_URL" "$FFMPEG_ARCHIVE" "$FFMPEG_SHA256"
rm -rf "$FFMPEG_SOURCE" "$BUILD_ROOT/ffmpeg-install"
tar -xf "$FFMPEG_ARCHIVE" -C "$BUILD_ROOT"

pushd "$FFMPEG_SOURCE" >/dev/null
./configure \
  --prefix="$BUILD_ROOT/ffmpeg-install" \
  --target-os=mingw32 \
  --arch=x86_64 \
  --pkg-config-flags=--static \
  --extra-cflags="-I$DEPS_PREFIX/include" \
  --extra-ldflags="-L$DEPS_PREFIX/lib -static -static-libgcc -static-libstdc++" \
  --extra-libs="-lstdc++" \
  --disable-autodetect \
  --disable-gpl \
  --disable-nonfree \
  --disable-network \
  --disable-doc \
  --disable-debug \
  --disable-ffplay \
  --disable-shared \
  --enable-static \
  --enable-libass
make -j "$JOBS"
make install
popd >/dev/null

install -m 755 "$BUILD_ROOT/ffmpeg-install/bin/ffmpeg.exe" "$BINARIES_DIR/ffmpeg-$TARGET_TRIPLE.exe"
install -m 755 "$BUILD_ROOT/ffmpeg-install/bin/ffprobe.exe" "$BINARIES_DIR/ffprobe-$TARGET_TRIPLE.exe"

FFMPEG_BINARY="$BINARIES_DIR/ffmpeg-$TARGET_TRIPLE.exe"
FFPROBE_BINARY="$BINARIES_DIR/ffprobe-$TARGET_TRIPLE.exe"
WHISPER_BINARY="$BINARIES_DIR/whisper-cli-$TARGET_TRIPLE.exe"
if [[ ! -x $WHISPER_BINARY ]]; then
  printf 'Missing MSVC whisper sidecar: %s\n' "$WHISPER_BINARY" >&2
  exit 1
fi

FFMPEG_CONFIGURATION=$($FFMPEG_BINARY -version | sed -n '3p')
FFMPEG_LICENSE=$($FFMPEG_BINARY -L 2>&1)
FFMPEG_ENCODERS=$($FFMPEG_BINARY -hide_banner -encoders 2>/dev/null)
FFMPEG_FILTERS=$($FFMPEG_BINARY -hide_banner -filters 2>/dev/null)
if [[ $FFMPEG_CONFIGURATION == *--enable-gpl* || $FFMPEG_CONFIGURATION == *--enable-nonfree* || $FFMPEG_CONFIGURATION == *--enable-libx264* ]]; then
  printf 'FFmpeg contains a forbidden GPL/nonfree build option.\n' >&2
  exit 1
fi
if [[ $FFMPEG_CONFIGURATION != *--disable-gpl* || $FFMPEG_CONFIGURATION != *--disable-nonfree* ]]; then
  printf 'FFmpeg does not record the required LGPL-only flags.\n' >&2
  exit 1
fi
if [[ $FFMPEG_LICENSE != *'GNU Lesser General Public'* ]]; then
  printf 'FFmpeg did not report an LGPL license.\n' >&2
  exit 1
fi
if [[ $FFMPEG_ENCODERS == *' libx264 '* || $FFMPEG_ENCODERS != *' mpeg4 '* ]]; then
  printf 'FFmpeg encoder policy check failed.\n' >&2
  exit 1
fi
if [[ $FFMPEG_FILTERS != *' ass '* ]]; then
  printf 'FFmpeg is missing the libass filter.\n' >&2
  exit 1
fi

FORBIDDEN_DLL_PATTERN='(msys-2\.0|libgcc_s|libstdc\+\+|libwinpthread|libass|libfreetype|libfribidi|libharfbuzz|libunibreak).*\.dll'
for binary in "$FFMPEG_BINARY" "$FFPROBE_BINARY" "$WHISPER_BINARY"; do
  dependencies=$(objdump -p "$binary" | sed -n 's/^[[:space:]]*DLL Name: //p')
  printf 'Runtime dependencies for %s:\n%s\n' "$(basename "$binary")" "$dependencies"
  if printf '%s\n' "$dependencies" | grep -Eiq "$FORBIDDEN_DLL_PATTERN"; then
    printf 'Sidecar has an unexpected toolchain or subtitle-stack DLL dependency: %s\n' "$binary" >&2
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
cp "$BUILD_ROOT/whisper.cpp-$WHISPER_COMMIT/LICENSE" "$NOTICES_DIR/licenses/whisper.cpp/"
cp "$BUILD_ROOT/libass-$LIBASS_VERSION/COPYING" "$NOTICES_DIR/licenses/libass/"
cp "$BUILD_ROOT/libunibreak-$LIBUNIBREAK_VERSION/LICENCE" "$NOTICES_DIR/licenses/libunibreak/"
cp "$BUILD_ROOT/fribidi-$FRIBIDI_VERSION/COPYING" "$NOTICES_DIR/licenses/fribidi/"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/LICENSE.TXT" "$NOTICES_DIR/licenses/freetype/"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/docs/FTL.TXT" "$NOTICES_DIR/licenses/freetype/"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/src/bdf/README" "$NOTICES_DIR/licenses/freetype/BDF-README.txt"
cp "$BUILD_ROOT/freetype-$FREETYPE_VERSION/src/pcf/README" "$NOTICES_DIR/licenses/freetype/PCF-README.txt"
cp "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/COPYING" "$NOTICES_DIR/licenses/harfbuzz/"
cp "$BUILD_ROOT/harfbuzz-$HARFBUZZ_VERSION/src/ms-use/COPYING" "$NOTICES_DIR/licenses/harfbuzz/ms-use-COPYING"

cat > "$NOTICES_DIR/build-manifest.txt" <<EOF
target=$TARGET_TRIPLE
windows_toolchain=msvc+msys2-ucrt64
whisper_version=$WHISPER_VERSION
whisper_commit=$WHISPER_COMMIT
whisper_source_sha256=$WHISPER_SOURCE_SHA256
ffmpeg_version=$FFMPEG_VERSION
ffmpeg_source_sha256=$FFMPEG_SHA256
libass_version=$LIBASS_VERSION
libass_source_sha256=$LIBASS_SHA256
libunibreak_version=$LIBUNIBREAK_VERSION
libunibreak_source_sha256=$LIBUNIBREAK_SHA256
fribidi_version=$FRIBIDI_VERSION
fribidi_source_sha256=$FRIBIDI_SHA256
freetype_version=$FREETYPE_VERSION
freetype_source_sha256=$FREETYPE_SHA256
harfbuzz_version=$HARFBUZZ_VERSION
harfbuzz_source_sha256=$HARFBUZZ_SHA256
ffmpeg_configuration=$FFMPEG_CONFIGURATION
prebundle_ffmpeg_binary_sha256=$(sha256sum "$FFMPEG_BINARY" | awk '{print $1}')
prebundle_ffprobe_binary_sha256=$(sha256sum "$FFPROBE_BINARY" | awk '{print $1}')
prebundle_whisper_binary_sha256=$(sha256sum "$WHISPER_BINARY" | awk '{print $1}')
EOF

printf 'Built and verified Atogaki Windows sidecars.\n'
