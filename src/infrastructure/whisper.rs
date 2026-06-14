use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::{
    domain::{TranscriptSegment, glossary},
    interface::cli::WhisperArgs,
};

#[derive(Debug, Deserialize)]
struct WhisperJson {
    transcription: Vec<WhisperItem>,
}

#[derive(Debug, Deserialize)]
struct WhisperItem {
    offsets: Offsets,
    text: String,
}

#[derive(Debug, Deserialize)]
struct Offsets {
    from: u64,
    to: u64,
}

pub async fn transcribe(
    whisper_cli: &Path,
    options: &WhisperArgs,
    wav: &Path,
    output_prefix: &Path,
) -> Result<Vec<TranscriptSegment>> {
    let model = &options.model;
    if !model.exists() {
        anyhow::bail!("Whisper model does not exist: {}", model.display());
    }
    if let Some(vad_model) = options.vad_model.as_deref()
        && !vad_model.exists()
    {
        anyhow::bail!("VAD model does not exist: {}", vad_model.display());
    }
    let prompt = glossary::build_whisper_prompt(options)?;

    let mut cmd = Command::new(whisper_cli);
    cmd.args(["-m"]);
    cmd.arg(model);
    cmd.args(["-l", "ja", "-sns", "-nth"]);
    cmd.arg(format!("{:.2}", options.no_speech_threshold));

    if options.max_len > 0 {
        cmd.args(["-ml", &options.max_len.to_string()]);
    }
    if options.split_on_word {
        cmd.arg("-sow");
    }
    if options.output_json_full {
        cmd.arg("-ojf");
    } else {
        cmd.arg("-oj");
    }
    if options.no_gpu {
        cmd.arg("--no-gpu");
    }
    if let Some(prompt) = prompt.as_deref() {
        cmd.args(["--prompt", prompt]);
    }
    cmd.arg("-otxt");

    if let Some(vad_model) = options.vad_model.as_deref() {
        cmd.arg("--vad");
        cmd.args(["--vad-model"]);
        cmd.arg(vad_model);
        cmd.args(["--vad-threshold", &format!("{:.2}", options.vad_threshold)]);
        cmd.args([
            "--vad-min-speech-duration-ms",
            &options.vad_min_speech_ms.to_string(),
        ]);
        cmd.args([
            "--vad-min-silence-duration-ms",
            &options.vad_min_silence_ms.to_string(),
        ]);
        cmd.args([
            "--vad-max-speech-duration-s",
            &options.vad_max_speech_s.to_string(),
        ]);
        cmd.args([
            "--vad-speech-pad-ms",
            &options.vad_speech_pad_ms.to_string(),
        ]);
    }

    cmd.arg("-of");
    cmd.arg(output_prefix);
    cmd.arg(wav);

    let output = cmd.output().await.context("failed to start whisper-cli")?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "whisper-cli failed with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            output.status
        );
    }

    let json_path = json_path(output_prefix);
    let data =
        fs::read(&json_path).with_context(|| format!("failed to read {}", json_path.display()))?;
    let decoded: WhisperJson =
        serde_json::from_slice(&data).context("failed to parse whisper json")?;

    Ok(decoded
        .transcription
        .into_iter()
        .filter_map(|item| {
            let text = item.text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(TranscriptSegment {
                start_ms: item.offsets.from,
                end_ms: item.offsets.to,
                ja_text: text,
                zh_text: None,
            })
        })
        .collect())
}

fn json_path(prefix: &Path) -> PathBuf {
    let mut path = prefix.to_path_buf();
    path.set_extension("json");
    path
}
