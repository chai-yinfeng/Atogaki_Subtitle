use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::LanguageCode;

/// Options for an ASR run, independent from a particular user interface.
///
/// CLI, desktop UI, and future automation callers each translate their own
/// input representation into this type before invoking the application layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionOptions {
    #[serde(default)]
    pub source_language: LanguageCode,
    pub glossary: Option<PathBuf>,
    pub prompt: Option<String>,
    #[serde(flatten)]
    pub whisper: WhisperTranscriptionConfig,
}

/// Provider-specific configuration for the bundled whisper.cpp adapter.
///
/// `TranscriptionOptions` flattens this value during serialization so existing
/// `recognition-options.json` files retain their original shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperTranscriptionConfig {
    pub model: PathBuf,
    pub vad_model: Option<PathBuf>,
    pub vad_threshold: f32,
    pub vad_min_speech_ms: u64,
    pub vad_min_silence_ms: u64,
    pub vad_max_speech_s: u64,
    pub vad_speech_pad_ms: u64,
    pub max_len: u32,
    pub split_on_word: bool,
    pub no_speech_threshold: f32,
    /// Maximum number of previous text tokens Whisper may carry into the next
    /// decoding window. `None` keeps whisper.cpp's default; `Some(0)` makes an
    /// explicitly isolated pass for repairing a selected range.
    #[serde(default)]
    pub max_context: Option<u32>,
    pub output_json_full: bool,
    pub no_gpu: bool,
}

impl TranscriptionOptions {
    pub fn new(model: PathBuf, source_language: LanguageCode) -> Self {
        Self {
            source_language,
            glossary: None,
            prompt: None,
            whisper: WhisperTranscriptionConfig {
                model,
                vad_model: None,
                vad_threshold: 0.50,
                vad_min_speech_ms: 250,
                vad_min_silence_ms: 450,
                vad_max_speech_s: 8,
                vad_speech_pad_ms: 120,
                max_len: 32,
                split_on_word: true,
                no_speech_threshold: 0.30,
                max_context: None,
                output_json_full: false,
                no_gpu: false,
            },
        }
    }

    pub fn japanese(model: PathBuf) -> Self {
        Self::new(model, LanguageCode::Japanese)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::LanguageCode;

    use super::TranscriptionOptions;

    #[test]
    fn provider_config_keeps_the_legacy_json_shape() {
        let options = TranscriptionOptions::new(PathBuf::from("model.bin"), LanguageCode::Japanese);
        let value = serde_json::to_value(&options).unwrap();
        assert_eq!(value["model"], "model.bin");
        assert_eq!(value["source_language"], "ja");
        assert!(value.get("whisper").is_none());

        let decoded: TranscriptionOptions = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.whisper.model, PathBuf::from("model.bin"));
        assert_eq!(decoded.whisper.vad_max_speech_s, 8);
    }
}
