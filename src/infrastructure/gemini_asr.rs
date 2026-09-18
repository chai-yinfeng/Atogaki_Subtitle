use std::{path::Path, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::{Client, StatusCode, Url, header::HeaderMap};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::{
    application::{
        AsrDataLocality, AsrFuture, AsrProvider, AsrProviderCapabilities, AsrProviderStatus,
        AsrRequest, AsrResponse, AsrTimingGranularity, TimedUnit, TimedUnitKind, TimingSource,
    },
    domain::{LanguageCode, TranscriptSegment},
};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const MODEL: &str = "gemini-3.5-transcribe";

#[derive(Clone)]
pub struct GeminiAsrConfig {
    pub api_key: String,
    pub word_timestamps: bool,
    pub speaker_diarization: bool,
    pub base_url: String,
}

impl GeminiAsrConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            word_timestamps: true,
            speaker_diarization: false,
            base_url: DEFAULT_BASE_URL.into(),
        }
    }
}

#[derive(Clone)]
pub struct GeminiAsrProvider {
    client: Client,
    config: GeminiAsrConfig,
}

impl std::fmt::Debug for GeminiAsrProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GeminiAsrProvider")
            .field("model", &MODEL)
            .field("word_timestamps", &self.config.word_timestamps)
            .field("speaker_diarization", &self.config.speaker_diarization)
            .field("configured", &true)
            .finish()
    }
}

impl GeminiAsrProvider {
    pub fn new(config: GeminiAsrConfig) -> Result<Self> {
        if config.api_key.trim().is_empty() {
            bail!("Gemini API key is missing");
        }
        validate_base_url(&config.base_url)?;
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(60 * 30))
                .build()
                .context("failed to build Gemini ASR client")?,
            config,
        })
    }

    async fn transcribe_file(&self, request: AsrRequest) -> Result<AsrResponse> {
        if !request.cloud_audio_upload_authorized {
            bail!("Gemini audio upload requires explicit authorization for this ASR run");
        }
        if !self.config.word_timestamps {
            bail!("Atogaki requires Gemini word timestamps for subtitle segmentation");
        }
        let (file_name, file_uri) = self.upload_audio(&request.audio_path).await?;
        let interaction = self.create_interaction(&file_uri, &request).await;
        let deletion = self.delete_file(&file_name).await;
        let interaction = interaction?;
        if let Err(error) = deletion {
            eprintln!("[gemini-asr] failed to delete uploaded audio {file_name}: {error:#}");
        }
        let raw_output_path = request.output_prefix.with_extension("json");
        tokio::fs::write(&raw_output_path, serde_json::to_vec_pretty(&interaction)?)
            .await
            .with_context(|| format!("failed to write {}", raw_output_path.display()))?;
        let timed_units = parse_word_annotations(&interaction)?;
        if timed_units.is_empty() {
            bail!("Gemini returned no word timestamps; refusing to invent cue timing");
        }
        let legacy_segments = timed_units
            .iter()
            .filter_map(|unit| {
                Some(TranscriptSegment::new(
                    unit.start_ms?,
                    unit.end_ms?,
                    unit.text.clone(),
                ))
            })
            .collect();
        Ok(AsrResponse {
            provider_id: "gemini-transcribe".into(),
            model_identity: MODEL.into(),
            raw_output_path,
            timed_units,
            legacy_segments,
        })
    }

    async fn upload_audio(&self, audio_path: &Path) -> Result<(String, String)> {
        let metadata = tokio::fs::metadata(audio_path)
            .await
            .with_context(|| format!("failed to inspect {}", audio_path.display()))?;
        let start = self
            .client
            .post(format!("{}/upload/v1beta/files", self.config.base_url))
            .header("x-goog-api-key", &self.config.api_key)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", metadata.len())
            .header("X-Goog-Upload-Header-Content-Type", "audio/wav")
            .json(&json!({"file": {"display_name": "Atogaki ASR audio"}}))
            .send()
            .await
            .context("failed to start Gemini audio upload")?;
        let status = start.status();
        let headers = start.headers().clone();
        let body = start.text().await.unwrap_or_default();
        ensure_success(status, &body, "start Gemini audio upload")?;
        let upload_url = upload_url(&headers)?;

        let mut file = tokio::fs::File::open(audio_path)
            .await
            .with_context(|| format!("failed to open {}", audio_path.display()))?;
        let mut offset = 0_u64;
        let body = loop {
            let remaining = metadata.len().saturating_sub(offset);
            let mut chunk = vec![0_u8; remaining.min(8 * 1024 * 1024) as usize];
            file.read_exact(&mut chunk)
                .await
                .context("failed to read audio during Gemini upload")?;
            let final_chunk = offset + chunk.len() as u64 == metadata.len();
            let uploaded = self
                .client
                .post(upload_url.clone())
                .header("Content-Length", chunk.len())
                .header("X-Goog-Upload-Offset", offset)
                .header(
                    "X-Goog-Upload-Command",
                    if final_chunk {
                        "upload, finalize"
                    } else {
                        "upload"
                    },
                )
                .body(chunk)
                .send()
                .await
                .context("failed to upload audio to Gemini")?;
            let status = uploaded.status();
            let response_body = uploaded
                .text()
                .await
                .context("failed to read Gemini upload response")?;
            ensure_success(status, &response_body, "upload audio to Gemini")?;
            if final_chunk {
                break response_body;
            }
            offset += 8 * 1024 * 1024;
        };
        let value: Value = serde_json::from_str(&body).context("invalid Gemini upload response")?;
        let file = value
            .get("file")
            .ok_or_else(|| anyhow!("Gemini upload response has no file"))?;
        Ok((
            required_string(file, "name")?,
            required_string(file, "uri")?,
        ))
    }

    async fn create_interaction(&self, file_uri: &str, request: &AsrRequest) -> Result<Value> {
        let mut mode = json!({"type": "verbatim", "timestamp_granularities": ["word"]});
        if self.config.speaker_diarization {
            mode["diarization_mode"] = json!("speaker");
        }
        let language = match request.transcription.source_language {
            LanguageCode::Japanese => "ja-JP",
            LanguageCode::English => "en-US",
            LanguageCode::Korean => "ko-KR",
            LanguageCode::SimplifiedChinese => "cmn-Hans-CN",
        };
        let body = json!({
            "model": MODEL,
            "input": [{"type": "audio", "uri": file_uri, "mime_type": "audio/wav"}],
            "generation_config": {"transcription_config": {
                "language_codes": [language], "mode": mode
            }}
        });
        let response = self
            .client
            .post(format!("{}/v1beta/interactions", self.config.base_url))
            .header("x-goog-api-key", &self.config.api_key)
            .json(&body)
            .send()
            .await
            .context("failed to call Gemini Transcribe")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read Gemini response")?;
        ensure_success(status, &body, "transcribe audio with Gemini")?;
        serde_json::from_str(&body).context("invalid Gemini transcription response")
    }

    async fn delete_file(&self, file_name: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/v1beta/{file_name}", self.config.base_url))
            .header("x-goog-api-key", &self.config.api_key)
            .send()
            .await
            .context("failed to delete Gemini upload")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        ensure_success(status, &body, "delete Gemini upload")
    }
}

