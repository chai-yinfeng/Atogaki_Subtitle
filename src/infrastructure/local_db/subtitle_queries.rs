//! Read-only subtitle views. The workspace snapshot reads revision and cues
//! through one existing transaction, never through independent pool reads.
use anyhow::{Context, Result};

use super::{
    LocalDatabase, LocalSubtitleProvenanceRecord, LocalSubtitleSegmentRecord,
    LocalTranslationContextState, LocalTranslationRunRecord,
    subtitle_edits::list_segments_in_transaction,
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

    /// Used once after a committed text save, never as a full-task refresh.
    pub async fn translation_context_state(
        &self,
        job_id: &str,
    ) -> Result<LocalTranslationContextState> {
        let mut tx = self.pool.begin().await?;
        let workspace_revision =
            sqlx::query_scalar("SELECT workspace_revision FROM local_jobs WHERE job_id = ?")
                .bind(job_id)
                .fetch_one(&mut *tx)
                .await?;
        let translation_context_stale_ids = sqlx::query_scalar("SELECT segment_id FROM local_subtitle_provenance WHERE job_id = ? AND translation_dependency_stale = 1 ORDER BY segment_id")
            .bind(job_id).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(LocalTranslationContextState {
            workspace_revision,
            translation_context_stale_ids,
        })
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

#[cfg(test)]
mod context_state_tests {
    use super::*;
    use crate::{
        application::{
            LocalWorkspaceService, job_manifest::JobManifest, job_snapshot::JobSnapshot,
            job_status::JobStatus,
        },
        domain::TranscriptSegment,
        infrastructure::{
            job_store::Job,
            local_db::{LocalMachineTranslation, LocalTranslationDependency},
        },
    };
    #[tokio::test]
    async fn lightweight_context_state_tracks_propagation_clear_and_job_isolation() {
        let root =
            std::env::temp_dir().join(format!("atogaki-context-status-{}", uuid::Uuid::new_v4()));
        let db = LocalDatabase::open(root.join("fixture.sqlite"))
            .await
            .unwrap();
        let mut jobs = Vec::new();
        for _ in 0..2 {
            let job = Job::create_in(&root).unwrap();
            let mut manifest = JobManifest::new(&job, None, None, Default::default());
            manifest.mark(JobStatus::Done);
            db.sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![
                    TranscriptSegment::new(0, 1000, "context".into()),
                    TranscriptSegment::new(2000, 3000, "consumer".into()),
                ],
            })
            .await
            .unwrap();
            jobs.push(job.id());
        }
        let cues = db.list_segments(&jobs[0]).await.unwrap();
        db.apply_machine_translation_group(
            &jobs[0],
            &[LocalMachineTranslation {
                segment_id: cues[1].id.clone(),
                source_text: cues[1].source_text.clone(),
                expected_translated_text: None,
                expected_translation_edited: false,
                expected_source_revision: Some(0),
                expected_translation_revision: Some(0),
                translated_text: "machine result".into(),
            }],
            &[LocalTranslationDependency {
                segment_id: cues[0].id.clone(),
                source_text: cues[0].source_text.clone(),
                expected_source_revision: 0,
            }],
        )
        .await
        .unwrap();
        assert!(
            db.translation_context_state(&jobs[0])
                .await
                .unwrap()
                .translation_context_stale_ids
                .is_empty()
        );
        db.update_segment(
            &jobs[0],
            &cues[0].id,
            "changed context".into(),
            None,
            0,
            1000,
        )
        .await
        .unwrap();
        let service = LocalWorkspaceService::new(db.clone());
        let stale = service.translation_context_state(&jobs[0]).await.unwrap();
        assert_eq!(
            stale.translation_context_stale_ids,
            vec![cues[1].id.clone()]
        );
        assert_eq!(
            stale.workspace_revision,
            db.subtitle_workspace_snapshot(&jobs[0]).await.unwrap().0
        );
        assert!(
            !db.get_segment(&jobs[0], &cues[1].id)
                .await
                .unwrap()
                .unwrap()
                .translation_stale
        );
        assert!(
            service
                .translation_context_state(&jobs[1])
                .await
                .unwrap()
                .translation_context_stale_ids
                .is_empty()
        );
        assert!(service.translation_context_state("missing").await.is_err());
        let wire = serde_json::to_value(&stale).unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 2); // no segments, text, provenance or evidence in this response.
        db.update_segment(
            &jobs[0],
            &cues[1].id,
            cues[1].source_text.clone(),
            Some("manual revision".into()),
            2000,
            3000,
        )
        .await
        .unwrap();
        let cleared = service.translation_context_state(&jobs[0]).await.unwrap();
        assert!(cleared.translation_context_stale_ids.is_empty());
        assert!(cleared.workspace_revision > stale.workspace_revision);
        db.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}
