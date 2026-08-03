use std::path::PathBuf;

/// Options for an ASR run, independent from a particular user interface.
///
/// CLI, desktop UI, and future automation callers each translate their own
/// input representation into this type before invoking the application layer.
#[derive(Debug, Clone)]
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
