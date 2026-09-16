use std::{fmt::Debug, future::Future, path::PathBuf, pin::Pin};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{application::TranscriptionOptions, domain::TranscriptSegment};

pub type AsrFuture<'a> = Pin<Box<dyn Future<Output = Result<AsrResponse>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrTimingGranularity {
    ProviderSegment,
    Token,
    Word,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsrProviderCapabilities {
    pub timing_granularities: Vec<AsrTimingGranularity>,
    pub vocabulary_biasing: bool,
    pub diarization: bool,
    pub streaming: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsrProviderStatus {
    pub id: String,
    pub name: String,
    pub capabilities: AsrProviderCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrRunKind {
    Full,
    SelectedRange,
}

impl AsrRunKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::SelectedRange => "selected_range",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsrInputScope {
    pub kind: AsrRunKind,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
}

impl AsrInputScope {
    pub fn full() -> Self {
        Self {
            kind: AsrRunKind::Full,
            start_ms: 0,
            end_ms: None,
        }
    }

    pub fn selected_range(start_ms: u64, end_ms: u64) -> Result<Self> {
        if end_ms <= start_ms {
            anyhow::bail!("ASR input range end must be after its start");
        }
        Ok(Self {
            kind: AsrRunKind::SelectedRange,
            start_ms,
            end_ms: Some(end_ms),
        })
    }
}

#[derive(Debug, Clone)]
pub struct AsrRequest {
    pub audio_path: PathBuf,
    pub output_prefix: PathBuf,
    pub scope: AsrInputScope,
    /// Compatibility input for the first provider extraction milestone.
    /// The next milestone separates generic run input from Whisper config while
    /// retaining the legacy JSON shape through serde flattening.
    pub transcription: TranscriptionOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimedUnitKind {
    ProviderSegment,
    Token,
    Word,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingSource {
    Model,
    ForcedAlignment,
    Interpolated,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedUnit {
    pub id: String,
    pub text: String,
    pub kind: TimedUnitKind,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub timing_source: TimingSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct AsrResponse {
    pub provider_id: String,
    pub model_identity: String,
    pub raw_output_path: PathBuf,
    pub timed_units: Vec<TimedUnit>,
    /// Compatibility projection consumed by the current subtitle segmenter.
    pub legacy_segments: Vec<TranscriptSegment>,
}

pub trait OfflineAsrProvider: Debug + Send + Sync {
    fn status(&self) -> AsrProviderStatus;
    fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a>;
}

#[cfg(test)]
mod tests {
    use super::{AsrInputScope, AsrRunKind};

    #[test]
    fn validates_selected_input_ranges() {
        assert!(AsrInputScope::selected_range(1_000, 1_000).is_err());
        let scope = AsrInputScope::selected_range(1_000, 2_000).unwrap();
        assert_eq!(scope.kind, AsrRunKind::SelectedRange);
        assert_eq!(scope.end_ms, Some(2_000));
    }
}
