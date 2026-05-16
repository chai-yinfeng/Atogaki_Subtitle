# Atogaki Subtitle

Local-first offline audio/video transcription and translation workflow.

The current target is a Rust executable, not a macOS/Xcode app. The core pipeline is platform-neutral except for optional recording, which uses `ffmpeg` audio devices on the current machine.

## Requirements

- Rust toolchain
- `ffmpeg`
- `whisper-cli` from whisper.cpp
- A local Whisper model, for example `ggml-medium.bin`
- DeepL API key for translation

Useful environment variables:

```bash
export DEEPL_AUTH_KEY="your-key"
export ATOGAKI_FFMPEG="$(brew --prefix ffmpeg-full)/bin/ffmpeg"
export ATOGAKI_WHISPER_CLI="/opt/homebrew/bin/whisper-cli"
export ATOGAKI_WHISPER_MODEL="/Users/black_magic/Models/whisper/ggml-medium.bin"
export ATOGAKI_VAD_MODEL="/Users/black_magic/Models/whisper/ggml-silero-v6.2.0.bin"
```

## Quick Start

```bash
cargo run -- process input.mp4 \
  --model /path/to/ggml-medium.bin \
  --deepl-auth-key "$DEEPL_AUTH_KEY"
```

If `ATOGAKI_WHISPER_MODEL` and `DEEPL_AUTH_KEY` are set:

```bash
cargo run -- process input.mp4
```

For tighter timestamping, pass a whisper.cpp VAD model:

```bash
cargo run -- process input.mp4 \
  --model /path/to/ggml-medium.bin \
  --vad-model /path/to/ggml-silero-v5.1.2.bin
```

If whisper.cpp crashes in the GPU backend, retry with `--no-gpu`.

Outputs are written to `./atogaki_jobs/<timestamp>/` by default:

- `audio.wav`
- `segments.json`
- `ja.srt`
- `zh.srt`
- `bilingual.srt`
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

Process and render subtitles into a video. If your `ffmpeg` has libass, this burns styled ASS subtitles. Otherwise it muxes bilingual SRT as a soft subtitle track.

```bash
cargo run -- process input.mp4 \
  --model /path/to/model.bin \
  --render-output output.mp4
```

Use `DEEPL_AUTH_KEY` or pass `--deepl-auth-key`.
