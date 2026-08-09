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
const MPEG4_QUALITY: u8 = 3;
const SOURCE_BITRATE_HEADROOM_NUMERATOR: u64 = 6;
const SOURCE_BITRATE_HEADROOM_DENOMINATOR: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HardSubtitleEncoder {
    VideoToolbox,
    Mpeg4,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaCapabilities {
    pub binary_path: String,
    pub version: String,
    pub ass_filter: bool,
    pub videotoolbox_encoder: bool,
    pub mpeg4_encoder: bool,
    pub ready_for_hard_subtitles: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MediaProbe {
    pub duration_ms: u64,
    pub has_video: bool,
    pub video_bitrate_bps: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderEncoder {
    VideoToolbox,
    Mpeg4,
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
    let mpeg4_encoder = supports_encoder(ffmpeg, "mpeg4").await?;

    Ok(MediaCapabilities {
        binary_path: ffmpeg.display().to_string(),
        version,
        ass_filter,
        videotoolbox_encoder,
        mpeg4_encoder,
        ready_for_hard_subtitles: ass_filter && (videotoolbox_encoder || mpeg4_encoder),
    })
}

pub async fn probe_media(ffmpeg: &Path, input: &Path) -> Result<MediaProbe> {
    if !input.is_file() {
        anyhow::bail!("input file does not exist: {}", input.display());
    }
    let ffprobe = ffprobe_path(ffmpeg);
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
            "stream=index,bit_rate",
            "-of",
            "default=noprint_wrappers=1",
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

    let video_fields = String::from_utf8_lossy(&video_output.stdout);
    let has_video = video_fields.lines().any(|line| line.starts_with("index="));
    let video_bitrate_bps = video_fields
        .lines()
        .find_map(|line| line.strip_prefix("bit_rate="))
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0);

    Ok(MediaProbe {
        duration_ms,
        has_video,
        video_bitrate_bps,
    })
}

fn ffprobe_path(ffmpeg: &Path) -> PathBuf {
    if let Some(configured) = std::env::var_os("ATOGAKI_FFPROBE").filter(|value| !value.is_empty())
    {
        return PathBuf::from(configured);
    }

    paired_ffprobe_path(ffmpeg)
}

fn paired_ffprobe_path(ffmpeg: &Path) -> PathBuf {
    let plain_name = if cfg!(target_os = "windows") {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    let plain = ffmpeg.with_file_name(plain_name);
    if plain.is_file() {
        return plain;
    }

    let Some(ffmpeg_name) = ffmpeg.file_name().and_then(|name| name.to_str()) else {
        return plain;
    };
    let Some(target_suffix) = ffmpeg_name.strip_prefix("ffmpeg-") else {
        return plain;
    };
    ffmpeg.with_file_name(format!("ffprobe-{target_suffix}"))
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
            "hard subtitle rendering requires the bundled FFmpeg sidecar with the libass/ass filter; select soft subtitles explicitly if the sidecar is unavailable"
        );
    }
    let execution = RenderExecution {
        duration_ms,
        progress_sender,
        cancelled,
    };
    let source_video_bitrate_bps = probe_media(ffmpeg, input).await?.video_bitrate_bps;
    burn_ass(
        ffmpeg,
        input,
        ass,
        output,
        source_relative_target_bitrate(source_video_bitrate_bps),
        &execution,
    )
    .await
}

fn source_relative_target_bitrate(source_video_bitrate_bps: Option<u64>) -> Option<u64> {
    source_video_bitrate_bps.map(|bitrate| {
        bitrate.saturating_mul(SOURCE_BITRATE_HEADROOM_NUMERATOR)
            / SOURCE_BITRATE_HEADROOM_DENOMINATOR
    })
}

async fn burn_ass(
    ffmpeg: &Path,
    input: &Path,
    ass: &Path,
    output: &Path,
    target_video_bitrate_bps: Option<u64>,
    execution: &RenderExecution,
) -> Result<RenderOutcome> {
    let absolute_ass = ass.canonicalize().unwrap_or_else(|_| ass.to_path_buf());
    let filter = format!("ass=filename='{}'", escape_filter_path(&absolute_ass));
    burn_ass_with_encoder(
        ffmpeg,
        input,
        output,
        &filter,
        target_video_bitrate_bps,
        execution,
    )
    .await
}

async fn burn_ass_with_encoder(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    filter: &str,
    target_video_bitrate_bps: Option<u64>,
    execution: &RenderExecution,
) -> Result<RenderOutcome> {
    let mut hardware_fallback_reason = Some(
        "VideoToolbox is unavailable; using the bundled LGPL MPEG-4 software encoder".to_string(),
    );
    if supports_encoder(ffmpeg, "h264_videotoolbox").await? {
        eprintln!("ffmpeg render: using VideoToolbox hardware H.264 encoder");
        let result = run_burn_command(
            ffmpeg,
            input,
            output,
            filter,
            EncodingSelection {
                video: HardSubtitleEncoder::VideoToolbox,
                audio: AudioEncoder::Copy,
            },
            target_video_bitrate_bps,
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
                EncodingSelection {
                    video: HardSubtitleEncoder::VideoToolbox,
                    audio: AudioEncoder::Aac,
                },
                target_video_bitrate_bps,
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
            "VideoToolbox render failed; retrying with the LGPL MPEG-4 encoder. Original failure: {}",
            fallback_error
        );
        hardware_fallback_reason = Some(format!(
            "VideoToolbox failed: {}",
            summarize_render_error(&fallback_error)
        ));
    }

    if !supports_encoder(ffmpeg, "mpeg4").await? {
        let prior = hardware_fallback_reason
            .as_deref()
            .unwrap_or("VideoToolbox did not produce an output");
        anyhow::bail!(
            "no usable video encoder remains after {prior}; the configured ffmpeg is missing the LGPL MPEG-4 software encoder"
        );
    }
    eprintln!("ffmpeg render: using the LGPL MPEG-4 software encoder");
    let result = run_burn_command(
        ffmpeg,
        input,
        output,
        filter,
        EncodingSelection {
            video: HardSubtitleEncoder::Mpeg4,
            audio: AudioEncoder::Copy,
        },
        target_video_bitrate_bps,
        execution,
    )
    .await;
    match result {
        Ok(()) => Ok(render_outcome(
            output,
            HardSubtitleEncoder::Mpeg4,
            AudioEncoder::Copy,
            hardware_fallback_reason.clone(),
        )),
        Err(error) if error.downcast_ref::<RenderCancelled>().is_some() => Err(error),
        Err(error) if audio_copy_failed(&error.to_string()) => {
            let copy_error = error.to_string();
            let aac_result = run_burn_command(
                ffmpeg,
                input,
                output,
                filter,
                EncodingSelection {
                    video: HardSubtitleEncoder::Mpeg4,
                    audio: AudioEncoder::Aac,
                },
                target_video_bitrate_bps,
                execution,
            )
            .await;
            match aac_result {
                Ok(()) => Ok(render_outcome(
                    output,
                    HardSubtitleEncoder::Mpeg4,
                    AudioEncoder::Aac,
                    hardware_fallback_reason,
                )),
                Err(error) if error.downcast_ref::<RenderCancelled>().is_some() => Err(error),
                Err(aac_error) => {
                    let prior = hardware_fallback_reason
                        .as_deref()
                        .unwrap_or("VideoToolbox did not produce an output");
                    Err(anyhow::anyhow!(
                        "MPEG-4 software fallback failed after {prior}; copied-audio attempt: {}; AAC retry: {aac_error:#}",
                        summarize_render_error(&copy_error)
                    ))
                }
            }
        }
        Err(error) => {
            let prior = hardware_fallback_reason
                .as_deref()
                .unwrap_or("VideoToolbox did not produce an output");
            Err(anyhow::anyhow!(
                "MPEG-4 software fallback failed after {prior}: {error:#}"
            ))
        }
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
        HardSubtitleEncoder::Mpeg4 => (RenderEncoder::Mpeg4, false),
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
    encoding: EncodingSelection,
    target_video_bitrate_bps: Option<u64>,
    execution: &RenderExecution,
) -> Result<()> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(burn_args(
        input,
        output,
        filter,
        encoding.video,
        encoding.audio,
        target_video_bitrate_bps,
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
    encoder: HardSubtitleEncoder,
    audio_encoder: AudioEncoder,
    target_video_bitrate_bps: Option<u64>,
) -> Vec<OsString> {
    let mut args = vec![
        "-y".into(),
        "-i".into(),
        input.as_os_str().to_os_string(),
        "-vf".into(),
        filter.into(),
    ];
    match encoder {
        HardSubtitleEncoder::VideoToolbox => {
            args.extend([
                "-c:v".into(),
                "h264_videotoolbox".into(),
                "-profile:v".into(),
                "high".into(),
                "-allow_sw".into(),
                "0".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
            append_video_bitrate(
                &mut args,
                target_video_bitrate_bps,
                VIDEOTOOLBOX_QUALITY,
                true,
            );
        }
        HardSubtitleEncoder::Mpeg4 => {
            args.extend([
                "-c:v".into(),
                "mpeg4".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
            append_video_bitrate(&mut args, target_video_bitrate_bps, MPEG4_QUALITY, false);
        }
    }
    args.extend(["-c:a".into()]);
    match audio_encoder {
        AudioEncoder::Copy => args.push("copy".into()),
        AudioEncoder::Aac => args.extend(["aac".into(), "-b:a".into(), "192k".into()]),
    }
    args.extend([
        "-movflags".into(),
        "+faststart".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        output.as_os_str().to_os_string(),
    ]);
    args
}

fn append_video_bitrate(
    args: &mut Vec<OsString>,
    target_bitrate_bps: Option<u64>,
    quality: u8,
    apply_vbv_limit: bool,
) {
    if let Some(bitrate) = target_bitrate_bps {
        args.extend(["-b:v".into(), bitrate.to_string().into()]);
        if apply_vbv_limit {
            args.extend([
                "-maxrate".into(),
                bitrate
                    .saturating_mul(5)
                    .saturating_div(4)
                    .to_string()
                    .into(),
                "-bufsize".into(),
                bitrate.saturating_mul(2).to_string().into(),
            ]);
        }
    } else {
        args.extend(["-q:v".into(), quality.to_string().into()]);
    }
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

    cmd.kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    cmd.kill_on_drop(true);
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
    use super::{
        AudioEncoder, HardSubtitleEncoder, audio_copy_failed, burn_args, paired_ffprobe_path,
        source_relative_target_bitrate,
    };
    use std::path::Path;

    fn string_args(encoder: HardSubtitleEncoder) -> Vec<String> {
        burn_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            "ass=subtitle.ass",
            encoder,
            AudioEncoder::Copy,
            None,
        )
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
    }

    #[test]
    fn target_suffixed_ffmpeg_uses_the_matching_ffprobe_sidecar() {
        let ffmpeg = Path::new("/tmp/atogaki-no-plain-probe/ffmpeg-aarch64-apple-darwin");
        assert_eq!(
            paired_ffprobe_path(ffmpeg),
            Path::new("/tmp/atogaki-no-plain-probe/ffprobe-aarch64-apple-darwin")
        );
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
    fn software_fallback_uses_the_lgpl_mpeg4_encoder() {
        let args = string_args(HardSubtitleEncoder::Mpeg4);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "mpeg4"]));
        assert!(args.windows(2).any(|pair| pair == ["-q:v", "3"]));
        assert!(!args.iter().any(|argument| argument == "libx264"));
    }

    #[test]
    fn source_relative_bitrate_leaves_modest_subtitle_headroom() {
        assert_eq!(source_relative_target_bitrate(Some(467_249)), Some(560_698));
        assert_eq!(source_relative_target_bitrate(None), None);

        let args = burn_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            "ass=subtitle.ass",
            HardSubtitleEncoder::VideoToolbox,
            AudioEncoder::Copy,
            Some(560_698),
        )
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

        assert!(args.windows(2).any(|pair| pair == ["-b:v", "560698"]));
        assert!(!args.iter().any(|argument| argument == "-q:v"));
    }

    #[test]
    fn detects_mp4_audio_copy_incompatibility() {
        assert!(audio_copy_failed(
            "Could not find tag for codec pcm_s16le in stream #1"
        ));
        assert!(!audio_copy_failed("VideoToolbox session failed"));
    }
}
