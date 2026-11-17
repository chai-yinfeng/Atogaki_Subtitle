//! Local MLX Qwen + forced alignment; shares the durable acoustic executor.
use crate::application::{
    AsrDataLocality, AsrFuture, AsrProvider, AsrProviderCapabilities, AsrProviderStatus,
    AsrRequest, AsrResponse, AsrTimingGranularity, TimedUnit,
};
use anyhow::{Context, bail};
use serde::Deserialize;
use std::{path::PathBuf, process::Stdio};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct QwenRuntime {
    pub python: PathBuf,
    pub worker_root: PathBuf,
}
#[derive(Debug, Clone)]
pub struct QwenAsrProvider {
    pub runtime: Option<QwenRuntime>,
}
#[derive(Deserialize)]
struct BridgeResponse {
    raw_output_path: PathBuf,
    timed_units: Vec<TimedUnit>,
}
impl AsrProvider for QwenAsrProvider {
    fn status(&self) -> AsrProviderStatus {
        AsrProviderStatus {
            id: "mlx-qwen3-asr".into(),
            name: "Qwen3-ASR + ForcedAligner (MLX)".into(),
            capabilities: AsrProviderCapabilities {
                timing_granularities: vec![AsrTimingGranularity::Word],
                vocabulary_biasing: true,
                diarization: false,
                streaming: false,
                data_locality: AsrDataLocality::Local,
            },
        }
    }
    fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
        Box::pin(async move {
            let runtime = self
                .runtime
                .as_ref()
                .context("Qwen offline runtime is not configured")?;
            let aligner = request
                .transcription
                .alignment_model
                .as_ref()
                .context("Qwen requires a local ForcedAligner model")?;
            if !request.transcription.whisper.model.is_dir() || !aligner.is_dir() {
                bail!("Qwen and ForcedAligner must be existing local model directories")
            }
            if request.transcription.whisper.vad_model.is_some()
                || request.transcription.whisper.no_gpu
            {
                bail!("Qwen does not use Whisper GGML VAD or CPU-only options")
            }
            let context = crate::domain::glossary::build_whisper_prompt(&request.transcription)?;
            let audio = request
                .audio_path
                .canonicalize()
                .context("resolve Qwen input WAV")?;
            let output_prefix = request
                .output_prefix
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .canonicalize()?
                .join(
                    request
                        .output_prefix
                        .file_name()
                        .context("Qwen output prefix missing")?,
                );
            let payload = serde_json::to_vec(&serde_json::json!({"audio":audio,
                "model":request.transcription.whisper.model.canonicalize()?,"alignment_model":aligner.canonicalize()?,
                "language":request.transcription.source_language.as_str(),"context":context,
                "output_prefix":output_prefix}))?;
            let mut python_paths = vec![runtime.worker_root.clone()];
            if let Some(runtimes) = runtime
                .python
                .parent()
                .and_then(|bin| bin.parent())
                .and_then(|online| online.parent())
            {
                let alignment = runtimes.join("qwen-alignment");
                if alignment.is_dir() {
                    python_paths.push(alignment);
                }
            }
            let mut command =
                crate::infrastructure::child_process::sidecar_command(&runtime.python);
            command
                .arg("-m")
                .arg("online_worker.offline_asr")
                .current_dir(&runtime.worker_root)
                .env("PYTHONPATH", std::env::join_paths(python_paths)?)
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .env("HF_HUB_OFFLINE", "1")
                .env("TRANSFORMERS_OFFLINE", "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command.spawn().context("start local Qwen ASR bridge")?;
            child
                .stdin
                .take()
                .context("Qwen stdin missing")?
                .write_all(&payload)
                .await?;
            let output = child.wait_with_output().await?;
            if !output.status.success() {
                bail!(
                    "Qwen ASR/alignment failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
            }
            let response: BridgeResponse =
                serde_json::from_slice(&output.stdout).context("invalid Qwen ASR response")?;
            for unit in &response.timed_units {
                if unit.timing_source != crate::application::TimingSource::ForcedAlignment
                    || unit.start_ms.zip(unit.end_ms).is_none_or(|(a, b)| b <= a)
                {
                    bail!("Qwen returned invalid forced-alignment evidence")
                }
            }
            Ok(AsrResponse {
                provider_id: self.status().id,
                model_identity: request.transcription.whisper.model.display().to_string(),
                raw_output_path: response.raw_output_path,
                timed_units: response.timed_units,
                legacy_segments: vec![],
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{AsrInputScope, TranscriptionOptions};
    #[tokio::test]
    async fn missing_alignment_is_rejected_before_spawning_python() {
        let provider = QwenAsrProvider {
            runtime: Some(QwenRuntime {
                python: "must-not-run".into(),
                worker_root: "unused".into(),
            }),
        };
        let error = provider
            .transcribe(AsrRequest {
                audio_path: "unused".into(),
                output_prefix: "unused".into(),
                scope: AsrInputScope::full(),
                cloud_audio_upload_authorized: false,
                transcription: TranscriptionOptions::japanese("model".into()),
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("ForcedAligner"));
    }
}
