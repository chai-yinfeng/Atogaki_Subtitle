use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Options for an ASR run, independent from a particular user interface.
///
/// CLI, desktop UI, and future automation callers each translate their own
/// input representation into this type before invoking the application layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionOptions {
    pub model: PathBuf,
    pub source_language: String,
    pub glossary: Option<PathBuf>,
    pub prompt: Option<String>,
    pub vad_model: Option<PathBuf>,
    pub vad_threshold: f32,
    pub vad_min_speech_ms: u64,
    pub vad_min_silence_ms: u64,
    pub vad_max_speech_s: u64,
    pub vad_speech_pad_ms: u64,
    pub max_len: u32,
    pub split_on_word: bool,
    pub no_speech_threshold: f32,
    pub output_json_full: bool,
    pub no_gpu: bool,
}

impl TranscriptionOptions {
    pub fn japanese(model: PathBuf) -> Self {
        Self {
            model,
            source_language: "ja".to_string(),
            glossary: None,
            prompt: None,
            vad_model: None,
            vad_threshold: 0.50,
            vad_min_speech_ms: 250,
            vad_min_silence_ms: 450,
            vad_max_speech_s: 8,
            vad_speech_pad_ms: 120,
            max_len: 32,
            split_on_word: true,
            no_speech_threshold: 0.30,
            output_json_full: false,
            no_gpu: false,
        }
    }
}
