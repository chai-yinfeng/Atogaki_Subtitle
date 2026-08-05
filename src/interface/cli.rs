use std::{net::SocketAddr, path::PathBuf};

use clap::{ArgAction, Args, Parser, Subcommand};

use crate::application::TranscriptionOptions;

#[derive(Debug, Parser)]
#[command(name = "atogaki")]
#[command(about = "Offline audio/video transcription and subtitle translation")]
pub struct Cli {
    #[arg(long, env = "ATOGAKI_FFMPEG", default_value = "ffmpeg")]
    pub ffmpeg: PathBuf,

    #[arg(long, env = "ATOGAKI_WHISPER_CLI", default_value = "whisper-cli")]
    pub whisper_cli: PathBuf,

    #[arg(long, env = "DEEPL_AUTH_KEY", hide_env_values = true)]
    pub deepl_auth_key: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    ApplyGlossary(ApplyGlossaryArgs),
    Devices,
    Record(RecordArgs),
    Rerender(RerenderArgs),
    Serve(ServeArgs),
    Transcribe(TranscribeArgs),
    Translate(TranslateArgs),
    Export(ExportArgs),
    Render(RenderArgs),
    Process(ProcessArgs),
}

#[derive(Debug, Args)]
pub struct RecordArgs {
    #[arg(
        long,
        help = "ffmpeg avfoundation input such as ':0' or ':BlackHole 2ch'"
    )]
    pub device: String,

    #[arg(long)]
    pub output: PathBuf,

    #[arg(
        long,
        help = "Recording duration in seconds. Omit to record until Ctrl-C."
    )]
    pub duration: Option<u64>,
}

#[derive(Debug, Args)]
pub struct TranscribeArgs {
    pub input: PathBuf,

    #[command(flatten)]
    pub whisper: WhisperArgs,

    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct TranslateArgs {
    pub job_dir: PathBuf,

    #[arg(long, env = "DEEPL_AUTH_KEY", hide_env_values = true)]
    pub deepl_auth_key: Option<String>,

    #[arg(
        long,
        env = "ATOGAKI_TRANSLATION_SOURCE_LANGUAGE",
        default_value = "ja"
    )]
    pub source_language: String,

    #[arg(
        long,
        env = "ATOGAKI_TRANSLATION_TARGET_LANGUAGE",
        default_value = "zh"
    )]
    pub target_language: String,
}

#[derive(Debug, Args)]
pub struct ApplyGlossaryArgs {
    pub job_dir: PathBuf,

    #[arg(long, env = "ATOGAKI_GLOSSARY")]
    pub glossary: PathBuf,

    #[arg(
        long,
        help = "Keep existing translations even when the Japanese source text changes"
    )]
    pub keep_translations: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    pub job_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct RenderArgs {
    pub input: PathBuf,

    pub job_dir: PathBuf,

    #[arg(long)]
    pub output: PathBuf,

    #[command(flatten)]
    pub render: RenderArgsCommon,
}

#[derive(Debug, Args)]
pub struct RerenderArgs {
    pub job_dir: PathBuf,

    #[arg(long, help = "Override the input media path saved in status.json")]
    pub input: Option<PathBuf>,

    #[arg(long, help = "Override the render output path saved in status.json")]
    pub output: Option<PathBuf>,

    #[command(flatten)]
    pub render: RenderArgsCommon,
}

#[derive(Debug, Clone, Args)]
pub struct ServeArgs {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,

    #[arg(long, env = "ATOGAKI_BIND", default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,

    #[arg(long, env = "ATOGAKI_JWT_SECRET", default_value = "dev-only-change-me")]
    pub jwt_secret: String,

    #[arg(long, env = "ATOGAKI_JOBS_DIR", default_value = "atogaki_jobs")]
    pub jobs_dir: PathBuf,

    #[arg(long, env = "ATOGAKI_UPLOADS_DIR", default_value = "atogaki_uploads")]
    pub uploads_dir: PathBuf,

    #[arg(long, env = "ATOGAKI_WHISPER_MODEL")]
    pub whisper_model: Option<PathBuf>,

    #[arg(long, env = "ATOGAKI_VAD_MODEL")]
    pub vad_model: Option<PathBuf>,

