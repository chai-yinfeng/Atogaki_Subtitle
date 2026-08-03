use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::Serialize;

use crate::{
    application::job_manifest::JobManifest, domain::TranscriptSegment,
    infrastructure::job_store::Job,
};

/// The complete read model needed by a desktop task list or subtitle editor.
///
/// A task may legitimately have no segments while it is queued or running.
#[derive(Debug, Clone, Serialize)]
pub struct JobSnapshot {
    pub manifest: JobManifest,
    pub segments: Vec<TranscriptSegment>,
}

impl JobSnapshot {
    pub fn load(job_dir: impl AsRef<Path>) -> Result<Self> {
        let job = Job::open(job_dir.as_ref().to_path_buf())?;
        let manifest = job
            .read_manifest_if_exists()?
            .ok_or_else(|| anyhow!("job status is missing: {}", job.status_json.display()))?;
        let segments = if job.segments_json.exists() {
            job.read_segments()
                .context("failed to load job subtitle segments")?
        } else {
            Vec::new()
        };

        Ok(Self { manifest, segments })
    }
}