impl AsrProvider for GeminiAsrProvider {
    fn status(&self) -> AsrProviderStatus {
        AsrProviderStatus {
            id: "gemini-transcribe".into(),
            name: "Gemini 3.5 Transcribe".into(),
            capabilities: AsrProviderCapabilities {
                timing_granularities: vec![AsrTimingGranularity::Word],
                vocabulary_biasing: true,
                diarization: true,
                streaming: false,
                data_locality: AsrDataLocality::CloudAudioUpload,
            },
        }
    }

    fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
        Box::pin(self.transcribe_file(request))
    }
}

fn parse_word_annotations(value: &Value) -> Result<Vec<TimedUnit>> {
    let mut units = Vec::new();
    for step in value
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for content in step
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for annotation in content
                .get("annotations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if annotation.get("type").and_then(Value::as_str) != Some("word_info") {
                    continue;
                }
                units.push(TimedUnit {
                    id: Uuid::new_v4().to_string(),
                    text: required_string(annotation, "text")?,
                    kind: TimedUnitKind::Word,
                    start_ms: Some(parse_offset(required_str(annotation, "start_offset")?)?),
                    end_ms: Some(parse_offset(required_str(annotation, "end_offset")?)?),
                    timing_source: TimingSource::Model,
                    provider_confidence: None,
                    speaker: annotation
                        .get("speaker")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
        }
    }
    Ok(units)
}

fn parse_offset(value: &str) -> Result<u64> {
    let seconds = value
        .strip_suffix('s')
        .ok_or_else(|| anyhow!("invalid Gemini time offset: {value}"))?
        .parse::<f64>()
        .with_context(|| format!("invalid Gemini time offset: {value}"))?;
    if !seconds.is_finite() || seconds < 0.0 {
        bail!("invalid Gemini time offset: {value}");
    }
    Ok((seconds * 1_000.0).round() as u64)
}

fn upload_url(headers: &HeaderMap) -> Result<Url> {
    let value = headers
        .get("x-goog-upload-url")
        .ok_or_else(|| anyhow!("Gemini upload response has no upload URL"))?
        .to_str()
        .context("invalid Gemini upload URL header")?;
    let url = Url::parse(value).context("invalid Gemini upload URL")?;
    if url.scheme() != "https" || url.host_str() != Some("generativelanguage.googleapis.com") {
        bail!("Gemini returned an unexpected upload URL");
    }
    Ok(url)
}

fn validate_base_url(value: &str) -> Result<()> {
    let url = Url::parse(value).context("invalid Gemini base URL")?;
    if url.scheme() != "https" || url.host_str() != Some("generativelanguage.googleapis.com") {
        bail!("Gemini base URL must use the official HTTPS endpoint");
    }
    Ok(())
}

fn required_string(value: &Value, field: &str) -> Result<String> {
    Ok(required_str(value, field)?.to_string())
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Gemini response is missing {field}"))
}

fn ensure_success(status: StatusCode, body: &str, action: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(500).collect());
    bail!("failed to {action}: HTTP {status}: {message}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_offset, parse_word_annotations};

    #[test]
    fn parses_word_timing_and_speaker() {
        let units = parse_word_annotations(&json!({
            "steps": [{"content": [{"annotations": [
                {"type": "word_info", "text": "こんにちは", "speaker": "spk_1",
                 "start_offset": "0.100s", "end_offset": "0.850s"}
            ]}]}]
        }))
        .unwrap();
        assert_eq!(units.len(), 1);
        assert_eq!((units[0].start_ms, units[0].end_ms), (Some(100), Some(850)));
        assert_eq!(units[0].speaker.as_deref(), Some("spk_1"));
    }

    #[test]
    fn rejects_invalid_offsets() {
        assert_eq!(parse_offset("1.234s").unwrap(), 1_234);
        assert!(parse_offset("-1s").is_err());
        assert!(parse_offset("later").is_err());
    }
}
