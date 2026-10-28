//! ASR run metadata and adoption/provenance protection reads.
//! Artifact IO and candidate adoption transactions remain in their existing services.
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::application::AsrRun;

use super::{LocalAsrRunRecord, LocalDatabase, to_i64};

impl LocalDatabase {
    pub async fn record_asr_run(&self, run: &AsrRun, artifact_dir: &Path) -> Result<()> {
        let config_json = serde_json::to_string(&run.config_snapshot)
            .context("failed to encode ASR configuration snapshot")?;
        sqlx::query(
            "INSERT INTO local_asr_runs (
                id, job_id, parent_run_id, run_kind, provider_id, provider_name,
                model_identity, input_media_path, scope_start_ms, scope_end_ms,
                config_json, artifact_dir, status, created_at_unix, started_at_unix,
                completed_at_unix, error_message
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.job_id)
        .bind(&run.parent_run_id)
        .bind(run.scope.kind.as_str())
        .bind(&run.provider_id)
        .bind(&run.provider_name)
        .bind(&run.model_identity)
        .bind(run.input_media.display().to_string())
        .bind(to_i64(run.scope.start_ms, "ASR scope start")?)
        .bind(
            run.scope
                .end_ms
                .map(|value| to_i64(value, "ASR scope end"))
                .transpose()?,
        )
        .bind(config_json)
        .bind(artifact_dir.display().to_string())
        .bind(run.status.as_str())
        .bind(to_i64(run.created_at_unix, "ASR run creation time")?)
        .bind(
            run.started_at_unix
                .map(|value| to_i64(value, "ASR run start time"))
                .transpose()?,
        )
        .bind(
            run.completed_at_unix
                .map(|value| to_i64(value, "ASR run completion time"))
                .transpose()?,
        )
        .bind(&run.error)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to record ASR run {}", run.id))?;
        Ok(())
    }

    pub async fn update_asr_run(&self, run: &AsrRun) -> Result<()> {
        let result = sqlx::query(
            "UPDATE local_asr_runs
             SET status = ?, started_at_unix = ?, completed_at_unix = ?, error_message = ?
             WHERE id = ? AND job_id = ?",
        )
        .bind(run.status.as_str())
        .bind(
            run.started_at_unix
                .map(|value| to_i64(value, "ASR run start time"))
                .transpose()?,
        )
        .bind(
            run.completed_at_unix
                .map(|value| to_i64(value, "ASR run completion time"))
                .transpose()?,
        )
        .bind(&run.error)
        .bind(&run.id)
        .bind(&run.job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to update ASR run {}", run.id))?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("ASR run does not exist: {}", run.id));
        }
        Ok(())
    }

    pub async fn asr_run_is_protected(&self, run_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT (EXISTS(SELECT 1 FROM local_asr_candidate_adoptions WHERE run_id = ?) OR EXISTS(SELECT 1 FROM local_subtitle_provenance WHERE source_run_id = ?))",
        )
        .bind(run_id)
        .bind(run_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn list_asr_runs(&self, job_id: &str) -> Result<Vec<LocalAsrRunRecord>> {
        sqlx::query_as(
            "SELECT id, job_id, parent_run_id, run_kind, provider_id, provider_name,
                    model_identity, input_media_path, scope_start_ms, scope_end_ms,
                    config_json, artifact_dir, status, created_at_unix, started_at_unix,
                    completed_at_unix, error_message
             FROM local_asr_runs
             WHERE job_id = ?
             ORDER BY created_at_unix DESC, id DESC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("failed to list ASR runs for task {job_id}"))
    }

    pub async fn recover_interrupted_asr_runs(&self, error: &str) -> Result<u64> {
        let now = chrono::Utc::now().timestamp();
        let result = sqlx::query(
            "UPDATE local_asr_runs
             SET status = 'failed', completed_at_unix = ?, error_message = ?
             WHERE status IN ('queued', 'running')",
        )
        .bind(now)
        .bind(error)
        .execute(&self.pool)
        .await
        .context("failed to recover interrupted ASR runs")?;
        Ok(result.rows_affected())
    }
}
