use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{application::job_status::JobStatus, infrastructure::job_store::Job};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobManifest {
    pub job_id: String,
    pub status: JobStatus,
    pub message: String,
    pub input: Option<PathBuf>,
    pub render_output: Option<PathBuf>,
    pub outputs: JobOutputs,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOutputs {
    pub job_dir: PathBuf,
    pub audio_wav: PathBuf,
    pub segments_json: PathBuf,
    pub ja_srt: PathBuf,
    pub zh_srt: PathBuf,
    pub bilingual_srt: PathBuf,
    pub bilingual_ass: PathBuf,
}

impl JobManifest {
    pub fn new(job: &Job, input: Option<PathBuf>, render_output: Option<PathBuf>) -> Self {
        let now = unix_now();
        Self {
            job_id: job.id(),
            status: JobStatus::Created,
            message: JobStatus::Created.label().to_string(),
            input,
            render_output,
            outputs: JobOutputs::from(job),
            created_at_unix: now,
            updated_at_unix: now,
            error: None,
        }
    }

    pub fn mark(&mut self, status: JobStatus) {
        self.status = status;
        self.message = status.label().to_string();
        self.updated_at_unix = unix_now();
        if status != JobStatus::Failed {
            self.error = None;
        }
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = JobStatus::Failed;
        self.message = JobStatus::Failed.label().to_string();
        self.updated_at_unix = unix_now();
        self.error = Some(error.into());
    }
}

impl From<&Job> for JobOutputs {
    fn from(job: &Job) -> Self {
        Self {
            job_dir: job.dir.clone(),
            audio_wav: job.audio_wav.clone(),
            segments_json: job.segments_json.clone(),
            ja_srt: job.ja_srt.clone(),
            zh_srt: job.zh_srt.clone(),
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
