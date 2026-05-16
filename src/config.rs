use std::path::PathBuf;

use crate::cli::Cli;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ffmpeg: PathBuf,
    pub whisper_cli: PathBuf,
    pub deepl_auth_key: Option<String>,
}

impl AppConfig {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            ffmpeg: cli.ffmpeg.clone(),
            whisper_cli: cli.whisper_cli.clone(),
            deepl_auth_key: cli.deepl_auth_key.clone(),
        }
    }
}
