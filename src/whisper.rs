use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::segment::TranscriptSegment;

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
    model: &Path,
    wav: &Path,
    output_prefix: &Path,
) -> Result<Vec<TranscriptSegment>> {
    if !model.exists() {
        anyhow::bail!("Whisper model does not exist: {}", model.display());
    }

    let mut cmd = Command::new(whisper_cli);
    cmd.args(["-m"]);
    cmd.arg(model);
    cmd.args(["-l", "ja", "-sns", "-nth", "0.30", "-oj", "-otxt", "-of"]);
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
