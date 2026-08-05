# Generated sidecars

This directory is populated by `scripts/build-sidecars-macos.sh`.

Tauri expects one target-suffixed file for each configured sidecar:

```text
ffmpeg-<target-triple>
ffprobe-<target-triple>
whisper-cli-<target-triple>
```

Generated binaries are intentionally ignored by Git. Release builds must run the sidecar build and verification script on the target platform before `tauri build`.
