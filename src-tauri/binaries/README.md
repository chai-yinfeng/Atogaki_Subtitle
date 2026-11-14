# Generated sidecars

`scripts/build-sidecars-macos.sh` populates the base macOS sidecars. The Windows
scripts are retained for historical recovery; Windows maintenance is paused.

Tauri expects one target-suffixed file for each configured sidecar:

```text
ffmpeg-<target-triple>
ffprobe-<target-triple>
whisper-cli-<target-triple>
```

Windows target files additionally end in `.exe`, for example
`ffmpeg-x86_64-pc-windows-msvc.exe`.

Generated binaries are intentionally ignored by Git. Release builds must run the sidecar build and verification script on the target platform before `tauri build`.

The development `tauri.macos-complete.conf.json` also requires:

```text
llama-server-aarch64-apple-darwin
atogaki-audio-tap-aarch64-apple-darwin
```

Build the pinned llama-server using the existing Hy-MT2 validation/build inputs
(`scripts/hy-mt2-versions.zsh`, `scripts/validate-hy-mt2.zsh`). The validation
script also downloads a model and runs inference; it is not a pure packaging step.
Build the capture helper from the repository root with:

```sh
./scripts/prepare-online-capture.sh src-tauri/binaries/atogaki-audio-tap-aarch64-apple-darwin
```

The base builder does not produce these two binaries. The installation script
prepares the capture helper before building. Its macOS 14.2 target does not
establish the minimum system version of the separate Python/MLX runtime, which
is not yet distributed as a complete new-machine setup.
