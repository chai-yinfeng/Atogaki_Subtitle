# Third-party distribution materials

This directory is copied into the App Bundle. It contains generated Rust and frontend dependency notices, the exact sidecar binary build manifest, and sidecar license texts. Atogaki's own Apache-2.0 license is bundled separately at the App resource root.

## Generated application notices

- `rust-licenses.html` is generated from `src-tauri/Cargo.lock` for `aarch64-apple-darwin` with `cargo-about 0.9.1`.
- `frontend-licenses.html` is generated from `ui/package-lock.json`; it distinguishes runtime packages copied into the WebView bundle from build-only packages.

Regenerate and review both files whenever either lockfile changes:

```bash
cargo install --locked --features cli --version 0.9.1 cargo-about
./scripts/generate-rust-licenses.sh
npm --prefix ui ci
node ./scripts/generate-frontend-licenses.mjs
```

The generators fail when they encounter an unreviewed license expression or a missing required license text. The reviewed scope and limitations are recorded in `docs/third-party-license-audit.md`.

## Bundled command-line components

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

The FFmpeg build script rejects GPL and nonfree configuration and verifies that `libx264` is absent. It builds libass and its LGPL/permissive dependencies from pinned, SHA-256-verified source archives and links them statically; it does not link Homebrew dylibs. `build-manifest.txt`, generated beside this file for release builds, records the exact dependency versions, source hashes, target, configure line and pre-bundle binary hashes. Tauri's final ad-hoc signature changes the Mach-O file hashes inside the App; release integrity is checked with `codesign` and the DMG SHA-256 instead of comparing signed sidecars to the pre-bundle hashes.

Before publishing an installer, generate the complete corresponding source archive:

```bash
./scripts/package-sidecar-sources-macos.sh
```

It writes `Atogaki-<version>-third-party-sources.tar.xz` and a SHA-256 file under `src-tauri/target/release/bundle/sources/`. Upload both beside the DMG in the same GitHub Release. The archive includes the exact FFmpeg, libass, libunibreak, FriBidi, FreeType, HarfBuzz and whisper.cpp upstream sources, their checksums, license texts, the pinned build script and the matching binary manifest. The archive is a Release asset: it is intentionally ignored by Git and not embedded in the DMG.

This repository documentation is a build aid, not legal advice.

This software uses the FreeType Project (https://www.freetype.org).
