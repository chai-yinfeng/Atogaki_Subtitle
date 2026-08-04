use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Created => "created",
            Self::ExtractingAudio => "extracting_audio",
            Self::Transcribing => "transcribing",
            Self::RefiningSegments => "refining_segments",
            Self::Translating => "translating",
            Self::ExportingSubtitles => "exporting_subtitles",
            Self::RenderingVideo => "rendering_video",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
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

#[cfg(test)]
mod tests {
    use super::JobStatus;

    #[test]
    fn queued_status_is_stable_in_manifests() {
        assert_eq!(
            serde_json::to_string(&JobStatus::Queued).unwrap(),
            "\"queued\""
        );
    }
}
