//! Experimental CLI adapter. It shares the durable executor; desktop does not
//! register this provider until separate quality and distribution acceptance.
use crate::application::{
    AsrDataLocality, AsrFuture, AsrProvider, AsrProviderCapabilities, AsrProviderStatus,
    AsrRequest, AsrResponse, AsrTimingGranularity, TimedUnit,
};
use anyhow::{Context, bail};
use serde::Deserialize;
use std::{path::PathBuf, process::Stdio};
use tokio::io::AsyncWriteExt;
#[derive(Debug, Clone)]
pub struct MlxWhisperProvider {
    pub python: PathBuf,
    pub model: PathBuf,
}
#[derive(Deserialize)]
struct BridgeResponse {
    raw_output_path: PathBuf,
    timed_units: Vec<TimedUnit>,
}
impl AsrProvider for MlxWhisperProvider {
    fn status(&self) -> AsrProviderStatus {
        AsrProviderStatus {
            id: "mlx-whisper".into(),
            name: "MLX Whisper (CLI experimental)".into(),
            capabilities: AsrProviderCapabilities {
                timing_granularities: vec![
                    AsrTimingGranularity::Word,
                    AsrTimingGranularity::ProviderSegment,
                ],
                vocabulary_biasing: true,
                diarization: false,
                streaming: false,
                data_locality: AsrDataLocality::Local,
            },
        }
    }
    fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
        Box::pin(async move {
            if !self.model.is_dir() {
                bail!("MLX model must be an existing local directory")
            }
            let prompt = crate::domain::glossary::build_whisper_prompt(&request.transcription)?;
            let payload = serde_json::to_vec(
                &serde_json::json!({"audio":request.audio_path,"model":self.model,"output_prefix":request.output_prefix,"language":request.transcription.source_language.whisper_code(),"prompt":prompt,"max_context":request.transcription.whisper.max_context}),
            )?;
            let mut command = crate::infrastructure::child_process::sidecar_command(&self.python);
            command
                .arg(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/mlx-whisper-bridge.py"),
                )
                .env("HF_HUB_OFFLINE", "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command
                .spawn()
                .context("start MLX bridge; install explicit experimental dependencies first")?;
            child
                .stdin
                .take()
                .context("MLX bridge stdin missing")?
                .write_all(&payload)
                .await?;
            let output = child.wait_with_output().await?;
            if !output.status.success() {
                bail!(
                    "MLX bridge failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
            }
            let response: BridgeResponse =
                serde_json::from_slice(&output.stdout).context("invalid MLX ASR contract")?;
            Ok(AsrResponse {
                provider_id: "mlx-whisper".into(),
                model_identity: self.model.display().to_string(),
                raw_output_path: response.raw_output_path,
                timed_units: response.timed_units,
                legacy_segments: vec![],
            })
        })
    }
}
