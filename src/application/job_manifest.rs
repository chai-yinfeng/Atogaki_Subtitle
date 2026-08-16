use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    application::job_status::JobStatus,
    domain::{LanguageCode, LanguagePair},
    infrastructure::job_store::Job,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobManifest {
    pub job_id: String,
    pub status: JobStatus,
    pub message: String,
    pub input: Option<PathBuf>,
    pub render_output: Option<PathBuf>,
    #[serde(default = "default_source_language")]
    pub source_language: LanguageCode,
    #[serde(default = "default_target_language")]
    pub target_language: LanguageCode,
    pub outputs: JobOutputs,
    pub created_at_unix: u64,
    #[serde(default)]
    pub started_at_unix: Option<u64>,
    #[serde(default)]
    pub completed_at_unix: Option<u64>,
    pub updated_at_unix: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOutputs {
    pub job_dir: PathBuf,
    pub audio_wav: PathBuf,
    pub segments_json: PathBuf,
    #[serde(alias = "ja_srt")]
    pub source_srt: PathBuf,
    #[serde(alias = "zh_srt")]
    pub translated_srt: PathBuf,
    pub bilingual_srt: PathBuf,
    pub bilingual_ass: PathBuf,
}

impl JobManifest {
    pub fn new(
        job: &Job,
        input: Option<PathBuf>,
        render_output: Option<PathBuf>,
        languages: LanguagePair,
    ) -> Self {
        let now = unix_now();
        Self {
            job_id: job.id(),
            status: JobStatus::Created,
            message: JobStatus::Created.label().to_string(),
            input,
            render_output,
            source_language: languages.source,
            target_language: languages.target,
            outputs: JobOutputs::from(job),
            created_at_unix: now,
            started_at_unix: None,
            completed_at_unix: None,
            updated_at_unix: now,
            error: None,
        }
    }

    pub fn mark(&mut self, status: JobStatus) {
        let now = unix_now();
        if status.is_processing() && self.started_at_unix.is_none() {
            self.started_at_unix = Some(now);
        }
        if status.is_terminal() {
            self.completed_at_unix = Some(now);
        }
        self.status = status;
        self.message = status.label().to_string();
        self.updated_at_unix = now;
        if status != JobStatus::Failed {
            self.error = None;
        }
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        let now = unix_now();
        self.status = JobStatus::Failed;
        self.message = JobStatus::Failed.label().to_string();
        self.completed_at_unix = Some(now);
        self.updated_at_unix = now;
        self.error = Some(error.into());
    }

    pub fn replace_input(&mut self, input: PathBuf) {
        self.input = Some(input);
        self.updated_at_unix = unix_now();
    }
}

fn default_source_language() -> LanguageCode {
    LanguageCode::Japanese
}

fn default_target_language() -> LanguageCode {
    LanguageCode::SimplifiedChinese
}

impl From<&Job> for JobOutputs {
    fn from(job: &Job) -> Self {
        Self {
            job_dir: job.dir.clone(),
            audio_wav: job.audio_wav.clone(),
            segments_json: job.segments_json.clone(),
            source_srt: job.source_srt.clone(),
            translated_srt: job.translated_srt.clone(),
            bilingual_srt: job.bilingual_srt.clone(),
            bilingual_ass: job.bilingual_ass.clone(),
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub fn job_id_from_dir(dir: &Path) -> String {
    dir.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("job")
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::domain::LanguageCode;

    use super::JobManifest;

    #[test]
    fn legacy_manifest_defaults_to_japanese_and_accepts_old_output_names() {
        let manifest: JobManifest = serde_json::from_value(serde_json::json!({
            "job_id": "legacy-job",
            "status": "done",
            "message": "done",
            "input": "/media/radio.mp4",
            "render_output": null,
            "outputs": {
                "job_dir": "/tasks/legacy-job",
                "audio_wav": "/tasks/legacy-job/audio.wav",
                "segments_json": "/tasks/legacy-job/segments.json",
                "ja_srt": "/tasks/legacy-job/ja.srt",
                "zh_srt": "/tasks/legacy-job/zh.srt",
                "bilingual_srt": "/tasks/legacy-job/bilingual.srt",
                "bilingual_ass": "/tasks/legacy-job/bilingual.ass"
            },
            "created_at_unix": 1,
            "updated_at_unix": 2,
            "error": null
        }))
        .unwrap();

        assert_eq!(manifest.source_language, LanguageCode::Japanese);
        assert_eq!(manifest.target_language, LanguageCode::SimplifiedChinese);
        assert!(manifest.outputs.source_srt.ends_with("ja.srt"));
        assert!(manifest.outputs.translated_srt.ends_with("zh.srt"));
        assert_eq!(manifest.started_at_unix, None);
        assert_eq!(manifest.completed_at_unix, None);
    }

    #[test]
    fn processing_timestamps_exclude_time_spent_queued() {
        let mut manifest: JobManifest = serde_json::from_value(serde_json::json!({
            "job_id": "timed-job",
            "status": "queued",
            "message": "queued",
            "input": "/media/radio.mp4",
            "render_output": null,
            "outputs": {
                "job_dir": "/tasks/timed-job",
                "audio_wav": "/tasks/timed-job/audio.wav",
                "segments_json": "/tasks/timed-job/segments.json",
                "source_srt": "/tasks/timed-job/source.srt",
                "translated_srt": "/tasks/timed-job/translated.srt",
                "bilingual_srt": "/tasks/timed-job/bilingual.srt",
                "bilingual_ass": "/tasks/timed-job/bilingual.ass"
            },
            "created_at_unix": 1,
            "updated_at_unix": 1,
            "error": null
        }))
        .unwrap();

        manifest.mark(crate::application::job_status::JobStatus::Queued);
        assert_eq!(manifest.started_at_unix, None);
        manifest.mark(crate::application::job_status::JobStatus::ExtractingAudio);
        let started = manifest.started_at_unix;
        assert!(started.is_some());
        manifest.mark(crate::application::job_status::JobStatus::Done);
        assert_eq!(manifest.started_at_unix, started);
        assert!(manifest.completed_at_unix >= started);
    }
}
