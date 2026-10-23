//! Read-only subtitle views. The workspace snapshot reads revision and cues
//! through one existing transaction, never through independent pool reads.
use anyhow::{Context, Result};

use super::{
    LocalDatabase, LocalSubtitleProvenanceRecord, LocalSubtitleSegmentRecord,
    LocalTranslationRunRecord, subtitle_edits::list_segments_in_transaction,
};

impl LocalDatabase {
    pub async fn list_segments(&self, job_id: &str) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             FROM local_subtitle_segments
             WHERE job_id = ?
             ORDER BY segment_index ASC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list local subtitle segments")
    }

    pub async fn get_segment(
        &self,
        job_id: &str,
        segment_id: &str,
    ) -> Result<Option<LocalSubtitleSegmentRecord>> {
        sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             FROM local_subtitle_segments
             WHERE job_id = ? AND id = ?",
        )
        .bind(job_id)
        .bind(segment_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local subtitle segment")
    }

    pub async fn list_subtitle_provenance(
        &self,
        job_id: &str,
    ) -> Result<Vec<LocalSubtitleProvenanceRecord>> {
        sqlx::query_as(
            "SELECT segment_id, job_id, source_kind, source_run_id, timed_unit_ids_json,
                    timing_source, alignment_status, source_revision, translation_revision,
                    translation_dependency_stale, updated_at_unix
             FROM local_subtitle_provenance
             WHERE job_id = ?
             ORDER BY segment_id",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("failed to list subtitle provenance for task {job_id}"))
    }

    pub async fn list_translation_runs(
        &self,
        job_id: &str,
    ) -> Result<Vec<LocalTranslationRunRecord>> {
        sqlx::query_as(
            "SELECT id, job_id, provider_id, provider_name, model, endpoint_kind,
                    grouping_strategy, options_json, plan_json,
                    segment_count, input_tokens, output_tokens, completed_at_unix
             FROM local_translation_runs
             WHERE job_id = ?
             ORDER BY completed_at_unix DESC, id DESC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list local translation runs")
    }

    /// Read the revision and subtitle rows together for conflict validation.
    pub async fn subtitle_workspace_snapshot(
        &self,
        job_id: &str,
    ) -> Result<(i64, Vec<LocalSubtitleSegmentRecord>)> {
        let mut tx = self.pool.begin().await?;
        let revision =
            sqlx::query_scalar("SELECT workspace_revision FROM local_jobs WHERE job_id = ?")
                .bind(job_id)
                .fetch_one(&mut *tx)
                .await?;
        let segments = list_segments_in_transaction(&mut tx, job_id).await?;
        tx.commit().await?;
        Ok((revision, segments))
    }
}
