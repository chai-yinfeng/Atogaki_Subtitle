use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tokio::process::Command;

use crate::{domain::render::RenderOptions, interface::cli::RecordArgs};

const VIDEOTOOLBOX_QUALITY: u8 = 65;

#[derive(Debug, Clone, Copy)]
enum HardSubtitleEncoder {
    VideoToolbox,
    Libx264,
}

pub async fn list_capture_devices(ffmpeg: &Path) -> Result<()> {
    let output = Command::new(ffmpeg)
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .await
        .context("failed to run ffmpeg device listing")?;

    let text = String::from_utf8_lossy(&output.stderr);
    println!("{text}");
    Ok(())
}

pub async fn record_audio(ffmpeg: &Path, args: &RecordArgs) -> Result<()> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-f", "avfoundation"]);
    if let Some(seconds) = args.duration {
        cmd.args(["-t", &seconds.to_string()]);
    }
    cmd.args([
        "-i",
        &args.device,
        "-vn",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-c:a",
        "pcm_s16le",
    ]);
    cmd.arg(&args.output);

    run_checked(cmd, "ffmpeg record").await
}

pub async fn extract_wav(ffmpeg: &Path, input: &Path, output: &Path) -> Result<PathBuf> {
    if !input.exists() {
        anyhow::bail!("input file does not exist: {}", input.display());
    }

    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-i"]);
    cmd.arg(input);
    cmd.args(["-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"]);
    cmd.arg(output);

    run_checked(cmd, "ffmpeg extract wav").await?;
    Ok(output.to_path_buf())
}

pub async fn render_subtitles(
    ffmpeg: &Path,
    input: &Path,
    ass: &Path,
    fallback_srt: &Path,
    output: &Path,
    options: &RenderOptions,
) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("input file does not exist: {}", input.display());
    }

    validate_render_options(options)?;

    if options.soft_subtitles {
        if !fallback_srt.exists() {
            anyhow::bail!(
                "soft subtitle file does not exist: {}",
                fallback_srt.display()
            );
        }

        return mux_srt_soft_subtitle(ffmpeg, input, fallback_srt, output).await;
    }

    if !ass.exists() {
        anyhow::bail!("ASS subtitle file does not exist: {}", ass.display());
    }

    if !supports_filter(ffmpeg, "ass").await? {
        anyhow::bail!(
            "hard subtitle rendering requires ffmpeg with the libass/ass filter; select soft subtitles explicitly or configure ffmpeg-full"
        );
    }
    burn_ass(ffmpeg, input, ass, output, options).await
}

async fn burn_ass(
    ffmpeg: &Path,
    input: &Path,
    ass: &Path,
    output: &Path,
    options: &RenderOptions,
) -> Result<()> {
    let absolute_ass = ass.canonicalize().unwrap_or_else(|_| ass.to_path_buf());
    let filter = format!("ass=filename='{}'", escape_filter_path(&absolute_ass));
    burn_ass_with_encoder(ffmpeg, input, output, &filter, options).await
}

async fn burn_ass_with_encoder(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    filter: &str,
    options: &RenderOptions,
) -> Result<()> {
    if supports_encoder(ffmpeg, "h264_videotoolbox").await? {
        eprintln!("ffmpeg render: using VideoToolbox hardware H.264 encoder");
        let result = run_burn_command(
            ffmpeg,
            input,
            output,
            filter,
            options,
            HardSubtitleEncoder::VideoToolbox,
        )
        .await;
        if result.is_ok() {
            return result;
        }
        eprintln!(
            "VideoToolbox render failed; retrying with libx264. Original failure: {}",
            result.unwrap_err()
        );
    }

    if !supports_encoder(ffmpeg, "libx264").await? {
        anyhow::bail!(
            "ffmpeg supports neither h264_videotoolbox nor libx264 for subtitle rendering"
        );
    }
    eprintln!("ffmpeg render: using libx264 software encoder");
    run_burn_command(
        ffmpeg,
        input,
        output,
        filter,
        options,
        HardSubtitleEncoder::Libx264,
    )
    .await
}

