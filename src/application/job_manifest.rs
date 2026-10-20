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

/// Accumulated execution time; stopped intervals and queue waits are excluded.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessingTime {
    pub elapsed_seconds: u64,
    pub active_since_unix: Option<u64>,
}

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
    #[serde(default)]
    pub processing_time: Option<ProcessingTime>,
    pub updated_at_unix: u64,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_stage: Option<JobStatus>,
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
            processing_time: Some(ProcessingTime::default()),
            updated_at_unix: now,
            error: None,
            failed_stage: None,
        }
    }

    pub fn mark(&mut self, status: JobStatus) {
        self.mark_at(status, unix_now());
    }

    fn settle_processing_time(&mut self, now: u64) {
        let clock = self.processing_time.get_or_insert_with(|| ProcessingTime {
            elapsed_seconds: match (self.started_at_unix, self.completed_at_unix) {
                (Some(start), Some(end)) => end.saturating_sub(start),
                _ => 0,
            },
            active_since_unix: self
                .status
                .is_processing()
                .then_some(self.started_at_unix)
                .flatten(),
        });
        if let Some(start) = clock.active_since_unix.take() {
            clock.elapsed_seconds = clock
                .elapsed_seconds
                .saturating_add(now.saturating_sub(start));
        }
    }

    fn mark_at(&mut self, status: JobStatus, now: u64) {
        self.settle_processing_time(now);
        self.processing_time.as_mut().unwrap().active_since_unix =
            status.is_processing().then_some(now);
        if status.is_processing() && self.started_at_unix.is_none() {
            self.started_at_unix = Some(now);
        }
        if status.is_terminal() {
            self.completed_at_unix = Some(now);
        } else {
            self.completed_at_unix = None;
        }
        self.status = status;
        self.message = status.label().to_string();
        self.updated_at_unix = now;
        if status != JobStatus::Failed {
            self.error = None;
            self.failed_stage = None;
        }
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.fail_at(error.into(), unix_now());
    }

    /// A crashed process stops at its last persisted heartbeat, not at restart.
    pub fn fail_interrupted(&mut self, error: impl Into<String>, last_observed_unix: u64) {
        self.fail_at(
            error.into(),
            last_observed_unix.max(self.updated_at_unix).min(unix_now()),
        );
    }

    fn fail_at(&mut self, error: String, now: u64) {
        self.settle_processing_time(now);
        if self.status != JobStatus::Failed {
            self.failed_stage = Some(self.status);
        }
        self.status = JobStatus::Failed;
        self.message = JobStatus::Failed.label().to_string();
        self.completed_at_unix = Some(now);
        self.updated_at_unix = now;
        self.error = Some(error);
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

    fn legacy_queued_manifest() -> JobManifest {
        serde_json::from_value(serde_json::json!({
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
        .unwrap()
    }

    #[test]
    fn processing_timestamps_exclude_time_spent_queued() {
        let mut manifest = legacy_queued_manifest();
        manifest.mark(crate::application::job_status::JobStatus::Queued);
        assert_eq!(manifest.started_at_unix, None);
        manifest.mark(crate::application::job_status::JobStatus::ExtractingAudio);
        let started = manifest.started_at_unix;
        assert!(started.is_some());
        manifest.mark(crate::application::job_status::JobStatus::Done);
        assert_eq!(manifest.started_at_unix, started);
        assert!(manifest.completed_at_unix >= started);
    }

    #[test]
    fn execution_clock_excludes_failure_pause_and_resume_queue_intervals() {
        use crate::application::job_status::JobStatus::*;
        let mut m = legacy_queued_manifest();
        m.mark_at(ExtractingAudio, 100);
        m.mark_at(Transcribing, 110);
        m.fail_at("paused".into(), 130);
        assert_eq!(m.processing_time.as_ref().unwrap().elapsed_seconds, 30);
        assert_eq!(m.processing_time.as_ref().unwrap().active_since_unix, None);
        m.mark_at(Queued, 3600);
        m.mark_at(Transcribing, 4000);
        m.mark_at(Translating, 4010);
        m.mark_at(Done, 4030);
        assert_eq!(m.processing_time.as_ref().unwrap().elapsed_seconds, 60);
        assert_eq!(m.started_at_unix, Some(100));
        let restored: JobManifest =
            serde_json::from_slice(&serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(restored.processing_time.unwrap().elapsed_seconds, 60);
    }

    #[test]
    fn legacy_failure_duration_is_frozen_before_resuming() {
        use crate::application::job_status::JobStatus::*;
        let mut m = legacy_queued_manifest();
        m.status = Failed;
        m.started_at_unix = Some(100);
        m.completed_at_unix = Some(130);
        m.mark_at(Queued, 3600);
        m.mark_at(Transcribing, 4000);
        m.fail_at("failed".into(), 4010);
        assert_eq!(m.processing_time.unwrap().elapsed_seconds, 40);
    }

    #[test]
    fn crash_recovery_stops_clock_at_last_observation() {
        use crate::application::job_status::JobStatus::*;
        let mut m = legacy_queued_manifest();
        m.mark_at(Transcribing, 100);
        m.fail_interrupted("interrupted", 130);
        assert_eq!(m.processing_time.unwrap().elapsed_seconds, 30);
        assert_eq!(m.completed_at_unix, Some(130));
    }
}
