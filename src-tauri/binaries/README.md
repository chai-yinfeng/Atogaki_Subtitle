# Generated sidecars

This directory is populated by `scripts/build-sidecars-macos.sh` or the paired
Windows scripts `build-whisper-windows.ps1` and `build-ffmpeg-windows.sh`.

Tauri expects one target-suffixed file for each configured sidecar:

```text
ffmpeg-<target-triple>
ffprobe-<target-triple>
whisper-cli-<target-triple>
```

Windows target files additionally end in `.exe`, for example
`ffmpeg-x86_64-pc-windows-msvc.exe`.

Generated binaries are intentionally ignored by Git. Release builds must run the sidecar build and verification script on the target platform before `tauri build`.