    #[arg(long, env = "ATOGAKI_WEB_WORKERS", default_value_t = 1)]
    pub workers: usize,
}

#[derive(Debug, Args)]
pub struct ProcessArgs {
    pub input: PathBuf,

    #[command(flatten)]
    pub whisper: WhisperArgs,

    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    #[arg(long, env = "DEEPL_AUTH_KEY", hide_env_values = true)]
    pub deepl_auth_key: Option<String>,

    #[arg(
        long,
        env = "ATOGAKI_TRANSLATION_TARGET_LANGUAGE",
        default_value = "zh",
        help = "DeepL target language; source language follows --source-language"
    )]
    pub target_language: String,

    #[arg(long, help = "Burn bilingual ASS subtitles into a video output")]
    pub render_output: Option<PathBuf>,

    #[command(flatten)]
    pub render: RenderArgsCommon,
}

#[derive(Debug, Clone, Args)]
pub struct RenderArgsCommon {
    #[arg(
        long,
        env = "ATOGAKI_RENDER_CRF",
        default_value_t = 20,
        help = "Legacy compatibility value; the LGPL MPEG-4 fallback currently uses a fixed quality"
    )]
    pub video_crf: u8,

    #[arg(
        long,
        env = "ATOGAKI_RENDER_PRESET",
        default_value = "medium",
        help = "Legacy compatibility value; the LGPL MPEG-4 fallback does not use x264 presets"
    )]
    pub video_preset: String,

    #[arg(
        long,
        help = "Mux bilingual SRT as a soft subtitle track without re-encoding video"
    )]
    pub soft_subtitles: bool,
}

#[derive(Debug, Clone, Args)]
pub struct WhisperArgs {
    #[arg(long, env = "ATOGAKI_WHISPER_MODEL")]
    pub model: PathBuf,

    #[arg(long, env = "ATOGAKI_SOURCE_LANGUAGE", default_value = "ja")]
    pub source_language: String,

    #[arg(long, env = "ATOGAKI_GLOSSARY")]
    pub glossary: Option<PathBuf>,

    #[arg(long, env = "ATOGAKI_WHISPER_PROMPT")]
    pub prompt: Option<String>,

    #[arg(long, env = "ATOGAKI_VAD_MODEL")]
    pub vad_model: Option<PathBuf>,

    #[arg(long, default_value_t = 0.50)]
    pub vad_threshold: f32,

    #[arg(long, default_value_t = 250)]
    pub vad_min_speech_ms: u64,

    #[arg(long, default_value_t = 450)]
    pub vad_min_silence_ms: u64,

    #[arg(long, default_value_t = 8)]
    pub vad_max_speech_s: u64,

    #[arg(long, default_value_t = 120)]
    pub vad_speech_pad_ms: u64,

    #[arg(
        long,
        default_value_t = 32,
        help = "Whisper max segment length in characters; 0 disables it"
    )]
    pub max_len: u32,

    #[arg(long = "no-split-on-word", action = ArgAction::SetFalse, default_value_t = true)]
    pub split_on_word: bool,

    #[arg(long, default_value_t = 0.30)]
    pub no_speech_threshold: f32,

    #[arg(long, action = ArgAction::SetTrue)]
    pub output_json_full: bool,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Force Whisper CPU mode. By default GPU is tried first and CPU is retried on GPU failure."
    )]
    pub no_gpu: bool,
}

impl From<WhisperArgs> for TranscriptionOptions {
    fn from(args: WhisperArgs) -> Self {
        Self {
            model: args.model,
            source_language: args.source_language,
            glossary: args.glossary,
            prompt: args.prompt,
            vad_model: args.vad_model,
            vad_threshold: args.vad_threshold,
            vad_min_speech_ms: args.vad_min_speech_ms,
            vad_min_silence_ms: args.vad_min_silence_ms,
            vad_max_speech_s: args.vad_max_speech_s,
            vad_speech_pad_ms: args.vad_speech_pad_ms,
            max_len: args.max_len,
            split_on_word: args.split_on_word,
            no_speech_threshold: args.no_speech_threshold,
            output_json_full: args.output_json_full,
            no_gpu: args.no_gpu,
        }
    }
}
