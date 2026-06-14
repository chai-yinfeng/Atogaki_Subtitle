use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

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
    Devices,
    Record(RecordArgs),
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

    #[arg(long, help = "Burn bilingual ASS subtitles into a video output")]
    pub render_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct WhisperArgs {
    #[arg(long, env = "ATOGAKI_WHISPER_MODEL")]
    pub model: PathBuf,

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
