use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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

    #[arg(long)]
    pub model: PathBuf,

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

    #[arg(long)]
    pub model: PathBuf,

    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    #[arg(long, env = "DEEPL_AUTH_KEY", hide_env_values = true)]
    pub deepl_auth_key: Option<String>,

    #[arg(long, help = "Burn bilingual ASS subtitles into a video output")]
    pub render_output: Option<PathBuf>,
}
