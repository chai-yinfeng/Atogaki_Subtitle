use std::{env, path::PathBuf};

use crate::interface::cli::Cli;

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

/// Resolve the ffmpeg binary used by the desktop application.
///
/// GUI applications launched outside a shell do not reliably inherit the
/// user's PATH. Keep the environment override authoritative, then prefer the
/// Homebrew ffmpeg-full locations that provide libass and VideoToolbox before
/// falling back to normal command lookup.
pub fn desktop_ffmpeg_path() -> PathBuf {
    if let Some(configured) = env::var_os("ATOGAKI_FFMPEG").filter(|value| !value.is_empty()) {
        return PathBuf::from(configured);
    }

    desktop_ffmpeg_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

fn desktop_ffmpeg_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/opt/homebrew/opt/ffmpeg-full/bin/ffmpeg"),
            PathBuf::from("/usr/local/opt/ffmpeg-full/bin/ffmpeg"),
        ]
    }

    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::desktop_ffmpeg_candidates;

    #[test]
    #[cfg(target_os = "macos")]
    fn desktop_prefers_homebrew_ffmpeg_full_locations() {
        let candidates = desktop_ffmpeg_candidates();

        assert_eq!(
            candidates[0].to_string_lossy(),
            "/opt/homebrew/opt/ffmpeg-full/bin/ffmpeg"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.to_string_lossy().contains("ffmpeg-full"))
        );
    }
}
