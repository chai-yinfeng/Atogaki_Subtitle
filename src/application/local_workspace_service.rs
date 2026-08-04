use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::infrastructure::local_db::{LocalDatabase, LocalJobRecord, LocalSubtitleSegmentRecord};

#[derive(Debug, Clone, Serialize)]
pub struct LocalWorkspaceJob {
    pub job: LocalJobRecord,
    pub segments: Vec<LocalSubtitleSegmentRecord>,
}

/// Application boundary for browsing and editing the durable desktop workspace.
///
/// Processing artifacts remain in each job directory, while subtitle edits are
/// written to SQLite so the UI never edits generated JSON files directly.
#[derive(Debug, Clone)]
pub struct LocalWorkspaceService {
    database: LocalDatabase,
}

impl LocalWorkspaceService {
    pub fn new(database: LocalDatabase) -> Self {
        Self { database }
    }

    pub async fn get_job(&self, job_id: &str) -> Result<LocalWorkspaceJob> {
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let segments = self.database.list_segments(job_id).await?;
        Ok(LocalWorkspaceJob { job, segments })
    }

    pub async fn update_subtitle_text(
        &self,
        job_id: &str,
        segment_id: &str,
        ja_text: String,
        zh_text: Option<String>,
    ) -> Result<LocalSubtitleSegmentRecord> {
        self.database
            .update_segment_text(job_id, segment_id, ja_text, zh_text)
            .await
    }
}
