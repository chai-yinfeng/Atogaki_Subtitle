# Bundled command-line components

Atogaki bundles separately executable command-line sidecars. They communicate with the application through command-line arguments, files, standard streams, and exit status.

## Version baseline

| Component | Version | License | Source |
| --- | --- | --- | --- |
| whisper.cpp / `whisper-cli` | v1.8.6 | MIT | https://github.com/ggml-org/whisper.cpp/releases/tag/v1.8.6 |
| FFmpeg / `ffmpeg` / `ffprobe` | 8.1.2 | LGPL v2.1+ build | https://ffmpeg.org/releases/ffmpeg-8.1.2.tar.xz |
| libass | 0.17.5 | ISC | https://github.com/libass/libass/releases/tag/0.17.5 |
| libunibreak | 7.0 | permissive; see bundled license | https://github.com/adah1972/libunibreak/releases/tag/libunibreak_7_0 |
| FriBidi | 1.0.16 | LGPL v2.1+ | https://github.com/fribidi/fribidi/releases/tag/v1.0.16 |
| FreeType | 2.14.3 | FreeType License | https://download.savannah.gnu.org/releases/freetype/ |
| HarfBuzz | 14.3.0 | permissive; see bundled license | https://github.com/harfbuzz/harfbuzz/releases/tag/14.3.0 |

The FFmpeg build script rejects GPL and nonfree configuration and verifies that `libx264` is absent. It builds libass and its LGPL/permissive dependencies from pinned, SHA-256-verified source archives and links them statically; it does not link Homebrew dylibs. `build-manifest.txt`, generated beside this file for release builds, records the exact dependency versions, target, configure line and binary hashes.

Before publishing an installer, distribute the complete corresponding FFmpeg and dependency source archives, license texts, build script and manifest from the same download location as the App. This repository documentation is a build aid, not legal advice.

This software uses the FreeType Project (https://www.freetype.org).
