# Pinned macOS sidecar sources. This file is sourced by both the sidecar build
# and release-source archive scripts so their version and checksum baselines
# cannot drift independently.

WHISPER_VERSION=v1.8.6
WHISPER_COMMIT=23ee03506a91ac3d3f0071b40e66a430eebdfa1d
WHISPER_SOURCE_URL="https://github.com/ggml-org/whisper.cpp/archive/$WHISPER_COMMIT.tar.gz"
WHISPER_SOURCE_SHA256=c8b0de473e9ec47a74bdf6104425c709261beeada8d6d7c1fec7432be701d032

FFMPEG_VERSION=8.1.2
FFMPEG_SOURCE_URL="https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
FFMPEG_SHA256=464beb5e7bf0c311e68b45ae2f04e9cc2af88851abb4082231742a74d97b524c

LIBASS_VERSION=0.17.5
LIBASS_SOURCE_URL="https://github.com/libass/libass/releases/download/$LIBASS_VERSION/libass-$LIBASS_VERSION.tar.xz"
LIBASS_SHA256=2dca25c0e0c837ddf00b52011b3f82cac1e4ddd3ad018227806b0c2288864acc

LIBUNIBREAK_VERSION=7.0
LIBUNIBREAK_SOURCE_URL="https://github.com/adah1972/libunibreak/releases/download/libunibreak_7_0/libunibreak-$LIBUNIBREAK_VERSION.tar.gz"
LIBUNIBREAK_SHA256=8c9a6e121736cd0d5c890ae3ae96f3f4010a19aa040f1dbded833a62a87717d3

FRIBIDI_VERSION=1.0.16
FRIBIDI_SOURCE_URL="https://github.com/fribidi/fribidi/releases/download/v$FRIBIDI_VERSION/fribidi-$FRIBIDI_VERSION.tar.xz"
FRIBIDI_SHA256=1b1cde5b235d40479e91be2f0e88a309e3214c8ab470ec8a2744d82a5a9ea05c

FREETYPE_VERSION=2.14.3
FREETYPE_SOURCE_URL="https://downloads.sourceforge.net/project/freetype/freetype2/$FREETYPE_VERSION/freetype-$FREETYPE_VERSION.tar.xz"
FREETYPE_SHA256=36bc4f1cc413335368ee656c42afca65c5a3987e8768cc28cf11ba775e785a5f

HARFBUZZ_VERSION=14.3.0
HARFBUZZ_SOURCE_URL="https://github.com/harfbuzz/harfbuzz/releases/download/$HARFBUZZ_VERSION/harfbuzz-$HARFBUZZ_VERSION.tar.xz"
HARFBUZZ_SHA256=16070d77cfc4ba1f1e7327e83bf9b3f55898081cabdb94e56a33e04fc8874eae
