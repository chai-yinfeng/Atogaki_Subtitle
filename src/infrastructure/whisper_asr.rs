use std::path::PathBuf;

use crate::application::{
    AsrFuture, AsrProviderCapabilities, AsrProviderStatus, AsrRequest, AsrTimingGranularity,
    OfflineAsrProvider,
};

#[derive(Debug, Clone)]
pub struct WhisperAsrProvider {
    whisper_cli: PathBuf,
}

impl WhisperAsrProvider {
    pub fn new(whisper_cli: impl Into<PathBuf>) -> Self {
        Self {
            whisper_cli: whisper_cli.into(),
        }
    }
}

impl OfflineAsrProvider for WhisperAsrProvider {
    fn status(&self) -> AsrProviderStatus {
        AsrProviderStatus {
            id: "whisper.cpp".to_string(),
            name: "Whisper.cpp".to_string(),
            capabilities: AsrProviderCapabilities {
                timing_granularities: vec![AsrTimingGranularity::ProviderSegment],
                vocabulary_biasing: true,
                diarization: false,
                streaming: false,
            },
        }
    }

    fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
        Box::pin(async move {
            crate::infrastructure::whisper::transcribe_response(
                &self.whisper_cli,
                &request.transcription,
                &request.audio_path,
                &request.output_prefix,
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::application::{AsrTimingGranularity, OfflineAsrProvider};

    use super::WhisperAsrProvider;

    #[test]
    fn exposes_current_whisper_capabilities_without_claiming_word_timing() {
        let status = WhisperAsrProvider::new("whisper-cli").status();
        assert_eq!(status.id, "whisper.cpp");
        assert_eq!(
            status.capabilities.timing_granularities,
            vec![AsrTimingGranularity::ProviderSegment]
        );
        assert!(status.capabilities.vocabulary_biasing);
        assert!(!status.capabilities.diarization);
        assert!(!status.capabilities.streaming);
    }
}
