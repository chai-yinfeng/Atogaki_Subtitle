use std::path::PathBuf;

use crate::interface::cli::WhisperArgs;

#[derive(Debug, Clone)]
pub struct TranscribeSpec {
    pub input: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub whisper: WhisperArgs,
}

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub input: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub render_output: Option<PathBuf>,
    pub deepl_auth_key: Option<String>,
    pub whisper: WhisperArgs,
}

#[derive(Debug, Clone)]
pub struct TranslateSpec {
    pub job_dir: PathBuf,
    pub deepl_auth_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExportSpec {
    pub job_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RenderSpec {
    pub input: PathBuf,
    pub job_dir: PathBuf,
    pub output: PathBuf,
}
