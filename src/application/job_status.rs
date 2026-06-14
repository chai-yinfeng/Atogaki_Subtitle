use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Created,
    ExtractingAudio,
    Transcribing,
    RefiningSegments,
    Translating,
    ExportingSubtitles,
    RenderingVideo,
    Done,
    Failed,
}

impl JobStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::ExtractingAudio => "extracting audio",
            Self::Transcribing => "transcribing",
            Self::RefiningSegments => "refining segments",
            Self::Translating => "translating",
            Self::ExportingSubtitles => "exporting subtitles",
            Self::RenderingVideo => "rendering video",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}