async fn run_burn_command(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    filter: &str,
    options: &RenderOptions,
    encoder: HardSubtitleEncoder,
) -> Result<()> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(burn_args(input, output, filter, options, encoder));
    run_checked(cmd, "ffmpeg render").await
}

fn burn_args(
    input: &Path,
    output: &Path,
    filter: &str,
    options: &RenderOptions,
    encoder: HardSubtitleEncoder,
) -> Vec<OsString> {
    let mut args = vec![
        "-y".into(),
        "-i".into(),
        input.as_os_str().to_os_string(),
        "-vf".into(),
        filter.into(),
    ];
    match encoder {
        HardSubtitleEncoder::VideoToolbox => args.extend([
            "-c:v".into(),
            "h264_videotoolbox".into(),
            "-q:v".into(),
            VIDEOTOOLBOX_QUALITY.to_string().into(),
            "-profile:v".into(),
            "high".into(),
            "-allow_sw".into(),
            "0".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]),
        HardSubtitleEncoder::Libx264 => args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            options.video_preset.clone().into(),
            "-crf".into(),
            options.video_crf.to_string().into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]),
    }
    args.extend([
        "-c:a".into(),
        "copy".into(),
        output.as_os_str().to_os_string(),
    ]);
    args
}

async fn mux_srt_soft_subtitle(
    ffmpeg: &Path,
    input: &Path,
    srt: &Path,
    output: &Path,
) -> Result<()> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-i"]);
    cmd.arg(input);
    cmd.args(["-i"]);
    cmd.arg(srt);
    cmd.args([
        "-map",
        "0",
        "-map",
        "1:0",
        "-c:v",
        "copy",
        "-c:a",
        "copy",
        "-c:s",
        "mov_text",
        "-metadata:s:s:0",
        "language=chi",
    ]);
    cmd.arg(output);

    run_checked(cmd, "ffmpeg mux subtitles").await
}

fn validate_render_options(options: &RenderOptions) -> Result<()> {
    if options.video_crf > 51 {
        anyhow::bail!("--video-crf must be between 0 and 51");
    }

    Ok(())
}

async fn supports_filter(ffmpeg: &Path, filter: &str) -> Result<bool> {
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-filters"])
        .output()
        .await
        .context("failed to query ffmpeg filters")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .any(|line| line.split_whitespace().any(|part| part == filter)))
}

async fn supports_encoder(ffmpeg: &Path, encoder: &str) -> Result<bool> {
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .await
        .context("failed to query ffmpeg encoders")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .any(|line| line.split_whitespace().any(|part| part == encoder)))
}

async fn run_checked(mut cmd: Command, name: &str) -> Result<()> {
    let output = cmd
        .output()
        .await
        .with_context(|| format!("failed to start {name}"))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "{name} failed with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            output.status
        );
    }

    Ok(())
}

fn escape_filter_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::{HardSubtitleEncoder, burn_args};
    use crate::domain::render::RenderOptions;
    use std::path::Path;

    fn options() -> RenderOptions {
        RenderOptions {
            video_crf: 20,
            video_preset: "medium".to_string(),
            soft_subtitles: false,
        }
    }

    fn string_args(encoder: HardSubtitleEncoder) -> Vec<String> {
        burn_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            "ass=subtitle.ass",
            &options(),
            encoder,
        )
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
    }

    #[test]
    fn videotoolbox_render_requires_real_hardware_encoding() {
        let args = string_args(HardSubtitleEncoder::VideoToolbox);

        assert!(
            args.windows(2)
                .any(|pair| pair == ["-c:v", "h264_videotoolbox"])
        );
        assert!(args.windows(2).any(|pair| pair == ["-allow_sw", "0"]));
        assert!(!args.iter().any(|argument| argument == "libx264"));
    }

    #[test]
    fn software_fallback_keeps_configured_crf_and_preset() {
        let args = string_args(HardSubtitleEncoder::Libx264);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
        assert!(args.windows(2).any(|pair| pair == ["-crf", "20"]));
        assert!(args.windows(2).any(|pair| pair == ["-preset", "medium"]));
    }
}
