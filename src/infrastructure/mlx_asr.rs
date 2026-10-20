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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{AsrInputScope, TranscriptionOptions};

    #[tokio::test]
    async fn unsupported_runtime_options_are_rejected_before_spawning_python() {
        let model =
            std::env::temp_dir().join(format!("atogaki-mlx-options-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&model).unwrap();
        let provider = MlxWhisperProvider {
            python: "must-not-run".into(),
            model: model.clone(),
        };
        for variant in 0..3 {
            let mut transcription = TranscriptionOptions::japanese(model.clone());
            match variant {
                0 => transcription.whisper.no_gpu = true,
                1 => transcription.whisper.vad_model = Some("vad.bin".into()),
                _ => transcription.whisper.max_context = Some(100),
            }
            let error = provider
                .transcribe(AsrRequest {
                    audio_path: "unused.wav".into(),
                    output_prefix: "unused".into(),
                    scope: AsrInputScope::full(),
                    cloud_audio_upload_authorized: false,
                    transcription,
                })
                .await
                .unwrap_err();
            assert!(error.to_string().contains("does not support"));
        }
        std::fs::remove_dir(model).unwrap();
    }
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
            if request.transcription.whisper.vad_model.is_some()
                || request.transcription.whisper.no_gpu
                || request
                    .transcription
                    .whisper
                    .max_context
                    .is_some_and(|value| value != 0)
            {
                bail!(
                    "MLX experiment does not support Silero VAD, CPU mode, or a bounded text-context token count; use an explicitly supported configuration"
                )
            }
            let prompt = crate::domain::glossary::build_whisper_prompt(&request.transcription)?;
            let payload = serde_json::to_vec(
                &serde_json::json!({"audio":request.audio_path,"model":self.model,"output_prefix":request.output_prefix,"language":request.transcription.source_language.whisper_code(),"prompt":prompt,"max_context":request.transcription.whisper.max_context,"no_speech_threshold":request.transcription.whisper.no_speech_threshold}),
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
