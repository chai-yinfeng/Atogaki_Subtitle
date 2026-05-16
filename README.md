# Atogaki Subtitle

Local-first offline audio/video transcription and translation workflow.

The current target is a Rust executable, not a macOS/Xcode app. The core pipeline is platform-neutral except for optional recording, which uses `ffmpeg` audio devices on the current machine.

## Requirements

- Rust toolchain
- `ffmpeg`
- `whisper-cli` from whisper.cpp
- A local Whisper model, for example `ggml-medium.bin`
- DeepL API key for translation

## Quick Start

```bash
cargo run -- process input.mp4 \
  --model /path/to/ggml-medium.bin \
  --deepl-auth-key "$DEEPL_AUTH_KEY"
```

Outputs are written to `./atogaki_jobs/<timestamp>/` by default:

- `audio.wav`
- `segments.json`
- `ja.srt`
- `zh.srt`
- `bilingual.ass`

## Commands

List macOS/ffmpeg capture devices:

```bash
cargo run -- devices
```

Record audio through ffmpeg:

```bash
cargo run -- record --device ":0" --duration 300 --output capture.wav
```

Process a media file end-to-end:

```bash
cargo run -- process input.mp4 --model /path/to/model.bin
```

Process and burn subtitles into a video:

```bash
cargo run -- process input.mp4 \
  --model /path/to/model.bin \
  --render-output output.mp4
```

Use `DEEPL_AUTH_KEY` or pass `--deepl-auth-key`.
