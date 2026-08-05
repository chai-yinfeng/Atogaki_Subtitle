use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result};
use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::mpsc,
    time::{Duration, sleep},
};

use crate::{domain::render::RenderOptions, interface::cli::RecordArgs};

const VIDEOTOOLBOX_QUALITY: u8 = 65;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HardSubtitleEncoder {
    VideoToolbox,
    Libx264,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaCapabilities {
    pub binary_path: String,
    pub version: String,
    pub ass_filter: bool,
    pub videotoolbox_encoder: bool,
    pub libx264_encoder: bool,
    pub ready_for_hard_subtitles: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MediaProbe {
    pub duration_ms: u64,
    pub has_video: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderEncoder {
    VideoToolbox,
    Libx264,
    SubtitleMux,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderOutcome {
    pub encoder: RenderEncoder,
    pub hardware_accelerated: bool,
    pub audio_encoder: String,
    pub fallback_reason: Option<String>,
    pub output_path: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RenderProgress {
    pub progress: f64,
    pub out_time_ms: u64,
}

#[derive(Debug)]
pub struct RenderCancelled;

impl std::fmt::Display for RenderCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("video render cancelled")
    }
}

impl std::error::Error for RenderCancelled {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioEncoder {
    Copy,
    Aac,
}

#[derive(Clone)]
struct RenderExecution {
    duration_ms: u64,
    progress_sender: Option<mpsc::UnboundedSender<RenderProgress>>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy)]
struct EncodingSelection {
    video: HardSubtitleEncoder,
    audio: AudioEncoder,
}

pub async fn inspect_capabilities(ffmpeg: &Path) -> Result<MediaCapabilities> {
    let output = Command::new(ffmpeg)
        .arg("-version")
        .output()
        .await
        .with_context(|| format!("failed to start ffmpeg at {}", ffmpeg.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "ffmpeg capability check failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("unknown ffmpeg version")
        .to_string();
    let ass_filter = supports_filter(ffmpeg, "ass").await?;
    let videotoolbox_encoder = supports_encoder(ffmpeg, "h264_videotoolbox").await?;
    let libx264_encoder = supports_encoder(ffmpeg, "libx264").await?;

    Ok(MediaCapabilities {
        binary_path: ffmpeg.display().to_string(),
        version,
        ass_filter,
        videotoolbox_encoder,
        libx264_encoder,
        ready_for_hard_subtitles: ass_filter && (videotoolbox_encoder || libx264_encoder),
    })
}

pub async fn probe_media(ffmpeg: &Path, input: &Path) -> Result<MediaProbe> {
    if !input.is_file() {
        anyhow::bail!("input file does not exist: {}", input.display());
    }
    let ffprobe = ffmpeg.with_file_name("ffprobe");
    let duration_output = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input)
        .output()
        .await
        .with_context(|| format!("failed to start ffprobe at {}", ffprobe.display()))?;
    if !duration_output.status.success() {
        anyhow::bail!(
            "ffprobe failed for {}: {}",
            input.display(),
            String::from_utf8_lossy(&duration_output.stderr).trim()
        );
    }
    let duration_seconds = String::from_utf8_lossy(&duration_output.stdout)
        .trim()
        .parse::<f64>()
        .context("ffprobe returned an invalid media duration")?;
    let duration_ms = (duration_seconds.max(0.0) * 1_000.0).round() as u64;

    let video_output = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=index",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input)
        .output()
        .await
        .with_context(|| format!("failed to inspect video streams with {}", ffprobe.display()))?;
    if !video_output.status.success() {
        anyhow::bail!(
            "ffprobe video stream check failed for {}: {}",
            input.display(),
            String::from_utf8_lossy(&video_output.stderr).trim()
        );
    }

    Ok(MediaProbe {
        duration_ms,
        has_video: !String::from_utf8_lossy(&video_output.stdout)
            .trim()
            .is_empty(),
    })
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
) -> Result<RenderOutcome> {
    render_subtitles_with_progress(
        ffmpeg,
        input,
        ass,
        fallback_srt,
        output,
        options,
        0,
        None,
        Arc::new(AtomicBool::new(false)),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn render_subtitles_with_progress(
    ffmpeg: &Path,
    input: &Path,
    ass: &Path,
    fallback_srt: &Path,
    output: &Path,
    options: &RenderOptions,
    duration_ms: u64,
    progress_sender: Option<mpsc::UnboundedSender<RenderProgress>>,
    cancelled: Arc<AtomicBool>,
) -> Result<RenderOutcome> {
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

        mux_srt_soft_subtitle(ffmpeg, input, fallback_srt, output).await?;
        return Ok(RenderOutcome {
            encoder: RenderEncoder::SubtitleMux,
            hardware_accelerated: false,
            audio_encoder: "copy".to_string(),
            fallback_reason: None,
            output_path: output.display().to_string(),
        });
    }

    if !ass.exists() {
        anyhow::bail!("ASS subtitle file does not exist: {}", ass.display());
    }

    if !supports_filter(ffmpeg, "ass").await? {
        anyhow::bail!(
            "hard subtitle rendering requires ffmpeg with the libass/ass filter; select soft subtitles explicitly or configure ffmpeg-full"
        );
    }
    let execution = RenderExecution {
        duration_ms,
        progress_sender,
        cancelled,
    };
    burn_ass(ffmpeg, input, ass, output, options, &execution).await
}

async fn burn_ass(
    ffmpeg: &Path,
    input: &Path,
    ass: &Path,
    output: &Path,
    options: &RenderOptions,
    execution: &RenderExecution,
) -> Result<RenderOutcome> {
    let absolute_ass = ass.canonicalize().unwrap_or_else(|_| ass.to_path_buf());
    let filter = format!("ass=filename='{}'", escape_filter_path(&absolute_ass));
    burn_ass_with_encoder(ffmpeg, input, output, &filter, options, execution).await
}

async fn burn_ass_with_encoder(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    filter: &str,
    options: &RenderOptions,
    execution: &RenderExecution,
) -> Result<RenderOutcome> {
    let mut hardware_fallback_reason = None;
    if supports_encoder(ffmpeg, "h264_videotoolbox").await? {
        eprintln!("ffmpeg render: using VideoToolbox hardware H.264 encoder");
        let result = run_burn_command(
            ffmpeg,
            input,
            output,
            filter,
            options,
            EncodingSelection {
                video: HardSubtitleEncoder::VideoToolbox,
                audio: AudioEncoder::Copy,
            },
            execution,
        )
        .await;
        if result.is_ok() {
            return Ok(render_outcome(
                output,
                HardSubtitleEncoder::VideoToolbox,
                AudioEncoder::Copy,
                None,
            ));
        }
        let error = result.unwrap_err();
        if error.downcast_ref::<RenderCancelled>().is_some() {
            return Err(error);
        }
        let mut fallback_error = error.to_string();
        if audio_copy_failed(&error.to_string()) {
            let aac_result = run_burn_command(
                ffmpeg,
                input,
                output,
                filter,
                options,
                EncodingSelection {
                    video: HardSubtitleEncoder::VideoToolbox,
                    audio: AudioEncoder::Aac,
                },
                execution,
            )
            .await;
            if aac_result.is_ok() {
                return Ok(render_outcome(
                    output,
                    HardSubtitleEncoder::VideoToolbox,
                    AudioEncoder::Aac,
                    None,
                ));
            }
            let aac_error = aac_result.unwrap_err();
            if aac_error.downcast_ref::<RenderCancelled>().is_some() {
                return Err(aac_error);
            }
            fallback_error = aac_error.to_string();
        }
        eprintln!(
            "VideoToolbox render failed; retrying with libx264. Original failure: {}",
            fallback_error
        );
        hardware_fallback_reason = Some(format!(
            "VideoToolbox failed: {}",
            summarize_render_error(&fallback_error)
        ));
    }

    if !supports_encoder(ffmpeg, "libx264").await? {
        anyhow::bail!(
            "ffmpeg supports neither h264_videotoolbox nor libx264 for subtitle rendering"
        );
    }
    eprintln!("ffmpeg render: using libx264 software encoder");
    let result = run_burn_command(
        ffmpeg,
        input,
        output,
        filter,
        options,
        EncodingSelection {
            video: HardSubtitleEncoder::Libx264,
            audio: AudioEncoder::Copy,
        },
        execution,
    )
    .await;
    match result {
        Ok(()) => Ok(render_outcome(
            output,
            HardSubtitleEncoder::Libx264,
            AudioEncoder::Copy,
            hardware_fallback_reason.clone(),
        )),
        Err(error) if error.downcast_ref::<RenderCancelled>().is_some() => Err(error),
        Err(error) if audio_copy_failed(&error.to_string()) => {
            run_burn_command(
                ffmpeg,
                input,
                output,
                filter,
                options,
                EncodingSelection {
                    video: HardSubtitleEncoder::Libx264,
                    audio: AudioEncoder::Aac,
                },
                execution,
            )
            .await?;
            Ok(render_outcome(
                output,
                HardSubtitleEncoder::Libx264,
                AudioEncoder::Aac,
                hardware_fallback_reason,
            ))
        }
        Err(error) => Err(error),
    }
}

fn render_outcome(
    output: &Path,
    encoder: HardSubtitleEncoder,
    audio_encoder: AudioEncoder,
    fallback_reason: Option<String>,
) -> RenderOutcome {
    let (encoder, hardware_accelerated) = match encoder {
        HardSubtitleEncoder::VideoToolbox => (RenderEncoder::VideoToolbox, true),
        HardSubtitleEncoder::Libx264 => (RenderEncoder::Libx264, false),
    };
    RenderOutcome {
        encoder,
        hardware_accelerated,
        audio_encoder: match audio_encoder {
            AudioEncoder::Copy => "copy",
            AudioEncoder::Aac => "aac",
        }
        .to_string(),
        fallback_reason,
        output_path: output.display().to_string(),
    }
}

async fn run_burn_command(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    filter: &str,
    options: &RenderOptions,
    encoding: EncodingSelection,
    execution: &RenderExecution,
) -> Result<()> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(burn_args(
        input,
        output,
        filter,
        options,
        encoding.video,
        encoding.audio,
    ));
    run_render_command(
        cmd,
        "ffmpeg render",
        execution.duration_ms,
        execution.progress_sender.clone(),
        Arc::clone(&execution.cancelled),
    )
    .await
}

fn burn_args(
    input: &Path,
    output: &Path,
    filter: &str,
    options: &RenderOptions,
    encoder: HardSubtitleEncoder,
    audio_encoder: AudioEncoder,
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
    args.extend(["-c:a".into()]);
    match audio_encoder {
        AudioEncoder::Copy => args.push("copy".into()),
        AudioEncoder::Aac => args.extend(["aac".into(), "-b:a".into(), "192k".into()]),
    }
    args.extend([
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        output.as_os_str().to_os_string(),
    ]);
    args
}

fn audio_copy_failed(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("not currently supported in container")
        || message.contains("could not find tag for codec")
        || message.contains("codec not supported in container")
        || message.contains("incompatible with output codec")
}

fn summarize_render_error(message: &str) -> String {
    let preferred = [
        "Cannot create compression session",
        "Error while opening encoder",
        "Could not find tag for codec",
        "not currently supported in container",
    ];
    let line = preferred
        .iter()
        .find_map(|pattern| message.lines().find(|line| line.contains(pattern)))
        .or_else(|| message.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or("unknown ffmpeg error")
        .trim();
    line.chars().take(240).collect()
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

async fn run_render_command(
    mut cmd: Command,
    name: &str,
    duration_ms: u64,
    progress_sender: Option<mpsc::UnboundedSender<RenderProgress>>,
    cancelled: Arc<AtomicBool>,
) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(RenderCancelled.into());
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to start {name}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture {name} progress"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture {name} errors"))?;
    let stderr_task = tokio::spawn(async move {
        let mut output = String::new();
        stderr.read_to_string(&mut output).await.map(|_| output)
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut out_time_ms = 0;
    if let Some(sender) = &progress_sender {
        let _ = sender.send(RenderProgress {
            progress: 0.0,
            out_time_ms: 0,
        });
    }

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line.context("failed to read ffmpeg render progress")? else {
                    break;
                };
                if let Some(value) = line
                    .strip_prefix("out_time_us=")
                    .or_else(|| line.strip_prefix("out_time_ms="))
                    .and_then(|value| value.parse::<u64>().ok())
                {
                    out_time_ms = value / 1_000;
                    let progress = if duration_ms == 0 {
                        0.0
                    } else {
                        (out_time_ms as f64 / duration_ms as f64).clamp(0.0, 0.99)
                    };
                    if let Some(sender) = &progress_sender {
                        let _ = sender.send(RenderProgress { progress, out_time_ms });
                    }
                } else if line == "progress=end"
                    && let Some(sender) = &progress_sender
                {
                    let _ = sender.send(RenderProgress {
                        progress: 1.0,
                        out_time_ms: duration_ms.max(out_time_ms),
                    });
                }
            }
            _ = sleep(Duration::from_millis(150)) => {
                if cancelled.load(Ordering::Relaxed) {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = stderr_task.await;
                    return Err(RenderCancelled.into());
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .with_context(|| format!("failed to wait for {name}"))?;
    let stderr = stderr_task
        .await
        .context("failed to join ffmpeg stderr reader")?
        .context("failed to read ffmpeg stderr")?;
    if !status.success() {
        anyhow::bail!("{name} failed with {status}\nstderr:\n{stderr}");
    }
    Ok(())
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
    use super::{AudioEncoder, HardSubtitleEncoder, audio_copy_failed, burn_args};
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
            AudioEncoder::Copy,
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
        assert!(args.windows(2).any(|pair| pair == ["-progress", "pipe:1"]));
    }

    #[test]
    fn software_fallback_keeps_configured_crf_and_preset() {
        let args = string_args(HardSubtitleEncoder::Libx264);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
        assert!(args.windows(2).any(|pair| pair == ["-crf", "20"]));
        assert!(args.windows(2).any(|pair| pair == ["-preset", "medium"]));
    }

    #[test]
    fn detects_mp4_audio_copy_incompatibility() {
        assert!(audio_copy_failed(
            "Could not find tag for codec pcm_s16le in stream #1"
        ));
        assert!(!audio_copy_failed("VideoToolbox session failed"));
    }
}
