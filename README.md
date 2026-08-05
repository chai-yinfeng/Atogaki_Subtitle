# Atogaki Subtitle

Local-first Japanese audio/video transcription and translation workspace.

The repository contains a reusable Rust processing core, a CLI, an experimental Web/Postgres shell, and a Tauri desktop MVP backed by local SQLite. Product direction and current milestones live in `docs/product-direction.md` and `docs/roadmap.md`.

## Architecture

The code is organized into four layers:

- `src/interface`: CLI argument parsing and command dispatch.
- `src/application`: job specs, job status, and `JobRunner` workflow orchestration.
- `src/domain`: transcript segments, glossary handling, segmentation, and subtitle formatting.
- `src/infrastructure`: filesystem job storage, ffmpeg, whisper-cli, DeepL, and runtime config.

The CLI and future Web API should call the application layer instead of invoking ffmpeg, Whisper, or DeepL directly.

## Requirements

- Rust toolchain
- CLI 开发需要可用的 `ffmpeg` 与 whisper.cpp `whisper-cli`
- 打包桌面 App 先运行 `./scripts/build-sidecars-macos.sh`，生成内置的 LGPL FFmpeg/ffprobe 与 whisper-cli sidecar
- A local Whisper model, for example `ggml-medium.bin`
- DeepL API key for translation

Useful environment variables:

```bash
export DEEPL_AUTH_KEY="your-key"
export ATOGAKI_FFMPEG="/path/to/ffmpeg"
export ATOGAKI_WHISPER_CLI="/opt/homebrew/bin/whisper-cli"
export ATOGAKI_WHISPER_MODEL="/Users/black_magic/Models/whisper/ggml-medium.bin"
export ATOGAKI_VAD_MODEL="/Users/black_magic/Models/whisper/ggml-silero-v6.2.0.bin"
export ATOGAKI_GLOSSARY="/Users/black_magic/Desktop/Coding_projects/Atogaki_Sub/assets/glossaries/yorushika.txt"
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

Whisper tries the GPU/Metal backend by default and automatically retries once with `--no-gpu` when the failure looks GPU-related. Pass `--no-gpu` to force CPU mode from the start.

The packaged desktop application uses `ffmpeg`, `ffprobe`, and `whisper-cli` from its App Bundle. `ATOGAKI_FFMPEG`, `ATOGAKI_FFPROBE`, and `ATOGAKI_WHISPER_CLI` remain explicit development overrides. Hard-subtitle rendering tries real `h264_videotoolbox`, falls back visibly to FFmpeg's native LGPL `mpeg4` encoder, and fails only if both layers fail. The distributed FFmpeg does not contain libx264.

Glossary files can be passed with `--glossary` or `ATOGAKI_GLOSSARY`. Plain lines are fed into Whisper's initial prompt as likely proper nouns. Lines in `wrong => correct` form are also applied as conservative text replacements after ASR.
For a canonical spelling whose Japanese reading differs, use the reading on the left, for example
`スイ => suis`: Whisper is prompted with both forms and the post-ASR pass normalizes the result to
`suis`. This is an ASR glossary and does not configure DeepL translation terminology.

Outputs are written to `./atogaki_jobs/<timestamp>/` by default:

- `status.json`
- `audio.wav`
- `segments.json`
- `ja.srt`
- `zh.srt`
- `bilingual.srt`
- `bilingual.ass`

`status.json` records the durable job state for CLI progress and future Web polling.

## Commands

Build and check the desktop MVP:

```bash
npm --prefix ui install
npm --prefix ui run build
./scripts/build-sidecars-macos.sh
cargo check --manifest-path src-tauri/Cargo.toml
```

After the frontend build, launch the desktop application directly:

```bash
export DEEPL_AUTH_KEY="your-deepl-api-key" # optional; enables Japanese -> Simplified Chinese
cargo run --manifest-path src-tauri/Cargo.toml
```

The desktop home screen can submit transcription jobs. Select a completed task to open the
playback workspace, follow the highlighted subtitle, click a timecode to seek, and save Japanese
or Chinese edits to SQLite. With DeepL configured, the workspace can translate one segment or
atomically retranslate all segments, then export Japanese, Chinese, and bilingual SRT/ASS files
from the current SQLite state. Recognition glossaries can be created and edited in the desktop,
selected for a new Whisper task, previewed against an existing SQLite workspace, and extended from
a manual subtitle correction. Each selected glossary is snapshotted inside its task directory for
reproducibility. See `docs/desktop-testing.md` for the manual smoke-test checklist and current codec
limitations.

Desktop tasks may be given a SQLite-backed display name without renaming their durable UUID
directory. Finished and failed tasks can be deleted from the task list; deletion removes only the
managed task directory and SQLite workspace, never the original imported media file.

Start the Web API shell:

```bash
cargo run -- serve --bind 127.0.0.1:8080
```

With Postgres configured, the server connects and runs migrations at startup:

```bash
export DATABASE_URL="postgres://$(whoami)@localhost:5432/atogaki_dev"
cargo run -- serve --bind 127.0.0.1:8080
```

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

Process and render subtitles into a video. With the bundled FFmpeg this burns styled ASS subtitles using VideoToolbox when available and native MPEG-4 as the software fallback. Otherwise, select soft subtitles explicitly to mux bilingual SRT.

```bash
cargo run -- process input.mp4 \
  --model /path/to/model.bin \
  --render-output output.mp4
```

Hard-subtitle rendering must re-encode the video because subtitles become pixels in the video stream. Legacy `--video-crf` and `--video-preset` options remain accepted for CLI compatibility but do not control the fixed-quality LGPL MPEG-4 fallback.

```bash
cargo run -- render input.mp4 atogaki_jobs/job-... \
  --output output.mp4 \
  --video-crf 18 \
  --video-preset medium
```

To preserve the original video stream, mux bilingual SRT as a soft subtitle track instead:

```bash
cargo run -- render input.mp4 atogaki_jobs/job-... \
  --output output-soft.mp4 \
  --soft-subtitles
```

After changing glossary rules, apply them to an existing job without rerunning Whisper. Changed Japanese lines have their stale Chinese translations cleared by default, so run `translate` again before final export/render.

```bash
cargo run -- apply-glossary atogaki_jobs/job-...
cargo run -- translate atogaki_jobs/job-...
cargo run -- export atogaki_jobs/job-...
```

If a job already has `input` and `render_output` in `status.json`, rerender it without repeating those paths:

```bash
cargo run -- rerender atogaki_jobs/job-...
```

Use `DEEPL_AUTH_KEY` or pass `--deepl-auth-key`.

## License

Atogaki's own source code, documentation, and build configuration are licensed under the
[Apache License 2.0](LICENSE). Bundled sidecars, models, libraries, and other third-party material
remain subject to their respective licenses; see `src-tauri/third-party/README.md`.
