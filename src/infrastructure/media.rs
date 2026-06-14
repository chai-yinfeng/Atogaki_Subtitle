use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::process::Command;

use crate::interface::cli::RecordArgs;

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
) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("input file does not exist: {}", input.display());
    }
    if !ass.exists() {
        anyhow::bail!("ASS subtitle file does not exist: {}", ass.display());
    }

    if supports_filter(ffmpeg, "ass").await? {
        return burn_ass(ffmpeg, input, ass, output).await;
    }

    if !fallback_srt.exists() {
        anyhow::bail!(
            "ffmpeg does not support the ass filter, and fallback SRT is missing: {}",
            fallback_srt.display()
        );
    }

    eprintln!(
        "ffmpeg does not support libass/ass filter; muxing bilingual soft subtitles instead of burning them."
    );
    mux_srt_soft_subtitle(ffmpeg, input, fallback_srt, output).await
}

async fn burn_ass(ffmpeg: &Path, input: &Path, ass: &Path, output: &Path) -> Result<()> {
    let absolute_ass = ass.canonicalize().unwrap_or_else(|_| ass.to_path_buf());
    let filter = format!("ass=filename='{}'", escape_filter_path(&absolute_ass));
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-i"]);
    cmd.arg(input);
    cmd.args(["-vf", &filter, "-c:a", "copy"]);
    cmd.arg(output);

    run_checked(cmd, "ffmpeg render").await
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
