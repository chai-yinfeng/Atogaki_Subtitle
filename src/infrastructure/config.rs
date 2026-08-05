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
/// Explicit environment overrides remain available for development. Packaged
/// applications prefer the signed sidecar next to the desktop executable, so
/// Finder launches do not depend on Homebrew or a shell PATH.
pub fn desktop_ffmpeg_path() -> PathBuf {
    if let Some(configured) = env::var_os("ATOGAKI_FFMPEG").filter(|value| !value.is_empty()) {
        return PathBuf::from(configured);
    }

    if let Some(sidecar) = sibling_executable(platform_executable_name("ffmpeg")) {
        return sidecar;
    }

    desktop_ffmpeg_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

pub fn desktop_whisper_cli_path() -> PathBuf {
    if let Some(configured) = env::var_os("ATOGAKI_WHISPER_CLI").filter(|value| !value.is_empty()) {
        return PathBuf::from(configured);
    }

    sibling_executable(platform_executable_name("whisper-cli"))
        .unwrap_or_else(|| PathBuf::from("whisper-cli"))
}

fn sibling_executable(file_name: impl AsRef<std::path::Path>) -> Option<PathBuf> {
    let executable = env::current_exe().ok()?;
    executable
        .parent()
        .map(|directory| directory.join(file_name))
        .filter(|path| path.is_file())
}

fn platform_executable_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
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
    use super::{desktop_ffmpeg_candidates, platform_executable_name};

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

    #[test]
    fn sidecar_names_follow_the_target_platform() {
        let name = platform_executable_name("whisper-cli");
        if cfg!(target_os = "windows") {
            assert_eq!(name, "whisper-cli.exe");
        } else {
            assert_eq!(name, "whisper-cli");
        }
    }
}
