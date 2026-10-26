//! The shared application gate owns commit + receipt recording + undo ordering.
//! Only opaque descriptors leave the session; raw provenance snapshots stay here.
use super::{
    CandidateCueSet,
    structure_undo_history::{
        StructureHistoryRecord, StructureHistoryStats, StructureHistoryUndo,
        StructureUndoDescriptor, StructureUndoHistory,
    },
};
use crate::{
    domain::TranscriptSegment,
    infrastructure::{
        job_store::Job,
        local_db::{LocalDatabase, LocalStructureEdit, LocalSubtitleSegmentRecord},
    },
};
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize)]
pub struct StructureEditResult {
    pub segments: Vec<LocalSubtitleSegmentRecord>,
    pub workspace_revision: i64,
    pub translation_context_stale_ids: Vec<String>,
    pub undo: StructureHistoryRecord,
    /// Post-commit side effects must not hide an already committed edit or its token.
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct StructureEditSession {
    database: LocalDatabase,
    history: Arc<Mutex<StructureUndoHistory>>,
}
impl std::fmt::Debug for StructureEditSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructureEditSession")
            .finish_non_exhaustive()
    }
}
impl StructureEditSession {
    pub fn new(database: LocalDatabase) -> Self {
        Self {
            database,
            history: Arc::new(Mutex::new(StructureUndoHistory::default())),
        }
    }
    pub async fn latest(&self, job: &str) -> Option<StructureUndoDescriptor> {
        self.history.lock().await.latest(job)
    }
    pub async fn stats(&self) -> StructureHistoryStats {
        self.history.lock().await.stats()
    }
    pub async fn forget_job(&self, job: &str) -> usize {
        self.history.lock().await.forget_job(job)
    }
    pub async fn clear(&self) {
        self.history.lock().await.clear();
    }
    pub async fn undo(&self, job: &str, token: &str) -> Result<StructureHistoryUndo> {
        self.history
            .lock()
            .await
            .undo(&self.database, job, token)
            .await
    }
    pub async fn edit_block(
        &self,
        job: &str,
        before: &[LocalSubtitleSegmentRecord],
        remove_id: Option<&str>,
        insertion: Option<LocalSubtitleSegmentRecord>,
    ) -> Result<StructureEditResult> {
        let mut history = self.history.lock().await;
        validate_block_media(&self.database, job, insertion.as_ref()).await?;
        let label = if remove_id.is_some() {
            "删除字幕块"
        } else {
            "插入字幕块"
        };
        let edit = self
            .database
            .edit_subtitle_block_recorded(job, before, remove_id, insertion)
            .await?;
        Ok(record(&mut history, edit, label))
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn split(
        &self,
        job: &str,
        revision: i64,
        cue: &str,
        boundary: i64,
        left_source: String,
        right_source: String,
        left_translation: Option<String>,
        right_translation: Option<String>,
    ) -> Result<StructureEditResult> {
        let mut history = self.history.lock().await;
        let edit = self
            .database
            .split_segment_recorded(
                job,
                revision,
                cue,
                boundary,
                left_source,
                right_source,
                left_translation,
                right_translation,
            )
            .await?;
        Ok(record(&mut history, edit, "Cut"))
    }
    pub async fn merge(
        &self,
        job: &str,
        revision: i64,
        left: &str,
        right: &str,
        source: String,
        translation: Option<String>,
    ) -> Result<StructureEditResult> {
        let mut history = self.history.lock().await;
        let edit = self
            .database
            .merge_adjacent_segments_recorded(job, revision, left, right, source, translation)
            .await?;
        Ok(record(&mut history, edit, "Join"))
    }
    pub async fn save_timing(
        &self,
        job: &str,
        before: &[LocalSubtitleSegmentRecord],
        after: &[LocalSubtitleSegmentRecord],
    ) -> Result<StructureEditResult> {
        let mut history = self.history.lock().await;
        let edit = self
            .database
            .save_segment_timing_recorded(job, before, after)
            .await?;
        Ok(record(&mut history, edit, "调整时间"))
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn adopt(
        &self,
        job: &str,
        start: i64,
        end: i64,
        before: &[LocalSubtitleSegmentRecord],
        candidates: &[TranscriptSegment],
        run: &str,
        evidence: &CandidateCueSet,
        revision: i64,
    ) -> Result<StructureEditResult> {
        let mut history = self.history.lock().await;
        let edit = self
            .database
            .replace_segment_range_recorded(
                job,
                start,
                end,
                before,
                candidates,
                Some((run, evidence)),
                revision,
            )
            .await?;
        Ok(record(&mut history, edit, "采用识别候选"))
    }
}
fn record(
    history: &mut StructureUndoHistory,
    edit: LocalStructureEdit,
    label: &str,
) -> StructureEditResult {
    let segments = edit.segments().to_vec();
    let workspace_revision = edit.workspace_revision();
    let translation_context_stale_ids = edit.translation_context_stale_ids();
    let undo = history.record(edit, label.into());
    StructureEditResult {
        segments,
        workspace_revision,
        translation_context_stale_ids,
        undo,
        warnings: vec![],
    }
}

/// Shared with the legacy entry point until IPC switches to the recorded session.
pub(crate) async fn validate_block_media(
    database: &LocalDatabase,
    job: &str,
    insertion: Option<&LocalSubtitleSegmentRecord>,
) -> Result<()> {
    if let Some(block) = insertion {
        let record = database
            .get_job(job)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        let job = Job::open(record.storage_dir.into())?;
        let duration = super::acoustic_plan::wav_duration(&job.audio_wav)?;
        if block.end_ms < 0 || block.end_ms as u64 > duration {
            return Err(anyhow!("字幕范围超过媒体时长"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
