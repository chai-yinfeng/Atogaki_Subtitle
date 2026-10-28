mod asr_review;
#[cfg(test)]
mod structure_undo_scope_tests;
mod subtitle_edits;
mod subtitle_publication;
mod subtitle_queries;
mod subtitle_structure_undo;
pub use subtitle_structure_undo::{LocalStructureEdit, LocalStructureUndoResult};

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sqlx::{
    FromRow, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::{
    application::{AsrRun, job_snapshot::JobSnapshot},
    domain::subtitle::SubtitleStyleSet,
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

#[derive(Debug, Clone)]
pub struct LocalDatabase {
    pool: SqlitePool,
}

/// Lightweight, consistent context status; no subtitle text or ASR evidence.
#[derive(Debug, Clone, Serialize)]
pub struct LocalTranslationContextState {
    pub workspace_revision: i64,
    pub translation_context_stale_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalJobRecord {
    pub job_id: String,
    pub display_name: Option<String>,
    pub storage_dir: String,
    pub input_path: Option<String>,
    pub render_output_path: Option<String>,
    pub status: String,
    pub message: String,
    pub error_message: Option<String>,
    pub glossary_id: Option<String>,
    pub glossary_name: Option<String>,
    pub glossary_snapshot_path: Option<String>,
    pub source_language: String,
    pub target_language: String,
    pub created_at_unix: i64,
    pub started_at_unix: Option<i64>,
    pub completed_at_unix: Option<i64>,
    pub updated_at_unix: i64,
    pub sort_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalJobTranslationStats {
    pub job_id: String,
    pub segment_count: i64,
    pub translated_segment_count: i64,
    pub stale_translation_count: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalGlossaryRecord {
    pub id: String,
    pub name: String,
    pub source_language: String,
    pub builtin_key: Option<String>,
    pub builtin_version: Option<String>,
    pub term_count: i64,
    pub prompt_term_count: i64,
    pub correction_count: i64,
    pub core_term_count: i64,
    pub content_term_count: i64,
    pub correction_only_count: i64,
    pub content_group_count: i64,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalGlossaryTermRecord {
    pub id: String,
    pub glossary_id: String,
    pub source_text: String,
    pub target_text: Option<String>,
    pub prompt_scope: String,
    pub content_group: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalGlossaryDetail {
    pub glossary: LocalGlossaryRecord,
    pub terms: Vec<LocalGlossaryTermRecord>,
}

#[derive(Debug, Clone)]
pub struct LocalGlossaryTermInput {
    pub source_text: String,
    pub target_text: Option<String>,
    pub prompt_scope: String,
    pub content_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalSubtitleSegmentRecord {
    pub id: String,
    pub job_id: String,
    pub segment_index: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub source_text: String,
    pub translated_text: Option<String>,
    pub source_edited: bool,
    pub translation_edited: bool,
    pub translation_stale: bool,
    pub timing_edited: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalSubtitleProvenanceRecord {
    pub segment_id: String,
    pub job_id: String,
    pub source_kind: String,
    pub source_run_id: Option<String>,
    pub timed_unit_ids_json: String,
    pub timing_source: String,
    pub alignment_status: String,
    pub source_revision: i64,
    pub translation_revision: i64,
    pub translation_dependency_stale: bool,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalLearningItemRecord {
    pub id: String,
    pub source_language: String,
    pub target_language: String,
    pub item_type: String,
    pub source_text: String,
    pub meaning_text: Option<String>,
    pub meaning_provider_id: Option<String>,
    pub meaning_source_label: Option<String>,
    pub occurrence_count: i64,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalLearningOccurrenceRecord {
    pub id: String,
    pub learning_item_id: String,
    pub job_id: Option<String>,
    pub segment_id: Option<String>,
    pub job_display_name_snapshot: String,
    pub segment_source_snapshot: String,
    pub segment_translation_snapshot: Option<String>,
    pub selection_start_utf16: i64,
    pub selection_end_utf16: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalLearningLookupSense {
    pub part_of_speech: Option<String>,
    pub definitions: Vec<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalLearningLookupResult {
    pub id: String,
    pub learning_item_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub headword: String,
    pub reading: Option<String>,
    pub pronunciation: Option<String>,
    pub audio_url: Option<String>,
    pub senses: Vec<LocalLearningLookupSense>,
    pub attribution_text: String,
    pub source_url: Option<String>,
    pub license_label: Option<String>,
    pub data_version: Option<String>,
    pub fetched_at_unix: i64,
    pub cache_expires_at_unix: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct LocalLearningLookupRow {
    id: String,
    learning_item_id: String,
    provider_id: String,
    provider_name: String,
    headword: String,
    reading: Option<String>,
    pronunciation: Option<String>,
    audio_url: Option<String>,
    senses_json: String,
    attribution_text: String,
    source_url: Option<String>,
    license_label: Option<String>,
    data_version: Option<String>,
    fetched_at_unix: i64,
    cache_expires_at_unix: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewLocalLearningLookupResult {
    pub learning_item_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub headword: String,
    pub reading: Option<String>,
    pub pronunciation: Option<String>,
    pub audio_url: Option<String>,
    pub senses: Vec<LocalLearningLookupSense>,
    pub attribution_text: String,
    pub source_url: Option<String>,
    pub license_label: Option<String>,
    pub data_version: Option<String>,
    pub fetched_at_unix: i64,
    pub cache_expires_at_unix: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalLearningItemDetail {
    pub item: LocalLearningItemRecord,
    pub occurrences: Vec<LocalLearningOccurrenceRecord>,
    pub lookup_results: Vec<LocalLearningLookupResult>,
}

#[derive(Debug, Clone)]
pub struct NewLocalLearningSelection {
    pub job_id: String,
    pub segment_id: String,
    pub item_type: String,
    pub selection_start_utf16: i64,
    pub selection_end_utf16: i64,
}

#[derive(Debug, FromRow)]
struct LocalLearningSourceRow {
    job_id: String,
    display_name: Option<String>,
    input_path: Option<String>,
    source_language: String,
    target_language: String,
    segment_id: String,
    start_ms: i64,
    end_ms: i64,
    source_text: String,
    translated_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LocalMachineTranslation {
    pub expected_source_revision: Option<i64>,
    pub expected_translation_revision: Option<i64>,
    pub segment_id: String,
    pub source_text: String,
    pub expected_translated_text: Option<String>,
    pub expected_translation_edited: bool,
    pub translated_text: String,
}

#[derive(Debug, Clone)]
pub struct LocalTranslationDependency {
    pub segment_id: String,
    pub source_text: String,
    pub expected_source_revision: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalTranslationRunRecord {
    pub id: String,
    pub job_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: Option<String>,
    pub endpoint_kind: String,
    pub grouping_strategy: Option<String>,
    pub options_json: Option<String>,
    pub plan_json: Option<String>,
    pub segment_count: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub completed_at_unix: i64,
}

#[derive(Debug, Clone)]
pub struct NewLocalTranslationRun {
    pub id: String,
    pub job_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: Option<String>,
    pub endpoint_kind: String,
    pub grouping_strategy: Option<String>,
    pub options_json: Option<String>,
    pub plan_json: Option<String>,
    pub segment_count: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalAsrRunRecord {
    pub id: String,
    pub job_id: String,
    pub parent_run_id: Option<String>,
    pub run_kind: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model_identity: String,
    pub input_media_path: String,
    pub scope_start_ms: i64,
    pub scope_end_ms: Option<i64>,
    pub config_json: String,
    pub artifact_dir: String,
    pub status: String,
    pub created_at_unix: i64,
    pub started_at_unix: Option<i64>,
    pub completed_at_unix: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalAsrQualitySignalRecord {
    pub id: String,
    pub job_id: String,
    pub run_id: String,
    pub signal_kind: String,
    pub severity: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub cue_ids_json: String,
    pub detector_version: String,
    pub message: String,
    pub evidence_json: String,
    pub created_at_unix: i64,
    pub latest_decision: Option<String>,
    pub latest_decision_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalAsrRepairAttemptRecord {
    pub id: String,
    pub signal_id: String,
    pub candidate_run_id: String,
    pub status: String,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalRenderJobRecord {
    pub id: String,
    pub source_job_id: String,
    pub input_path: String,
    pub subtitle_path: String,
    pub output_path: String,
    pub subtitle_track: String,
    pub status: String,
    pub progress: f64,
    pub encoder: Option<String>,
    pub audio_encoder: Option<String>,
    pub fallback_reason: Option<String>,
    pub error_message: Option<String>,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone)]
pub struct NewLocalRenderJob {
    pub id: String,
    pub source_job_id: String,
    pub input_path: String,
    pub subtitle_path: String,
    pub output_path: String,
    pub subtitle_track: String,
}

#[derive(Debug, Clone)]
pub struct LocalGlossaryCorrection {
    pub segment_id: String,
    pub source_text: String,
    pub corrected_text: String,
}

impl LocalDatabase {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let parent = path.parent().ok_or_else(|| {
            anyhow!(
                "local database path must have a parent directory: {}",
                path.display()
            )
        })?;
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create local data directory {}", parent.display())
        })?;

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("failed to open local SQLite database")?;
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await
            .context("failed to enable SQLite WAL mode")?;
        MIGRATOR
            .run(&pool)
            .await
            .context("failed to run local SQLite migrations")?;

        Ok(Self { pool })
    }

    /// Close every connection shared by this database handle and its clones.
    ///
    /// Dropping the final SQLite pool handle eventually closes its connections,
    /// but callers that need to move or remove the database file must wait for
    /// an explicit close on platforms such as Windows that lock open files.
    pub async fn acoustic_checkpoint_ranges(&self, job_id: &str) -> Result<Vec<(i64, i64)>> {
        Ok(sqlx::query_as("SELECT start_ms,end_ms FROM local_acoustic_checkpoints WHERE job_id=? ORDER BY chunk_index").bind(job_id).fetch_all(&self.pool).await?)
    }

    pub async fn record_acoustic_checkpoint(
        &self,
        job_id: &str,
        run_id: &str,
        chunk: &crate::application::acoustic_plan::AcousticChunk,
        path: &Path,
        device: &str,
    ) -> Result<()> {
        sqlx::query("INSERT INTO local_acoustic_checkpoints (job_id, chunk_index, run_id, start_ms, end_ms, artifact_path, device) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(job_id, chunk_index) DO UPDATE SET run_id=excluded.run_id, artifact_path=excluded.artifact_path, device=excluded.device")
            .bind(job_id).bind(chunk.index as i64).bind(run_id).bind(chunk.start_ms as i64).bind(chunk.end_ms as i64).bind(path.display().to_string()).bind(device).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT value FROM local_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("failed to read local setting {key}"))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        if key.trim().is_empty() {
            return Err(anyhow!("local setting key cannot be empty"));
        }
        sqlx::query(
            "INSERT INTO local_settings (key, value, updated_at_unix)
             VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at_unix = excluded.updated_at_unix",
        )
        .bind(key)
        .bind(value)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to save local setting {key}"))?;
        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM local_settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .with_context(|| format!("failed to delete local setting {key}"))?;
        Ok(())
    }

    pub async fn get_subtitle_styles(&self, job_id: &str) -> Result<SubtitleStyleSet> {
        self.get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let styles_json: Option<String> =
            sqlx::query_scalar("SELECT styles_json FROM local_subtitle_styles WHERE job_id = ?")
                .bind(job_id)
                .fetch_optional(&self.pool)
                .await
                .with_context(|| format!("failed to read subtitle styles for task {job_id}"))?;
        let styles: SubtitleStyleSet = styles_json
            .map(|json| serde_json::from_str(&json).context("invalid stored subtitle styles"))
            .transpose()?
            .unwrap_or_default();
        styles.validate()?;
        Ok(styles)
    }

    pub async fn save_subtitle_styles(
        &self,
        job_id: &str,
        styles: &SubtitleStyleSet,
    ) -> Result<SubtitleStyleSet> {
        self.get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        styles.validate()?;
        let styles_json =
            serde_json::to_string(styles).context("failed to encode subtitle styles")?;
        sqlx::query(
            "INSERT INTO local_subtitle_styles (job_id, styles_json, updated_at_unix)
             VALUES (?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
                styles_json = excluded.styles_json,
                updated_at_unix = excluded.updated_at_unix",
        )
        .bind(job_id)
        .bind(styles_json)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to save subtitle styles for task {job_id}"))?;
        Ok(styles.clone())
    }

    pub async fn mark_job_failed(&self, job_id: &str, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE local_jobs
             SET status = 'failed', message = '任务已中断', error_message = ?,
                 completed_at_unix = COALESCE(completed_at_unix, ?), updated_at_unix = ?
             WHERE job_id = ? AND status NOT IN ('done', 'failed')",
        )
        .bind(error)
        .bind(chrono::Utc::now().timestamp())
        .bind(chrono::Utc::now().timestamp())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark local task {job_id} as failed"))?;
        Ok(())
    }

    pub async fn sync_snapshot(&self, snapshot: &JobSnapshot) -> Result<()> {
        let manifest = &snapshot.manifest;
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local task transaction")?;

        sqlx::query(
            "INSERT INTO local_jobs (
                job_id, storage_dir, input_path, render_output_path, status, message,
                error_message, source_language, target_language,
                created_at_unix, started_at_unix, completed_at_unix, updated_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
                storage_dir = excluded.storage_dir,
                input_path = excluded.input_path,
                render_output_path = excluded.render_output_path,
                status = excluded.status,
                message = excluded.message,
                error_message = excluded.error_message,
                source_language = excluded.source_language,
                target_language = excluded.target_language,
                started_at_unix = COALESCE(local_jobs.started_at_unix, excluded.started_at_unix),
                completed_at_unix = excluded.completed_at_unix,
                updated_at_unix = MAX(local_jobs.updated_at_unix, excluded.updated_at_unix)",
        )
        .bind(&manifest.job_id)
        .bind(manifest.outputs.job_dir.display().to_string())
        .bind(
            manifest
                .input
                .as_ref()
                .map(|path| path.display().to_string()),
        )
        .bind(
            manifest
                .render_output
                .as_ref()
                .map(|path| path.display().to_string()),
        )
        .bind(manifest.status.as_str())
        .bind(&manifest.message)
        .bind(&manifest.error)
        .bind(manifest.source_language.as_str())
        .bind(manifest.target_language.as_str())
        .bind(to_i64(manifest.created_at_unix, "created_at_unix")?)
        .bind(
            manifest
                .started_at_unix
                .map(|value| to_i64(value, "started_at_unix"))
                .transpose()?,
        )
        .bind(
            manifest
                .completed_at_unix
                .map(|value| to_i64(value, "completed_at_unix"))
                .transpose()?,
        )
        .bind(to_i64(manifest.updated_at_unix, "updated_at_unix")?)
        .execute(&mut *tx)
        .await
        .context("failed to upsert local task")?;

        let structure_edited = sqlx::query_scalar::<_, bool>(
            "SELECT subtitle_structure_edited FROM local_jobs WHERE job_id = ?",
        )
        .bind(&manifest.job_id)
        .fetch_one(&mut *tx)
        .await
        .context("failed to read local subtitle structure state")?;
        if !snapshot.segments.is_empty() && !structure_edited {
            self.merge_snapshot_segments_in_transaction(
                &mut tx,
                &manifest.job_id,
                &snapshot.segments,
            )
            .await?;
        }
        tx.commit()
            .await
            .context("failed to commit local task transaction")
    }

    pub async fn list_jobs(&self) -> Result<Vec<LocalJobRecord>> {
        sqlx::query_as::<_, LocalJobRecord>(
            "SELECT job_id, display_name, storage_dir, input_path, render_output_path, status, message,
                error_message, glossary_id, glossary_name, glossary_snapshot_path,
                source_language, target_language,
                created_at_unix, started_at_unix, completed_at_unix, updated_at_unix,
                sort_position
             FROM local_jobs
             ORDER BY sort_position IS NOT NULL, sort_position ASC,
                updated_at_unix DESC, job_id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list local tasks")
    }

    pub async fn list_job_translation_stats(&self) -> Result<Vec<LocalJobTranslationStats>> {
        sqlx::query_as::<_, LocalJobTranslationStats>(
            "SELECT j.job_id,
                COUNT(s.id) AS segment_count,
                COALESCE(SUM(CASE
                    WHEN s.translated_text IS NOT NULL AND trim(s.translated_text) != '' THEN 1
                    ELSE 0
                END), 0) AS translated_segment_count,
                COALESCE(SUM(CASE WHEN s.translation_stale = 1 THEN 1 ELSE 0 END), 0)
                    AS stale_translation_count
             FROM local_jobs j
             LEFT JOIN local_subtitle_segments s ON s.job_id = j.job_id
             GROUP BY j.job_id",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to summarize local task translations")
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<LocalJobRecord>> {
        sqlx::query_as::<_, LocalJobRecord>(
            "SELECT job_id, display_name, storage_dir, input_path, render_output_path, status, message,
                error_message, glossary_id, glossary_name, glossary_snapshot_path,
                source_language, target_language,
                created_at_unix, started_at_unix, completed_at_unix, updated_at_unix,
                sort_position
             FROM local_jobs
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local task")
    }

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

    pub async fn reorder_jobs(&self, job_ids: &[String]) -> Result<Vec<LocalJobRecord>> {
        let requested = job_ids.iter().cloned().collect::<HashSet<_>>();
        if requested.len() != job_ids.len() {
            return Err(anyhow!("task order contains duplicate ids"));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin task reorder transaction")?;
        let existing = sqlx::query_scalar::<_, String>("SELECT job_id FROM local_jobs")
            .fetch_all(&mut *tx)
            .await
            .context("failed to read tasks before reordering")?;
        if existing.len() != job_ids.len()
            || existing.iter().any(|job_id| !requested.contains(job_id))
        {
            return Err(anyhow!(
                "task list changed while reordering; refresh and try again"
            ));
        }
        for (position, job_id) in job_ids.iter().enumerate() {
            sqlx::query("UPDATE local_jobs SET sort_position = ? WHERE job_id = ?")
                .bind(i64::try_from(position).context("task order exceeds SQLite i64")?)
                .bind(job_id)
                .execute(&mut *tx)
                .await
                .with_context(|| format!("failed to reorder local task {job_id}"))?;
        }
        tx.commit()
            .await
            .context("failed to commit task reorder transaction")?;
        self.list_jobs().await
    }

    pub async fn update_job_input_path(
        &self,
        job_id: &str,
        input_path: &Path,
        updated_at_unix: i64,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE local_jobs
             SET input_path = ?, updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(input_path.display().to_string())
        .bind(updated_at_unix)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to update source media for local task {job_id}"))?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("local task not found: {job_id}"));
        }
        Ok(())
    }

    pub async fn create_render_job(
        &self,
        render: NewLocalRenderJob,
    ) -> Result<LocalRenderJobRecord> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO local_render_jobs (
                id, source_job_id, input_path, subtitle_path, output_path,
                subtitle_track, status, progress, created_at_unix, updated_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, 'queued', 0, ?, ?)",
        )
        .bind(&render.id)
        .bind(&render.source_job_id)
        .bind(&render.input_path)
        .bind(&render.subtitle_path)
        .bind(&render.output_path)
        .bind(&render.subtitle_track)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("failed to create local video render")?;
        self.get_render_job(&render.id)
            .await?
            .ok_or_else(|| anyhow!("created local video render disappeared: {}", render.id))
    }

    pub async fn list_render_jobs(&self, source_job_id: &str) -> Result<Vec<LocalRenderJobRecord>> {
        sqlx::query_as::<_, LocalRenderJobRecord>(
            "SELECT id, source_job_id, input_path, subtitle_path, output_path,
                subtitle_track, status, progress, encoder, audio_encoder, fallback_reason,
                error_message,
                created_at_unix, updated_at_unix
             FROM local_render_jobs
             WHERE source_job_id = ?
             ORDER BY created_at_unix DESC, id DESC",
        )
        .bind(source_job_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list local video renders")
    }

    pub async fn get_render_job(&self, render_id: &str) -> Result<Option<LocalRenderJobRecord>> {
        sqlx::query_as::<_, LocalRenderJobRecord>(
            "SELECT id, source_job_id, input_path, subtitle_path, output_path,
                subtitle_track, status, progress, encoder, audio_encoder, fallback_reason,
                error_message,
                created_at_unix, updated_at_unix
             FROM local_render_jobs
             WHERE id = ?",
        )
        .bind(render_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local video render")
    }

    pub async fn mark_render_running(&self, render_id: &str) -> Result<()> {
        self.update_render_state(render_id, "running", None, None, None, None)
            .await
    }

    pub async fn update_render_progress(&self, render_id: &str, progress: f64) -> Result<()> {
        let progress = progress.clamp(0.0, 0.99);
        let result = sqlx::query(
            "UPDATE local_render_jobs
             SET progress = MAX(progress, ?), updated_at_unix = ?
             WHERE id = ? AND status = 'running'",
        )
        .bind(progress)
        .bind(chrono::Utc::now().timestamp())
        .bind(render_id)
        .execute(&self.pool)
        .await
        .context("failed to update local video render progress")?;
        if result.rows_affected() > 1 {
            return Err(anyhow!("updated multiple local video renders: {render_id}"));
        }
        Ok(())
    }

    pub async fn finish_render(
        &self,
        render_id: &str,
        encoder: &str,
        audio_encoder: &str,
        fallback_reason: Option<&str>,
    ) -> Result<()> {
        self.update_render_state(
            render_id,
            "done",
            Some(encoder),
            Some(audio_encoder),
            fallback_reason,
            None,
        )
        .await
    }

    pub async fn fail_render(&self, render_id: &str, error: &str) -> Result<()> {
        self.update_render_state(render_id, "failed", None, None, None, Some(error))
            .await
    }

    pub async fn cancel_render(&self, render_id: &str) -> Result<()> {
        self.update_render_state(render_id, "cancelled", None, None, None, None)
            .await
    }

    pub async fn interrupt_unfinished_renders(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE local_render_jobs
             SET status = 'failed', error_message = 'Atogaki exited before this render completed',
                 updated_at_unix = ?
             WHERE status IN ('queued', 'running')",
        )
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("failed to recover interrupted local video renders")?;
        Ok(result.rows_affected())
    }

    async fn update_render_state(
        &self,
        render_id: &str,
        status: &str,
        encoder: Option<&str>,
        audio_encoder: Option<&str>,
        fallback_reason: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE local_render_jobs
             SET status = ?, progress = CASE WHEN ? = 'done' THEN 1 ELSE progress END,
                 encoder = COALESCE(?, encoder),
                 audio_encoder = COALESCE(?, audio_encoder),
                 fallback_reason = COALESCE(?, fallback_reason), error_message = ?,
                 updated_at_unix = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(status)
        .bind(encoder)
        .bind(audio_encoder)
        .bind(fallback_reason)
        .bind(error_message)
        .bind(chrono::Utc::now().timestamp())
        .bind(render_id)
        .execute(&self.pool)
        .await
        .context("failed to update local video render state")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("local video render not found: {render_id}"));
        }
        Ok(())
    }

    pub async fn rename_job(
        &self,
        job_id: &str,
        display_name: Option<String>,
    ) -> Result<LocalJobRecord> {
        let display_name = display_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
        if display_name
            .as_ref()
            .is_some_and(|name| name.chars().count() > 100)
        {
            return Err(anyhow!("task name cannot exceed 100 characters"));
        }
        if display_name
            .as_ref()
            .is_some_and(|name| name.chars().any(char::is_control))
        {
            return Err(anyhow!("task name cannot contain control characters"));
        }
        let result = sqlx::query(
            "UPDATE local_jobs
             SET display_name = ?, updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(display_name)
        .bind(chrono::Utc::now().timestamp())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .context("failed to rename local task")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("local task not found: {job_id}"));
        }
        self.get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("renamed local task disappeared: {job_id}"))
    }

    pub async fn save_learning_selection(
        &self,
        input: NewLocalLearningSelection,
    ) -> Result<LocalLearningItemDetail> {
        if !matches!(input.item_type.as_str(), "selection" | "sentence") {
            return Err(anyhow!(
                "unsupported learning item type: {}",
                input.item_type
            ));
        }
        let start = usize::try_from(input.selection_start_utf16)
            .context("learning selection start must be non-negative")?;
        let end = usize::try_from(input.selection_end_utf16)
            .context("learning selection end must be non-negative")?;
        if end <= start {
            return Err(anyhow!("learning selection cannot be empty"));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin learning item transaction")?;
        let source = sqlx::query_as::<_, LocalLearningSourceRow>(
            "SELECT j.job_id, j.display_name, j.input_path,
                    j.source_language, j.target_language,
                    s.id AS segment_id, s.start_ms, s.end_ms,
                    s.source_text, s.translated_text
             FROM local_subtitle_segments s
             JOIN local_jobs j ON j.job_id = s.job_id
             WHERE j.job_id = ? AND s.id = ?",
        )
        .bind(&input.job_id)
        .bind(&input.segment_id)
        .fetch_optional(&mut *tx)
        .await
        .context("failed to read learning item source")?
        .ok_or_else(|| {
            anyhow!(
                "subtitle segment {} does not belong to task {}",
                input.segment_id,
                input.job_id
            )
        })?;

        let selected_text = utf16_slice(&source.source_text, start, end)?;
        let selected_text = selected_text.trim().to_string();
        if selected_text.is_empty() {
            return Err(anyhow!("learning selection cannot contain only whitespace"));
        }
        if input.item_type == "sentence"
            && (start != 0 || end != source.source_text.encode_utf16().count())
        {
            return Err(anyhow!(
                "sentence collections must include the complete subtitle segment"
            ));
        }
        let source_key = selected_text.to_lowercase();
        let meaning_text = (input.item_type == "sentence")
            .then(|| source.translated_text.clone())
            .flatten()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let now = chrono::Utc::now().timestamp();
        let item_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO local_learning_items (
                id, source_language, target_language, item_type, source_text, source_key,
                meaning_text, meaning_provider_id, meaning_source_label,
                created_at_unix, updated_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
             ON CONFLICT(source_language, target_language, item_type, source_key) DO UPDATE SET
                meaning_text = COALESCE(local_learning_items.meaning_text, excluded.meaning_text),
                updated_at_unix = excluded.updated_at_unix",
        )
        .bind(&item_id)
        .bind(&source.source_language)
        .bind(&source.target_language)
        .bind(&input.item_type)
        .bind(&selected_text)
        .bind(&source_key)
        .bind(meaning_text)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .context("failed to save learning item")?;
        let item_id: String = sqlx::query_scalar(
            "SELECT id FROM local_learning_items
             WHERE source_language = ? AND target_language = ?
               AND item_type = ? AND source_key = ?",
        )
        .bind(&source.source_language)
        .bind(&source.target_language)
        .bind(&input.item_type)
        .bind(&source_key)
        .fetch_one(&mut *tx)
        .await
        .context("failed to read saved learning item id")?;

        let display_name = source
            .display_name
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                source.input_path.as_deref().and_then(|value| {
                    Path::new(value)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
            })
            .unwrap_or_else(|| source.job_id.clone());
        sqlx::query(
            "INSERT OR IGNORE INTO local_learning_occurrences (
                id, learning_item_id, job_id, segment_id, job_display_name_snapshot,
                segment_source_snapshot, segment_translation_snapshot,
                selection_start_utf16, selection_end_utf16, start_ms, end_ms, created_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&item_id)
        .bind(&source.job_id)
        .bind(&source.segment_id)
        .bind(display_name)
        .bind(&source.source_text)
        .bind(&source.translated_text)
        .bind(input.selection_start_utf16)
        .bind(input.selection_end_utf16)
        .bind(source.start_ms)
        .bind(source.end_ms)
        .bind(now)
        .execute(&mut *tx)
        .await
        .context("failed to save learning item source")?;
        tx.commit()
            .await
            .context("failed to commit learning item transaction")?;

        self.get_learning_item(&item_id)
            .await?
            .ok_or_else(|| anyhow!("saved learning item disappeared: {item_id}"))
    }

    pub async fn list_learning_items(&self) -> Result<Vec<LocalLearningItemDetail>> {
        sqlx::query(
            "DELETE FROM local_learning_lookup_results
             WHERE cache_expires_at_unix IS NOT NULL AND cache_expires_at_unix <= ?",
        )
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("failed to expire learning dictionary results")?;
        let items = sqlx::query_as::<_, LocalLearningItemRecord>(
            "SELECT i.id, i.source_language, i.target_language, i.item_type,
                    i.source_text, i.meaning_text, i.meaning_provider_id,
                    i.meaning_source_label, COUNT(o.id) AS occurrence_count,
                    i.created_at_unix, i.updated_at_unix
             FROM local_learning_items i
             LEFT JOIN local_learning_occurrences o ON o.learning_item_id = i.id
             GROUP BY i.id
             ORDER BY i.updated_at_unix DESC, i.id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list learning items")?;
        let occurrences = sqlx::query_as::<_, LocalLearningOccurrenceRecord>(
            "SELECT id, learning_item_id, job_id, segment_id, job_display_name_snapshot,
                    segment_source_snapshot, segment_translation_snapshot,
                    selection_start_utf16, selection_end_utf16,
                    start_ms, end_ms, created_at_unix
             FROM local_learning_occurrences
             ORDER BY created_at_unix DESC, id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list learning item sources")?;
        let mut occurrences_by_item = HashMap::<String, Vec<_>>::new();
        for occurrence in occurrences {
            occurrences_by_item
                .entry(occurrence.learning_item_id.clone())
                .or_default()
                .push(occurrence);
        }
        let lookup_rows = sqlx::query_as::<_, LocalLearningLookupRow>(
            "SELECT id, learning_item_id, provider_id, provider_name, headword,
                    reading, pronunciation, audio_url, senses_json, attribution_text, source_url,
                    license_label, data_version, fetched_at_unix, cache_expires_at_unix
             FROM local_learning_lookup_results
             ORDER BY provider_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list learning dictionary results")?;
        let mut lookups_by_item = HashMap::<String, Vec<_>>::new();
        for row in lookup_rows {
            let learning_item_id = row.learning_item_id.clone();
            lookups_by_item
                .entry(learning_item_id)
                .or_default()
                .push(learning_lookup_result_from_row(row)?);
        }
        Ok(items
            .into_iter()
            .map(|item| LocalLearningItemDetail {
                occurrences: occurrences_by_item.remove(&item.id).unwrap_or_default(),
                lookup_results: lookups_by_item.remove(&item.id).unwrap_or_default(),
                item,
            })
            .collect())
    }

    pub async fn upsert_learning_lookup_result(
        &self,
        input: NewLocalLearningLookupResult,
    ) -> Result<LocalLearningItemDetail> {
        let provider_id = input.provider_id.trim();
        let provider_name = input.provider_name.trim();
        let headword = input.headword.trim();
        let attribution_text = input.attribution_text.trim();
        if provider_id.is_empty()
            || provider_name.is_empty()
            || headword.is_empty()
            || attribution_text.is_empty()
        {
            return Err(anyhow!(
                "learning dictionary results require provider, headword, and attribution"
            ));
        }
        if input.senses.is_empty()
            || input.senses.iter().any(|sense| {
                sense
                    .definitions
                    .iter()
                    .all(|value| value.trim().is_empty())
            })
        {
            return Err(anyhow!(
                "learning dictionary results require at least one definition per sense"
            ));
        }
        let senses_json = serde_json::to_string(&input.senses)
            .context("failed to encode learning dictionary senses")?;
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO local_learning_lookup_results (
                id, learning_item_id, provider_id, provider_name, headword,
                reading, pronunciation, audio_url, senses_json, attribution_text, source_url,
                license_label, data_version, fetched_at_unix, cache_expires_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(learning_item_id, provider_id) DO UPDATE SET
                provider_name = excluded.provider_name,
                headword = excluded.headword,
                reading = excluded.reading,
                pronunciation = excluded.pronunciation,
                audio_url = excluded.audio_url,
                senses_json = excluded.senses_json,
                attribution_text = excluded.attribution_text,
                source_url = excluded.source_url,
                license_label = excluded.license_label,
                data_version = excluded.data_version,
                fetched_at_unix = excluded.fetched_at_unix,
                cache_expires_at_unix = excluded.cache_expires_at_unix",
        )
        .bind(id)
        .bind(&input.learning_item_id)
        .bind(provider_id)
        .bind(provider_name)
        .bind(headword)
        .bind(
            input
                .reading
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(
            input
                .pronunciation
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(
            input
                .audio_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(senses_json)
        .bind(attribution_text)
        .bind(input.source_url)
        .bind(input.license_label)
        .bind(input.data_version)
        .bind(input.fetched_at_unix)
        .bind(input.cache_expires_at_unix)
        .execute(&self.pool)
        .await
        .context("failed to save learning dictionary result")?;

        self.get_learning_item(&input.learning_item_id)
            .await?
            .ok_or_else(|| anyhow!("learning item not found: {}", input.learning_item_id))
    }

    pub async fn get_learning_item(
        &self,
        item_id: &str,
    ) -> Result<Option<LocalLearningItemDetail>> {
        Ok(self
            .list_learning_items()
            .await?
            .into_iter()
            .find(|detail| detail.item.id == item_id))
    }

    pub async fn update_learning_item_meaning(
        &self,
        item_id: &str,
        meaning_text: Option<String>,
    ) -> Result<LocalLearningItemDetail> {
        let meaning_text = meaning_text
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let result = sqlx::query(
            "UPDATE local_learning_items
             SET meaning_text = ?, meaning_provider_id = NULL, meaning_source_label = NULL,
                 updated_at_unix = ?
             WHERE id = ?",
        )
        .bind(meaning_text)
        .bind(chrono::Utc::now().timestamp())
        .bind(item_id)
        .execute(&self.pool)
        .await
        .context("failed to update learning item meaning")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("learning item not found: {item_id}"));
        }
        self.get_learning_item(item_id)
            .await?
            .ok_or_else(|| anyhow!("updated learning item disappeared: {item_id}"))
    }

    pub async fn update_learning_item_meaning_from_provider(
        &self,
        item_id: &str,
        meaning_text: &str,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<LocalLearningItemDetail> {
        let meaning_text = meaning_text.trim();
        if meaning_text.is_empty()
            || provider_id.trim().is_empty()
            || provider_name.trim().is_empty()
        {
            return Err(anyhow!(
                "dictionary-backed learning meaning requires definition and provider"
            ));
        }
        let result = sqlx::query(
            "UPDATE local_learning_items
             SET meaning_text = ?, meaning_provider_id = ?, meaning_source_label = ?,
                 updated_at_unix = ?
             WHERE id = ?",
        )
        .bind(meaning_text)
        .bind(provider_id.trim())
        .bind(provider_name.trim())
        .bind(chrono::Utc::now().timestamp())
        .bind(item_id)
        .execute(&self.pool)
        .await
        .context("failed to use dictionary definition as learning meaning")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("learning item not found: {item_id}"));
        }
        self.get_learning_item(item_id)
            .await?
            .ok_or_else(|| anyhow!("updated learning item disappeared: {item_id}"))
    }

    pub async fn delete_learning_item(&self, item_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM local_learning_items WHERE id = ?")
            .bind(item_id)
            .execute(&self.pool)
            .await
            .context("failed to delete learning item")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("learning item not found: {item_id}"));
        }
        Ok(())
    }

    pub async fn delete_job(&self, job_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM local_jobs WHERE job_id = ?")
            .bind(job_id)
            .execute(&self.pool)
            .await
            .context("failed to delete local task")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("local task not found: {job_id}"));
        }
        sqlx::query("DELETE FROM local_settings WHERE key = ?")
            .bind(format!("listening.playback_position_ms.{job_id}"))
            .execute(&self.pool)
            .await
            .context("failed to delete local task playback position")?;
        Ok(())
    }

    pub async fn assign_job_glossary(
        &self,
        job_id: &str,
        glossary_id: &str,
        glossary_name: &str,
        snapshot_path: &Path,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE local_jobs
             SET glossary_id = ?, glossary_name = ?, glossary_snapshot_path = ?
             WHERE job_id = ?",
        )
        .bind(glossary_id)
        .bind(glossary_name)
        .bind(snapshot_path.display().to_string())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .context("failed to associate glossary with local task")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("local task not found: {job_id}"));
        }
        Ok(())
    }

    pub async fn ensure_builtin_glossary(
        &self,
        name: &str,
        builtin_key: &str,
        builtin_version: &str,
        source_language: &str,
        terms: Vec<LocalGlossaryTermInput>,
    ) -> Result<()> {
        let name = normalized_glossary_name(name)?;
        let builtin_key = builtin_key.trim();
        let builtin_version = builtin_version.trim();
        if builtin_key.is_empty() || builtin_version.is_empty() {
            return Err(anyhow!("built-in glossary key and version cannot be empty"));
        }
        let mut seen = HashSet::new();
        let mut terms = terms;
        terms.retain(|term| {
            seen.insert((
                term.source_text.clone(),
                term.target_text.clone(),
                term.prompt_scope.clone(),
                term.content_group.clone(),
            ))
        });
        let terms = normalized_glossary_terms(terms)?;
        let mut existing = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT id, builtin_version FROM local_glossaries WHERE builtin_key = ?",
        )
        .bind(builtin_key)
        .fetch_optional(&self.pool)
        .await
        .context("failed to find versioned built-in glossary")?;
        if existing.is_none() {
            let legacy = sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT id, builtin_key FROM local_glossaries WHERE name = ?",
            )
            .bind(&name)
            .fetch_optional(&self.pool)
            .await
            .context("failed to find legacy built-in glossary")?;
            if let Some((legacy_id, None)) = legacy {
                sqlx::query(
                    "UPDATE local_glossaries
                     SET builtin_key = ?, builtin_version = NULL
                     WHERE id = ?",
                )
                .bind(builtin_key)
                .bind(&legacy_id)
                .execute(&self.pool)
                .await
                .context("failed to adopt legacy glossary as versioned built-in")?;
                existing = Some((legacy_id, None));
            }
        }
        if existing
            .as_ref()
            .and_then(|(_, version)| version.as_deref())
            == Some(builtin_version)
        {
            return Ok(());
        }
        let glossary_id = existing
            .map(|(id, _)| id)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO local_glossaries
                (id, name, source_language, builtin_key, builtin_version, created_at_unix, updated_at_unix)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                source_language = excluded.source_language,
                builtin_version = excluded.builtin_version,
                updated_at_unix = excluded.updated_at_unix",
        )
        .bind(&glossary_id)
        .bind(&name)
        .bind(source_language)
        .bind(builtin_key)
        .bind(builtin_version)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .context("failed to save versioned built-in glossary")?;
        sqlx::query("DELETE FROM local_glossary_terms WHERE glossary_id = ?")
            .bind(&glossary_id)
            .execute(&mut *tx)
            .await?;
        for term in terms {
            sqlx::query(
                "INSERT INTO local_glossary_terms
                    (id, glossary_id, source_text, target_text, prompt_scope, content_group)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&glossary_id)
            .bind(term.source_text)
            .bind(term.target_text)
            .bind(term.prompt_scope)
            .bind(term.content_group)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit()
            .await
            .context("failed to update built-in glossary")?;
        Ok(())
    }

    pub async fn list_glossaries(&self) -> Result<Vec<LocalGlossaryRecord>> {
        sqlx::query_as::<_, LocalGlossaryRecord>(
            "SELECT g.id, g.name, g.source_language, g.builtin_key, g.builtin_version, COUNT(t.id) AS term_count,
                COALESCE(SUM(CASE WHEN t.id IS NOT NULL AND t.prompt_scope != 'correction_only' THEN 1 ELSE 0 END), 0) AS prompt_term_count,
                COALESCE(SUM(CASE WHEN t.target_text IS NOT NULL THEN 1 ELSE 0 END), 0) AS correction_count,
                COALESCE(SUM(CASE WHEN t.prompt_scope = 'core' THEN 1 ELSE 0 END), 0) AS core_term_count,
                COALESCE(SUM(CASE WHEN t.prompt_scope = 'content' THEN 1 ELSE 0 END), 0) AS content_term_count,
                COALESCE(SUM(CASE WHEN t.prompt_scope = 'correction_only' THEN 1 ELSE 0 END), 0) AS correction_only_count,
                COUNT(DISTINCT CASE WHEN t.prompt_scope = 'content' THEN t.content_group END) AS content_group_count,
                g.created_at_unix, g.updated_at_unix
             FROM local_glossaries g
             LEFT JOIN local_glossary_terms t ON t.glossary_id = g.id
             GROUP BY g.id
             ORDER BY lower(g.name), g.id",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list local glossaries")
    }

    pub async fn get_glossary(&self, glossary_id: &str) -> Result<Option<LocalGlossaryDetail>> {
        let glossary = sqlx::query_as::<_, LocalGlossaryRecord>(
            "SELECT g.id, g.name, g.source_language, g.builtin_key, g.builtin_version, COUNT(t.id) AS term_count,
                COALESCE(SUM(CASE WHEN t.id IS NOT NULL AND t.prompt_scope != 'correction_only' THEN 1 ELSE 0 END), 0) AS prompt_term_count,
                COALESCE(SUM(CASE WHEN t.target_text IS NOT NULL THEN 1 ELSE 0 END), 0) AS correction_count,
                COALESCE(SUM(CASE WHEN t.prompt_scope = 'core' THEN 1 ELSE 0 END), 0) AS core_term_count,
                COALESCE(SUM(CASE WHEN t.prompt_scope = 'content' THEN 1 ELSE 0 END), 0) AS content_term_count,
                COALESCE(SUM(CASE WHEN t.prompt_scope = 'correction_only' THEN 1 ELSE 0 END), 0) AS correction_only_count,
                COUNT(DISTINCT CASE WHEN t.prompt_scope = 'content' THEN t.content_group END) AS content_group_count,
                g.created_at_unix, g.updated_at_unix
             FROM local_glossaries g
             LEFT JOIN local_glossary_terms t ON t.glossary_id = g.id
             WHERE g.id = ?
             GROUP BY g.id",
        )
        .bind(glossary_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local glossary")?;
        let Some(glossary) = glossary else {
            return Ok(None);
        };
        let terms = sqlx::query_as::<_, LocalGlossaryTermRecord>(
            "SELECT id, glossary_id, source_text, target_text, prompt_scope, content_group
             FROM local_glossary_terms
             WHERE glossary_id = ?
             ORDER BY CASE prompt_scope WHEN 'core' THEN 0 WHEN 'content' THEN 1 ELSE 2 END,
                lower(COALESCE(content_group, '')), lower(source_text), id",
        )
        .bind(glossary_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to read local glossary terms")?;
        Ok(Some(LocalGlossaryDetail { glossary, terms }))
    }

    pub async fn save_glossary(
        &self,
        glossary_id: Option<&str>,
        name: String,
        source_language: &str,
        terms: Vec<LocalGlossaryTermInput>,
    ) -> Result<LocalGlossaryDetail> {
        let mut name = normalized_glossary_name(&name)?;
        let terms = normalized_glossary_terms(terms)?;
        let builtin_name = if let Some(glossary_id) = glossary_id {
            sqlx::query_scalar::<_, String>(
                "SELECT name FROM local_glossaries WHERE id = ? AND builtin_key IS NOT NULL",
            )
            .bind(glossary_id)
            .fetch_optional(&self.pool)
            .await?
        } else {
            None
        };
        if let Some(builtin_name) = &builtin_name {
            let preferred = if name == *builtin_name {
                let base = name.strip_suffix("（内置）").unwrap_or(&name);
                format!("{base}（自定义）")
            } else {
                name.clone()
            };
            name = self.available_glossary_name(&preferred).await?;
        }
        let glossary_id = builtin_name
            .is_none()
            .then_some(glossary_id)
            .flatten()
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let now = chrono::Utc::now().timestamp();
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local glossary transaction")?;

        if glossary_id.is_empty() {
            return Err(anyhow!("glossary id cannot be empty"));
        }
        let result = sqlx::query(
            "INSERT INTO local_glossaries
                (id, name, source_language, created_at_unix, updated_at_unix)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                source_language = excluded.source_language,
                updated_at_unix = excluded.updated_at_unix",
        )
        .bind(&glossary_id)
        .bind(&name)
        .bind(source_language)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("failed to save glossary {name}"))?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("failed to save glossary {name}"));
        }

        sqlx::query("DELETE FROM local_glossary_terms WHERE glossary_id = ?")
            .bind(&glossary_id)
            .execute(&mut *tx)
            .await
            .context("failed to replace local glossary terms")?;
        for term in terms {
            sqlx::query(
                "INSERT INTO local_glossary_terms
                    (id, glossary_id, source_text, target_text, prompt_scope, content_group)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&glossary_id)
            .bind(term.source_text)
            .bind(term.target_text)
            .bind(term.prompt_scope)
            .bind(term.content_group)
            .execute(&mut *tx)
            .await
            .context("failed to insert local glossary term")?;
        }
        tx.commit()
            .await
            .context("failed to commit local glossary")?;
        self.get_glossary(&glossary_id)
            .await?
            .ok_or_else(|| anyhow!("saved glossary disappeared: {glossary_id}"))
    }

    pub async fn delete_glossary(&self, glossary_id: &str) -> Result<()> {
        if sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM local_glossaries WHERE id = ? AND builtin_key IS NOT NULL",
        )
        .bind(glossary_id)
        .fetch_one(&self.pool)
        .await?
            > 0
        {
            return Err(anyhow!("built-in glossaries cannot be deleted"));
        }
        let result = sqlx::query("DELETE FROM local_glossaries WHERE id = ?")
            .bind(glossary_id)
            .execute(&self.pool)
            .await
            .context("failed to delete local glossary")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("local glossary not found: {glossary_id}"));
        }
        Ok(())
    }

    async fn available_glossary_name(&self, preferred: &str) -> Result<String> {
        for suffix in 1..=10_000 {
            let candidate = if suffix == 1 {
                preferred.to_string()
            } else {
                format!("{preferred} {suffix}")
            };
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM local_glossaries WHERE name = ?",
            )
            .bind(&candidate)
            .fetch_one(&self.pool)
            .await?
                > 0;
            if !exists {
                return Ok(candidate);
            }
        }
        Err(anyhow!("could not allocate a custom glossary name"))
    }

    pub async fn apply_machine_translations(
        &self,
        job_id: &str,
        translations: &[LocalMachineTranslation],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.apply_machine_translation_group(job_id, translations, &[])
            .await
    }

    pub async fn apply_machine_translation_group(
        &self,
        job_id: &str,
        translations: &[LocalMachineTranslation],
        dependencies: &[LocalTranslationDependency],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        if translations.is_empty() {
            return self.list_segments(job_id).await;
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local translation transaction")?;

        let mut dependency_stale = false;
        let mut dependency_revisions = HashMap::new();
        for dependency in dependencies {
            let current = sqlx::query_as::<_, (String, i64)>(
                "SELECT s.source_text, p.source_revision
                 FROM local_subtitle_segments s
                 JOIN local_subtitle_provenance p ON p.segment_id = s.id
                 WHERE s.job_id = ? AND s.id = ?",
            )
            .bind(job_id)
            .bind(&dependency.segment_id)
            .fetch_optional(&mut *tx)
            .await
            .context("failed to verify translation context revision")?;
            match current {
                Some((source_text, revision)) => {
                    dependency_stale |= source_text != dependency.source_text
                        || revision != dependency.expected_source_revision;
                    dependency_revisions.insert(dependency.segment_id.as_str(), revision);
                }
                None => dependency_stale = true,
            }
        }

        for translation in translations {
            if translation.expected_source_revision.is_some()
                || translation.expected_translation_revision.is_some()
            {
                let current: (i64, i64) = sqlx::query_as("SELECT source_revision, translation_revision FROM local_subtitle_provenance WHERE job_id = ? AND segment_id = ?")
                    .bind(job_id).bind(&translation.segment_id).fetch_one(&mut *tx).await?;
                if translation
                    .expected_source_revision
                    .is_some_and(|revision| revision != current.0)
                    || translation
                        .expected_translation_revision
                        .is_some_and(|revision| revision != current.1)
                {
                    return Err(anyhow!(
                        "subtitle revision changed while it was being translated: {}",
                        translation.segment_id
                    ));
                }
            }
            let translated_text = translation.translated_text.trim();
            if translated_text.is_empty() {
                return Err(anyhow!(
                    "machine translation cannot be empty for segment {}",
                    translation.segment_id
                ));
            }
            let result = sqlx::query(
                "UPDATE local_subtitle_segments
                 SET translated_text = ?, translation_edited = 0, translation_stale = 0
                 WHERE job_id = ? AND id = ? AND source_text = ?
                   AND translated_text IS ? AND translation_edited = ?",
            )
            .bind(translated_text)
            .bind(job_id)
            .bind(&translation.segment_id)
            .bind(&translation.source_text)
            .bind(&translation.expected_translated_text)
            .bind(translation.expected_translation_edited)
            .execute(&mut *tx)
            .await
            .context("failed to store local machine translation")?;
            if result.rows_affected() != 1 {
                return Err(anyhow!(
                    "subtitle changed while it was being translated: {}",
                    translation.segment_id
                ));
            }

            // An identical translation does not fire the translated_text trigger.
            // Always replace its context snapshot, including removed dependencies.
            sqlx::query(
                "DELETE FROM local_translation_dependency_edges WHERE target_segment_id = ?",
            )
            .bind(&translation.segment_id)
            .execute(&mut *tx)
            .await
            .context("failed to replace translation context dependencies")?;

            for (context_segment_id, context_source_revision) in &dependency_revisions {
                if *context_segment_id == translation.segment_id {
                    continue;
                }
                sqlx::query(
                    "INSERT INTO local_translation_dependency_edges (
                        target_segment_id, context_segment_id, context_source_revision
                     ) VALUES (?, ?, ?)",
                )
                .bind(&translation.segment_id)
                .bind(context_segment_id)
                .bind(context_source_revision)
                .execute(&mut *tx)
                .await
                .context("failed to record translation context dependency")?;
            }
            sqlx::query(
                "UPDATE local_subtitle_provenance
                 SET translation_dependency_stale = ?,
                     updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
                 WHERE segment_id = ?",
            )
            .bind(dependency_stale)
            .bind(&translation.segment_id)
            .execute(&mut *tx)
            .await
            .context("failed to update translation dependency state")?;
        }

        sqlx::query(
            "UPDATE local_jobs
             SET updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(chrono::Utc::now().timestamp())
        .bind(job_id)
        .execute(&mut *tx)
        .await
        .context("failed to update local task translation timestamp")?;
        tx.commit()
            .await
            .context("failed to commit local translation transaction")?;
        self.list_segments(job_id).await
    }

    pub async fn record_translation_run(&self, run: &NewLocalTranslationRun) -> Result<()> {
        sqlx::query(
            "INSERT INTO local_translation_runs (
                id, job_id, provider_id, provider_name, model, endpoint_kind,
                grouping_strategy, options_json, plan_json,
                segment_count, input_tokens, output_tokens, completed_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.job_id)
        .bind(&run.provider_id)
        .bind(&run.provider_name)
        .bind(&run.model)
        .bind(&run.endpoint_kind)
        .bind(&run.grouping_strategy)
        .bind(&run.options_json)
        .bind(&run.plan_json)
        .bind(run.segment_count)
        .bind(run.input_tokens)
        .bind(run.output_tokens)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("failed to record local translation run")?;
        Ok(())
    }

    pub async fn apply_glossary_corrections(
        &self,
        job_id: &str,
        corrections: &[LocalGlossaryCorrection],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        if corrections.is_empty() {
            return self.list_segments(job_id).await;
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local glossary application transaction")?;

        for correction in corrections {
            let corrected_text = correction.corrected_text.trim();
            if corrected_text.is_empty() {
                return Err(anyhow!(
                    "glossary correction cannot empty segment {}",
                    correction.segment_id
                ));
            }
            let result = sqlx::query(
                "UPDATE local_subtitle_segments
                 SET source_text = ?, source_edited = 1,
                     translation_stale = CASE WHEN translated_text IS NULL THEN translation_stale ELSE 1 END
                 WHERE job_id = ? AND id = ? AND source_text = ?",
            )
            .bind(corrected_text)
            .bind(job_id)
            .bind(&correction.segment_id)
            .bind(&correction.source_text)
            .execute(&mut *tx)
            .await
            .context("failed to apply local glossary correction")?;
            if result.rows_affected() != 1 {
                return Err(anyhow!(
                    "subtitle changed while glossary preview was open: {}",
                    correction.segment_id
                ));
            }
        }

        sqlx::query(
            "UPDATE local_jobs
             SET updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(chrono::Utc::now().timestamp())
        .bind(job_id)
        .execute(&mut *tx)
        .await
        .context("failed to update local task glossary timestamp")?;
        tx.commit()
            .await
            .context("failed to commit local glossary application")?;
        self.list_segments(job_id).await
    }
}

fn utf16_slice(value: &str, start: usize, end: usize) -> Result<String> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    let selected = units
        .get(start..end)
        .ok_or_else(|| anyhow!("learning selection is outside the subtitle text"))?;
    String::from_utf16(selected)
        .map_err(|_| anyhow!("learning selection splits a Unicode character"))
}

fn learning_lookup_result_from_row(
    row: LocalLearningLookupRow,
) -> Result<LocalLearningLookupResult> {
    let senses = serde_json::from_str(&row.senses_json)
        .with_context(|| format!("failed to decode {} dictionary senses", row.provider_id))?;
    Ok(LocalLearningLookupResult {
        id: row.id,
        learning_item_id: row.learning_item_id,
        provider_id: row.provider_id,
        provider_name: row.provider_name,
        headword: row.headword,
        reading: row.reading,
        pronunciation: row.pronunciation,
        audio_url: row.audio_url,
        senses,
        attribution_text: row.attribution_text,
        source_url: row.source_url,
        license_label: row.license_label,
        data_version: row.data_version,
        fetched_at_unix: row.fetched_at_unix,
        cache_expires_at_unix: row.cache_expires_at_unix,
    })
}

fn to_i64(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| anyhow!("{field} exceeds SQLite i64"))
}

fn normalized_glossary_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("glossary name cannot be empty"));
    }
    if name.chars().count() > 80 {
        return Err(anyhow!("glossary name cannot exceed 80 characters"));
    }
    Ok(name.to_string())
}

fn normalized_glossary_terms(
    terms: Vec<LocalGlossaryTermInput>,
) -> Result<Vec<LocalGlossaryTermInput>> {
    let mut seen = HashSet::new();
    let mut terms = terms
        .into_iter()
        .map(|term| {
            let source_text = term.source_text.trim().to_string();
            if source_text.is_empty() {
                return Err(anyhow!("glossary source text cannot be empty"));
            }
            let target_text = term
                .target_text
                .map(|target| target.trim().to_string())
                .filter(|target| !target.is_empty());
            if target_text.as_deref() == Some(source_text.as_str()) {
                return Err(anyhow!(
                    "glossary replacement source and target cannot be identical: {source_text}"
                ));
            }
            let prompt_scope = term.prompt_scope.trim().to_string();
            if !matches!(
                prompt_scope.as_str(),
                "core" | "content" | "correction_only"
            ) {
                return Err(anyhow!("invalid glossary prompt scope: {prompt_scope}"));
            }
            let content_group = term
                .content_group
                .map(|group| group.trim().to_string())
                .filter(|group| !group.is_empty());
            if prompt_scope == "content" && content_group.is_none() {
                return Err(anyhow!("content glossary terms require a content group"));
            }
            if prompt_scope == "correction_only" && target_text.is_none() {
                return Err(anyhow!(
                    "correction-only glossary terms require a canonical target"
                ));
            }
            if !seen.insert((
                source_text.clone(),
                target_text.clone(),
                prompt_scope.clone(),
                content_group.clone(),
            )) {
                return Err(anyhow!("duplicate glossary term: {source_text}"));
            }
            Ok(LocalGlossaryTermInput {
                source_text,
                target_text,
                content_group: (prompt_scope == "content")
                    .then_some(content_group)
                    .flatten(),
                prompt_scope,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let correction_targets = terms
        .iter()
        // A prompted correction already includes its canonical target in Whisper's
        // prompt. Correction-only rules do not, so they must not suppress an
        // explicit core/content spelling of that target.
        .filter(|term| term.prompt_scope != "correction_only")
        .filter_map(|term| term.target_text.clone())
        .collect::<HashSet<_>>();
    terms.retain(|term| {
        term.target_text.is_some() || !correction_targets.contains(&term.source_text)
    });
    Ok(terms)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        LocalDatabase, LocalGlossaryTermInput, LocalLearningLookupSense, LocalMachineTranslation,
        LocalTranslationDependency, NewLocalLearningLookupResult, NewLocalLearningSelection,
        NewLocalRenderJob,
    };
    use crate::{
        application::{
            AsrInputScope, AsrRun, AsrRunStatus, job_manifest::JobManifest,
            job_snapshot::JobSnapshot, job_status::JobStatus,
        },
        domain::TranscriptSegment,
        infrastructure::job_store::Job,
    };

    #[tokio::test]
    async fn checkpoint_migration_preserves_existing_manual_work_and_learning_sources() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-checkpoint-migration-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let path = root.join("legacy.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let legacy = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(
                super::MIGRATOR
                    .iter()
                    .filter(|m| m.version < 202610180001)
                    .cloned()
                    .collect(),
            ),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        legacy.run(&pool).await.unwrap();
        let old = LocalDatabase { pool };
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        job.write_manifest(&manifest).unwrap();
        let segment = TranscriptSegment::new(1000, 2500, "元の文".into());
        old.sync_snapshot(&JobSnapshot {
            manifest,
            segments: vec![segment.clone()],
        })
        .await
        .unwrap();
        let mut run = AsrRun::new(
            job.id(),
            None,
            "whisper.cpp",
            "Whisper.cpp",
            "legacy-model",
            job.audio_wav.clone(),
            AsrInputScope::full(),
            serde_json::json!({}),
        );
        run.succeed();
        old.record_asr_run(&run, &job.asr_runs_dir.join(&run.id))
            .await
            .unwrap();
        let candidates = crate::application::CandidateCueSet {
            schema_version: 1,
            segmentation_policy: "legacy-v1".into(),
            cues: vec![crate::application::CandidateCue {
                cue: segment.clone(),
                timed_unit_ids: vec!["legacy-unit".into()],
            }],
        };
        old.assign_asr_cue_provenance(&job.id(), &run.id, &candidates)
            .await
            .unwrap();
        assert_eq!(
            old.list_subtitle_provenance(&job.id()).await.unwrap()[0].source_kind,
            "asr"
        );
        crate::application::LocalWorkspaceService::new(old.clone())
            .update_subtitle(
                &job.id(),
                &segment.id,
                "人工修正".into(),
                Some("人工译文".into()),
                1000,
                2500,
            )
            .await
            .unwrap();
        let before = old.list_segments(&job.id()).await.unwrap();
        let saved = old
            .save_learning_selection(NewLocalLearningSelection {
                job_id: job.id(),
                segment_id: segment.id.clone(),
                item_type: "sentence".into(),
                selection_start_utf16: 0,
                selection_end_utf16: 4,
            })
            .await
            .unwrap();
        old.close().await;
        let upgraded = LocalDatabase::open(&path).await.unwrap();
        upgraded
            .assign_asr_cue_provenance(&job.id(), &run.id, &candidates)
            .await
            .unwrap();
        let provenance = upgraded.list_subtitle_provenance(&job.id()).await.unwrap();
        assert_eq!(provenance[0].source_kind, "manual");
        assert_eq!(provenance[0].alignment_status, "stale");
        assert_eq!(upgraded.list_segments(&job.id()).await.unwrap(), before);
        assert!(before[0].source_edited && before[0].translation_edited);
        assert_eq!(
            upgraded
                .acoustic_checkpoint_ranges(&job.id())
                .await
                .unwrap(),
            vec![]
        );
        assert!(
            !crate::application::acoustic_plan::recovery_summary(&job)
                .unwrap()
                .can_resume
        );
        let learning = upgraded
            .get_learning_item(&saved.item.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            learning.occurrences[0].segment_id.as_deref(),
            Some(segment.id.as_str())
        );
        assert_eq!(learning.occurrences[0].segment_source_snapshot, "人工修正");
        assert_eq!(
            learning.occurrences[0]
                .segment_translation_snapshot
                .as_deref(),
            Some("人工译文")
        );
        let exported = crate::application::local_learning_service::anki_tsv(
            &upgraded.list_learning_items().await.unwrap(),
            "ja",
        );
        assert!(exported.contains("人工修正") && exported.contains("人工译文"));
        upgraded.close().await;
        let repeated = LocalDatabase::open(&path).await.unwrap();
        assert_eq!(repeated.list_segments(&job.id()).await.unwrap(), before);
        assert_eq!(repeated.list_learning_items().await.unwrap().len(), 1);
        repeated.close().await;
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn indexes_asr_run_lifecycle_and_recovers_interrupted_runs() {
        let root =
            std::env::temp_dir().join(format!("atogaki-asr-run-db-test-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: Vec::new(),
            })
            .await
            .unwrap();
        let mut run = AsrRun::new(
            job.id(),
            None,
            "whisper.cpp",
            "Whisper.cpp",
            "model.bin",
            "video.mp4".into(),
            AsrInputScope::full(),
            serde_json::json!({"max_context": 0}),
        );
        database
            .record_asr_run(&run, &job.asr_runs_dir.join(&run.id))
            .await
            .unwrap();
        run.start();
        database.update_asr_run(&run).await.unwrap();
        assert_eq!(
            database.list_asr_runs(&job.id()).await.unwrap()[0].status,
            "running"
        );

        assert_eq!(
            database
                .recover_interrupted_asr_runs("application stopped")
                .await
                .unwrap(),
            1
        );
        let recovered = database.list_asr_runs(&job.id()).await.unwrap();
        assert_eq!(recovered[0].status, AsrRunStatus::Failed.as_str());
        assert_eq!(
            recovered[0].error_message.as_deref(),
            Some("application stopped")
        );

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn stores_quality_reviews_and_repair_attempts_as_run_metadata() {
        use crate::application::{
            QualitySignal, QualitySignalKind, QualitySignalSeverity, RepairAttempt,
            RepairAttemptStatus, ReviewDecision, ReviewDecisionKind,
        };

        let root = std::env::temp_dir().join(format!(
            "atogaki-asr-quality-db-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: Vec::new(),
            })
            .await
            .unwrap();
        let run = AsrRun::new(
            job.id(),
            None,
            "whisper.cpp",
            "Whisper.cpp",
            "model.bin",
            "video.mp4".into(),
            AsrInputScope::full(),
            serde_json::json!({}),
        );
        database
            .record_asr_run(&run, &job.asr_runs_dir.join(&run.id))
            .await
            .unwrap();
        let signal = QualitySignal {
            schema_version: 1,
            id: "signal-1".into(),
            job_id: job.id(),
            run_id: run.id.clone(),
            kind: QualitySignalKind::RepeatedLoop,
            severity: QualitySignalSeverity::Critical,
            start_ms: Some(100),
            end_ms: Some(300),
            cue_ids: vec!["cue-1".into()],
            detector_version: "test".into(),
            message: "loop".into(),
            evidence: serde_json::json!({"count": 2}),
            created_at_unix: 10,
        };
        database
            .replace_asr_quality_signals(&run.id, std::slice::from_ref(&signal))
            .await
            .unwrap();
        database
            .record_asr_review_decision(&ReviewDecision {
                id: "decision-1".into(),
                signal_id: signal.id.clone(),
                decision: ReviewDecisionKind::ConfirmedProblem,
                note: Some("heard repetition".into()),
                created_at_unix: 11,
            })
            .await
            .unwrap();
        database
            .record_asr_repair_attempt(&RepairAttempt {
                id: "repair-1".into(),
                signal_id: signal.id.clone(),
                candidate_run_id: run.id.clone(),
                status: RepairAttemptStatus::Candidate,
                created_at_unix: 12,
                updated_at_unix: 12,
            })
            .await
            .unwrap();

        let signals = database.list_asr_quality_signals(&job.id()).await.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(
            signals[0].latest_decision.as_deref(),
            Some("confirmed_problem")
        );
        assert_eq!(
            signals[0].latest_decision_note.as_deref(),
            Some("heard repetition")
        );
        database
            .record_asr_review_decision(&ReviewDecision {
                id: "a-later-decision".into(),
                signal_id: signal.id.clone(),
                decision: ReviewDecisionKind::FalsePositive,
                note: None,
                created_at_unix: 11,
            })
            .await
            .unwrap();
        assert_eq!(
            database.list_asr_quality_signals(&job.id()).await.unwrap()[0]
                .latest_decision
                .as_deref(),
            Some("false_positive")
        );
        assert_eq!(
            database.list_asr_repair_attempts(&signal.id).await.unwrap()[0].status,
            "candidate"
        );

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn language_migration_preserves_legacy_rows_and_track_meanings() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE local_jobs (
                job_id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                created_at_unix INTEGER NOT NULL,
                updated_at_unix INTEGER NOT NULL
             );
             CREATE TABLE local_subtitle_segments (
                id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
                ja_text TEXT NOT NULL,
                zh_text TEXT
             );
             CREATE TABLE local_glossaries (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at_unix INTEGER NOT NULL,
                updated_at_unix INTEGER NOT NULL
             );
             CREATE TABLE local_render_jobs (
                id TEXT PRIMARY KEY,
                source_job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
                input_path TEXT NOT NULL,
                subtitle_path TEXT NOT NULL,
                output_path TEXT NOT NULL,
                subtitle_track TEXT NOT NULL,
                status TEXT NOT NULL,
                progress REAL NOT NULL DEFAULT 0,
                encoder TEXT,
                audio_encoder TEXT,
                error_message TEXT,
                created_at_unix INTEGER NOT NULL,
                updated_at_unix INTEGER NOT NULL,
                fallback_reason TEXT
             );
             CREATE INDEX local_render_jobs_source_idx
                ON local_render_jobs (source_job_id, created_at_unix DESC);
             CREATE INDEX local_render_jobs_status_idx
                ON local_render_jobs (status, updated_at_unix DESC);
             INSERT INTO local_jobs VALUES ('legacy-job', 'done', 1, 2);
             INSERT INTO local_subtitle_segments
                VALUES ('legacy-segment', 'legacy-job', 'こんばんは', '晚上好');
             INSERT INTO local_glossaries VALUES ('legacy-glossary', '旧词表', 1, 2);
             INSERT INTO local_render_jobs VALUES (
                'legacy-render', 'legacy-job', 'in.mov', 'sub.ass', 'out.mp4',
                'japanese', 'done', 1, 'videotoolbox', 'aac', NULL, 1, 2, NULL
             );
             INSERT INTO local_render_jobs VALUES (
                'legacy-render-translation', 'legacy-job', 'in.mov', 'sub.ass', 'out-zh.mp4',
                'chinese', 'done', 1, 'videotoolbox', 'aac', NULL, 1, 3, NULL
             );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/sqlite/202608090001_add_task_languages.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        let job: (String, String) = sqlx::query_as(
            "SELECT source_language, target_language FROM local_jobs WHERE job_id = 'legacy-job'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(job, ("ja".to_string(), "zh-Hans".to_string()));
        let segment: (String, Option<String>) = sqlx::query_as(
            "SELECT source_text, translated_text FROM local_subtitle_segments
             WHERE id = 'legacy-segment'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            segment,
            ("こんばんは".to_string(), Some("晚上好".to_string()))
        );
        let glossary_language: String = sqlx::query_scalar(
            "SELECT source_language FROM local_glossaries WHERE id = 'legacy-glossary'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(glossary_language, "ja");
        let track: String = sqlx::query_scalar(
            "SELECT subtitle_track FROM local_render_jobs WHERE id = 'legacy-render'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(track, "source");
        let translated_track: String = sqlx::query_scalar(
            "SELECT subtitle_track FROM local_render_jobs
             WHERE id = 'legacy-render-translation'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(translated_track, "translation");
    }

    #[tokio::test]
    async fn syncs_task_metadata_and_subtitle_segments() {
        let root =
            std::env::temp_dir().join(format!("atogaki-local-db-test-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Queued);
        let mut segment = TranscriptSegment::new(0, 1_000, "テスト".to_string());
        segment.set_translation(Some("测试".to_string()));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();

        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![segment.clone()],
            })
            .await
            .unwrap();

        let jobs = database.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, manifest.job_id);
        assert_eq!(jobs[0].status, "queued");

        let translation_stats = database.list_job_translation_stats().await.unwrap();
        assert_eq!(translation_stats.len(), 1);
        assert_eq!(translation_stats[0].segment_count, 1);
        assert_eq!(translation_stats[0].translated_segment_count, 1);
        assert_eq!(translation_stats[0].stale_translation_count, 0);

        let segments = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, segment.id);
        assert_eq!(segments[0].source_text, "テスト");

        let default_styles = database
            .get_subtitle_styles(&manifest.job_id)
            .await
            .unwrap();
        let mut custom_styles = default_styles.clone();
        custom_styles.source.font_family = "Arial".to_string();
        custom_styles.source.font_size = 54.0;
        database
            .save_subtitle_styles(&manifest.job_id, &custom_styles)
            .await
            .unwrap();
        assert_eq!(
            database
                .get_subtitle_styles(&manifest.job_id)
                .await
                .unwrap(),
            custom_styles
        );

        let stale = database
            .update_segment(
                &manifest.job_id,
                &segment.id,
                "手動修正".to_string(),
                Some("测试".to_string()),
                100,
                900,
            )
            .await
            .unwrap();
        assert!(stale.source_edited);
        assert!(!stale.translation_edited);
        assert!(stale.translation_stale);
        assert!(stale.timing_edited);
        assert_eq!(
            database.list_job_translation_stats().await.unwrap()[0].stale_translation_count,
            1
        );

        let edited = database
            .update_segment(
                &manifest.job_id,
                &segment.id,
                "手動修正".to_string(),
                Some("人工翻译".to_string()),
                100,
                900,
            )
            .await
            .unwrap();
        assert!(edited.source_edited);
        assert!(edited.translation_edited);
        assert!(!edited.translation_stale);

        let mut regenerated = segment.clone();
        regenerated.source_text = "再生成された文".to_string();
        regenerated.translated_text = Some("重新生成的翻译".to_string());
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![regenerated],
            })
            .await
            .unwrap();
        let preserved = database.list_segments(&edited.job_id).await.unwrap();
        assert_eq!(preserved[0].source_text, "手動修正");
        assert_eq!(preserved[0].translated_text.as_deref(), Some("人工翻译"));
        assert!(preserved[0].source_edited);
        assert!(preserved[0].translation_edited);
        assert!(preserved[0].timing_edited);
        assert_eq!((preserved[0].start_ms, preserved[0].end_ms), (100, 900));

        let invalid_timing = database
            .update_segment(
                &edited.job_id,
                &edited.id,
                edited.source_text,
                edited.translated_text,
                900,
                900,
            )
            .await
            .unwrap_err();
        assert!(invalid_timing.to_string().contains("later than"));

        let restored = database.restore_segment(&segments[0]).await.unwrap();
        assert_eq!(restored.source_text, "テスト");
        assert_eq!(restored.translated_text.as_deref(), Some("测试"));
        assert_eq!((restored.start_ms, restored.end_ms), (0, 1_000));
        assert!(!restored.source_edited);
        assert!(!restored.translation_edited);
        assert!(!restored.translation_stale);
        assert!(!restored.timing_edited);

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn persists_manual_task_order_and_places_new_tasks_first() {
        let root =
            std::env::temp_dir().join(format!("atogaki-task-order-test-{}", uuid::Uuid::new_v4()));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let first_job = Job::create_in(&root).unwrap();
        let second_job = Job::create_in(&root).unwrap();
        let first = JobManifest::new(
            &first_job,
            None,
            None,
            crate::domain::LanguagePair::default(),
        );
        let second = JobManifest::new(
            &second_job,
            None,
            None,
            crate::domain::LanguagePair::default(),
        );
        for manifest in [&first, &second] {
            database
                .sync_snapshot(&JobSnapshot {
                    manifest: manifest.clone(),
                    segments: Vec::new(),
                })
                .await
                .unwrap();
        }

        let requested = vec![second.job_id.clone(), first.job_id.clone()];
        let reordered = database.reorder_jobs(&requested).await.unwrap();
        assert_eq!(
            reordered
                .iter()
                .map(|job| job.job_id.as_str())
                .collect::<Vec<_>>(),
            [second.job_id.as_str(), first.job_id.as_str()]
        );
        assert!(
            database
                .reorder_jobs(&[first.job_id.clone()])
                .await
                .is_err()
        );

        let newest_job = Job::create_in(&root).unwrap();
        let newest = JobManifest::new(
            &newest_job,
            None,
            None,
            crate::domain::LanguagePair::default(),
        );
        database
            .sync_snapshot(&JobSnapshot {
                manifest: newest.clone(),
                segments: Vec::new(),
            })
            .await
            .unwrap();
        assert_eq!(database.list_jobs().await.unwrap()[0].job_id, newest.job_id);

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn learning_items_keep_utf16_selections_and_source_snapshots() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-learning-item-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let mut segment = TranscriptSegment::new(1_000, 2_500, "今日は🍎を食べる".to_string());
        segment.set_translation(Some("今天吃苹果".to_string()));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![segment.clone()],
            })
            .await
            .unwrap();

        let input = NewLocalLearningSelection {
            job_id: manifest.job_id.clone(),
            segment_id: segment.id.clone(),
            item_type: "selection".to_string(),
            selection_start_utf16: 3,
            selection_end_utf16: 5,
        };
        let saved = database
            .save_learning_selection(input.clone())
            .await
            .unwrap();
        assert_eq!(saved.item.source_text, "🍎");
        assert_eq!(saved.item.occurrence_count, 1);
        assert_eq!(saved.occurrences[0].start_ms, 1_000);
        assert_eq!(
            saved.occurrences[0].segment_translation_snapshot.as_deref(),
            Some("今天吃苹果")
        );

        let deduplicated = database.save_learning_selection(input).await.unwrap();
        assert_eq!(deduplicated.item.id, saved.item.id);
        assert_eq!(deduplicated.item.occurrence_count, 1);
        let updated = database
            .update_learning_item_meaning(&saved.item.id, Some("苹果".to_string()))
            .await
            .unwrap();
        assert_eq!(updated.item.meaning_text.as_deref(), Some("苹果"));

        let with_dictionary = database
            .upsert_learning_lookup_result(NewLocalLearningLookupResult {
                learning_item_id: saved.item.id.clone(),
                provider_id: "jmdict".to_string(),
                provider_name: "JMdict".to_string(),
                headword: "林檎".to_string(),
                reading: Some("りんご".to_string()),
                pronunciation: None,
                audio_url: None,
                senses: vec![LocalLearningLookupSense {
                    part_of_speech: Some("名词".to_string()),
                    definitions: vec!["apple".to_string()],
                    examples: Vec::new(),
                }],
                attribution_text: "JMdict / EDRDG".to_string(),
                source_url: Some("https://www.edrdg.org/jmdict/".to_string()),
                license_label: Some("CC BY-SA 4.0".to_string()),
                data_version: Some("test".to_string()),
                fetched_at_unix: 42,
                cache_expires_at_unix: None,
            })
            .await
            .unwrap();
        assert_eq!(with_dictionary.lookup_results.len(), 1);
        assert_eq!(with_dictionary.lookup_results[0].provider_id, "jmdict");
        assert_eq!(
            with_dictionary.lookup_results[0].senses[0].definitions,
            vec!["apple"]
        );

        let replaced_dictionary = database
            .upsert_learning_lookup_result(NewLocalLearningLookupResult {
                learning_item_id: saved.item.id.clone(),
                provider_id: "jmdict".to_string(),
                provider_name: "JMdict".to_string(),
                headword: "林檎".to_string(),
                reading: Some("りんご".to_string()),
                pronunciation: None,
                audio_url: None,
                senses: vec![LocalLearningLookupSense {
                    part_of_speech: Some("名词".to_string()),
                    definitions: vec!["apple; fruit".to_string()],
                    examples: Vec::new(),
                }],
                attribution_text: "JMdict / EDRDG".to_string(),
                source_url: None,
                license_label: Some("CC BY-SA 4.0".to_string()),
                data_version: Some("test-2".to_string()),
                fetched_at_unix: 43,
                cache_expires_at_unix: None,
            })
            .await
            .unwrap();
        assert_eq!(replaced_dictionary.lookup_results.len(), 1);
        assert_eq!(
            replaced_dictionary.lookup_results[0].senses[0].definitions,
            vec!["apple; fruit"]
        );
        let selected_meaning = database
            .update_learning_item_meaning_from_provider(
                &saved.item.id,
                "apple; fruit",
                "jmdict",
                "JMdict",
            )
            .await
            .unwrap();
        assert_eq!(
            selected_meaning.item.meaning_text.as_deref(),
            Some("apple; fruit")
        );
        assert_eq!(
            selected_meaning.item.meaning_provider_id.as_deref(),
            Some("jmdict")
        );
        assert_eq!(
            selected_meaning.item.meaning_source_label.as_deref(),
            Some("JMdict")
        );

        database.delete_job(&manifest.job_id).await.unwrap();
        let preserved = database
            .get_learning_item(&saved.item.id)
            .await
            .unwrap()
            .unwrap();
        assert!(preserved.occurrences[0].job_id.is_none());
        assert!(preserved.occurrences[0].segment_id.is_none());
        assert_eq!(
            preserved.occurrences[0].segment_source_snapshot,
            "今日は🍎を食べる"
        );

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn block_deletion_only_invalidates_recorded_machine_context_consumers() {
        let root =
            std::env::temp_dir().join(format!("atogaki-context-delete-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let segments = (0..4)
            .map(|i| {
                let mut cue = TranscriptSegment::new(i * 2000, i * 2000 + 1000, format!("cue {i}"));
                cue.set_translation(Some(format!("translation {i}")));
                cue
            })
            .collect();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments,
            })
            .await
            .unwrap();
        let initial = database.list_segments(&manifest.job_id).await.unwrap();
        sqlx::query("UPDATE local_subtitle_segments SET translation_edited = 1 WHERE id = ?")
            .bind(&initial[2].id)
            .execute(&database.pool)
            .await
            .unwrap();
        for target in [&initial[0].id, &initial[2].id] {
            sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,0)")
                .bind(target)
                .bind(&initial[3].id)
                .execute(&database.pool)
                .await
                .unwrap();
        }
        let before = database.list_segments(&manifest.job_id).await.unwrap();
        let provenance = database
            .list_subtitle_provenance(&manifest.job_id)
            .await
            .unwrap();
        let after = database
            .edit_subtitle_block(&manifest.job_id, &before, Some(&before[3].id), None)
            .await
            .unwrap();
        assert!(after[0].translation_stale);
        assert_eq!(after[1], before[1]); // no dependency: absolutely unchanged
        assert_eq!(after[2], before[2]); // manual translation: protected
        for (a, b) in after.iter().zip(before.iter()) {
            assert_eq!(a.source_text, b.source_text);
            assert_eq!(a.source_edited, b.source_edited);
            assert_eq!(a.start_ms, b.start_ms);
            assert_eq!(a.end_ms, b.end_ms);
        }
        let after_provenance = database
            .list_subtitle_provenance(&manifest.job_id)
            .await
            .unwrap();
        for p in after_provenance {
            assert_eq!(
                Some(&p),
                provenance.iter().find(|old| old.segment_id == p.segment_id)
            );
        }
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn splits_merges_restores_and_freezes_subtitle_structure() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-local-structure-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let mut first = TranscriptSegment::new(0, 1_000, "hello world".to_string());
        first.set_translation(Some("你好 世界".to_string()));
        let mut second = TranscriptSegment::new(1_100, 2_000, "again".to_string());
        second.set_translation(Some("再次".to_string()));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second.clone()],
            })
            .await
            .unwrap();
        let original = database.list_segments(&manifest.job_id).await.unwrap();

        sqlx::query("INSERT INTO local_learning_items(id,source_language,target_language,item_type,source_text,source_key,created_at_unix,updated_at_unix) VALUES ('test-learning','en','zh-CN','sentence','hello world','hello',0,0)")
            .execute(&database.pool).await.unwrap();
        sqlx::query("INSERT INTO local_learning_occurrences(id,learning_item_id,job_id,segment_id,job_display_name_snapshot,segment_source_snapshot,selection_start_utf16,selection_end_utf16,start_ms,end_ms,created_at_unix) VALUES ('test-occurrence','test-learning',?,?,'test','hello world',0,5,0,1000,0)")
            .bind(&manifest.job_id).bind(&original[0].id).execute(&database.pool).await.unwrap();
        let mut inserted = original[0].clone();
        inserted.id = uuid::Uuid::new_v4().to_string();
        inserted.start_ms = 1000;
        inserted.end_ms = 1100;
        inserted.source_text = "added".into();
        inserted.translated_text = None;
        let mut bad = inserted.clone();
        bad.start_ms = 999;
        assert!(
            database
                .edit_subtitle_block(&manifest.job_id, &original, None, Some(bad))
                .await
                .is_err()
        );
        let added = database
            .edit_subtitle_block(&manifest.job_id, &original, None, Some(inserted.clone()))
            .await
            .unwrap();
        assert_eq!(added.len(), 3);
        assert_eq!(added[1].id, inserted.id);
        assert_eq!(added[0], original[0]);
        let mut unchanged_second = original[1].clone();
        unchanged_second.segment_index = 2;
        assert_eq!(added[2], unchanged_second);
        let link: Option<String> = sqlx::query_scalar(
            "SELECT segment_id FROM local_learning_occurrences WHERE id='test-occurrence'",
        )
        .fetch_one(&database.pool)
        .await
        .unwrap();
        assert_eq!(link.as_deref(), Some(original[0].id.as_str()));
        assert!(
            database
                .edit_subtitle_block(&manifest.job_id, &original, Some(&inserted.id), None)
                .await
                .is_err()
        );
        let deleted = database
            .edit_subtitle_block(&manifest.job_id, &added, Some(&inserted.id), None)
            .await
            .unwrap();
        assert_eq!(deleted.len(), 2);
        let restored = database
            .restore_segment_structure(&manifest.job_id, &added, &deleted)
            .await
            .unwrap();
        assert_eq!(restored, added);
        database
            .restore_segment_structure(&manifest.job_id, &original, &restored)
            .await
            .unwrap();
        // Delete all blocks and undo, retaining learning text/time snapshots.
        let one = database
            .edit_subtitle_block(&manifest.job_id, &original, Some(&original[0].id), None)
            .await
            .unwrap();
        let empty = database
            .edit_subtitle_block(&manifest.job_id, &one, Some(&one[0].id), None)
            .await
            .unwrap();
        assert!(empty.is_empty());
        let snapshot:String=sqlx::query_scalar("SELECT segment_source_snapshot FROM local_learning_occurrences WHERE id='test-occurrence'").fetch_one(&database.pool).await.unwrap();
        assert_eq!(snapshot, "hello world");
        database
            .restore_segment_structure(&manifest.job_id, &original, &empty)
            .await
            .unwrap();

        let mut timing_draft = original.clone();
        timing_draft[0].end_ms = 1_050;
        let retimed = database
            .save_segment_timing(&manifest.job_id, &original, &timing_draft)
            .await
            .unwrap();
        assert_eq!(retimed[0].end_ms, 1_050);
        assert_eq!(retimed[1].start_ms, 1_100);
        assert!(retimed[0].timing_edited);
        assert!(!retimed[1].timing_edited);
        let mut overlapping = retimed.clone();
        overlapping[0].end_ms = 1_150;
        let rejected_overlap = database
            .save_segment_timing(&manifest.job_id, &retimed, &overlapping)
            .await
            .unwrap_err();
        assert!(
            rejected_overlap
                .to_string()
                .contains("cannot create or change overlaps")
        );
        let stale_timing = database
            .save_segment_timing(&manifest.job_id, &original, &timing_draft)
            .await
            .unwrap_err();
        assert!(stale_timing.to_string().contains("reload before saving"));
        database
            .restore_segment_structure(&manifest.job_id, &original, &retimed)
            .await
            .unwrap();
        let mut text_tamper = original.clone();
        text_tamper[0].source_text = "timing endpoint must reject text".to_string();
        let rejected_text = database
            .save_segment_timing(&manifest.job_id, &original, &text_tamper)
            .await
            .unwrap_err();
        assert!(
            rejected_text
                .to_string()
                .contains("cannot change subtitle text")
        );

        let split = database
            .split_segment(
                &manifest.job_id,
                &first.id,
                500,
                "hello".to_string(),
                "world".to_string(),
                Some("你好".to_string()),
                Some("世界".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(split.len(), 3);
        assert_eq!(split[0].id, first.id);
        assert_ne!(split[1].id, first.id);
        assert_eq!(split[1].segment_index, 1);
        assert!(split[0].source_edited && split[1].source_edited);
        assert!(split[0].timing_edited && split[1].timing_edited);

        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second.clone()],
            })
            .await
            .unwrap();
        assert_eq!(
            database.list_segments(&manifest.job_id).await.unwrap(),
            split
        );

        let restored = database
            .restore_segment_structure(&manifest.job_id, &original, &split)
            .await
            .unwrap();
        assert_eq!(restored, original);
        let split_again = database
            .split_segment(
                &manifest.job_id,
                &first.id,
                500,
                "hello".to_string(),
                "world".to_string(),
                Some("你好".to_string()),
                Some("世界".to_string()),
            )
            .await
            .unwrap();
        let merged = database
            .merge_adjacent_segments(
                &manifest.job_id,
                &split_again[0].id,
                &split_again[1].id,
                "hello world".to_string(),
                Some("你好 世界".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, first.id);
        assert_eq!(merged[0].source_text, "hello world");
        assert_eq!(merged[0].translated_text.as_deref(), Some("你好 世界"));
        let restored_split = database
            .restore_segment_structure(&manifest.job_id, &split_again, &merged)
            .await
            .unwrap();
        assert_eq!(restored_split, split_again);

        first.source_text = "incoming snapshot must not replace structure".to_string();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![first, second],
            })
            .await
            .unwrap();
        assert_eq!(
            database
                .list_segments(&restored_split[0].job_id)
                .await
                .unwrap(),
            restored_split
        );

        let partially_translated_split = database
            .split_segment(
                &restored_split[0].job_id,
                &restored_split[1].id,
                750,
                "wo".to_string(),
                "rld".to_string(),
                Some("世".to_string()),
                None,
            )
            .await
            .unwrap();
        assert!(partially_translated_split[1].translated_text.is_none());
        assert!(partially_translated_split[2].translated_text.is_none());
        let untranslated_merge = database
            .merge_adjacent_segments(
                &restored_split[0].job_id,
                &partially_translated_split[1].id,
                &partially_translated_split[2].id,
                "world".to_string(),
                Some("世界".to_string()),
            )
            .await
            .unwrap();
        assert!(untranslated_merge[1].translated_text.is_none());

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn persists_video_render_progress_separately_from_source_tasks() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-local-render-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let manifest = JobManifest::new(
            &job,
            Some(root.join("input.mov")),
            None,
            crate::domain::LanguagePair::default(),
        );
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![],
            })
            .await
            .unwrap();

        let render = database
            .create_render_job(NewLocalRenderJob {
                id: "render-one".to_string(),
                source_job_id: manifest.job_id.clone(),
                input_path: root.join("input.mov").display().to_string(),
                subtitle_path: root.join("render.ass").display().to_string(),
                output_path: root.join("output.mp4").display().to_string(),
                subtitle_track: "bilingual".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(render.status, "queued");
        assert_eq!(
            database
                .get_job(&manifest.job_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "created"
        );

        database.mark_render_running(&render.id).await.unwrap();
        database
            .update_render_progress(&render.id, 0.42)
            .await
            .unwrap();
        database
            .finish_render(&render.id, "videotoolbox", "copy", None)
            .await
            .unwrap();
        let finished = database.list_render_jobs(&manifest.job_id).await.unwrap();
        assert_eq!(finished[0].status, "done");
        assert_eq!(finished[0].progress, 1.0);
        assert_eq!(finished[0].encoder.as_deref(), Some("videotoolbox"));

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn machine_translations_are_atomic_and_survive_snapshot_refresh() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-local-translation-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        let first = TranscriptSegment::new(0, 1_000, "最初の文".to_string());
        let second = TranscriptSegment::new(1_000, 2_000, "次の文".to_string());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second.clone()],
            })
            .await
            .unwrap();

        database
            .apply_machine_translations(
                &manifest.job_id,
                &[
                    LocalMachineTranslation {
                        expected_source_revision: None,
                        expected_translation_revision: None,
                        segment_id: first.id.clone(),
                        source_text: first.source_text.clone(),
                        expected_translated_text: None,
                        expected_translation_edited: false,
                        translated_text: "第一句话".to_string(),
                    },
                    LocalMachineTranslation {
                        expected_source_revision: None,
                        expected_translation_revision: None,
                        segment_id: second.id.clone(),
                        source_text: second.source_text.clone(),
                        expected_translated_text: None,
                        expected_translation_edited: false,
                        translated_text: "下一句话".to_string(),
                    },
                ],
            )
            .await
            .unwrap();
        let translated = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(translated[0].translated_text.as_deref(), Some("第一句话"));
        assert!(!translated[0].translation_edited);
        assert!(!translated[0].translation_stale);

        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second.clone()],
            })
            .await
            .unwrap();
        let preserved = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(preserved[0].translated_text.as_deref(), Some("第一句话"));

        database
            .update_segment(
                &manifest.job_id,
                &second.id,
                second.source_text.clone(),
                Some("人工修改".to_string()),
                second.start_ms as i64,
                second.end_ms as i64,
            )
            .await
            .unwrap();
        let overwrite_error = database
            .apply_machine_translations(
                &manifest.job_id,
                &[LocalMachineTranslation {
                    expected_source_revision: None,
                    expected_translation_revision: None,
                    segment_id: second.id.clone(),
                    source_text: second.source_text.clone(),
                    expected_translated_text: Some("下一句话".to_string()),
                    expected_translation_edited: false,
                    translated_text: "迟到的机器结果".to_string(),
                }],
            )
            .await
            .unwrap_err();
        assert!(overwrite_error.to_string().contains("changed while"));
        assert_eq!(
            database.list_segments(&manifest.job_id).await.unwrap()[1]
                .translated_text
                .as_deref(),
            Some("人工修改")
        );

        let mut changed = first.clone();
        changed.source_text = "変更された文".to_string();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![changed, second],
            })
            .await
            .unwrap();
        let stale = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(stale[0].translated_text.as_deref(), Some("第一句话"));
        assert!(stale[0].translation_stale);

        let error = database
            .apply_machine_translations(
                &manifest.job_id,
                &[LocalMachineTranslation {
                    expected_source_revision: None,
                    expected_translation_revision: None,
                    segment_id: first.id,
                    source_text: "已经过时的日文".to_string(),
                    expected_translated_text: Some("第一句话".to_string()),
                    expected_translation_edited: false,
                    translated_text: "不应写入".to_string(),
                }],
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("changed while"));
        let unchanged = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(unchanged[0].translated_text.as_deref(), Some("第一句话"));

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn identical_retranslation_replaces_context_and_preserves_conflict_guard() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-identical-translation-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        let before = TranscriptSegment::new(0, 1_000, "前文".into());
        let target = TranscriptSegment::new(1_000, 2_000, "対象".into());
        let after = TranscriptSegment::new(2_000, 3_000, "後文".into());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![before.clone(), target.clone(), after.clone()],
            })
            .await
            .unwrap();
        let update = |expected: Option<&str>| LocalMachineTranslation {
            expected_source_revision: None,
            expected_translation_revision: None,
            segment_id: target.id.clone(),
            source_text: target.source_text.clone(),
            expected_translated_text: expected.map(str::to_string),
            expected_translation_edited: false,
            translated_text: "目标".into(),
        };
        let dependency = |segment: &TranscriptSegment| LocalTranslationDependency {
            segment_id: segment.id.clone(),
            source_text: segment.source_text.clone(),
            expected_source_revision: 0,
        };
        database
            .apply_machine_translation_group(
                &manifest.job_id,
                &[update(None)],
                &[dependency(&before)],
            )
            .await
            .unwrap();
        // Same output, same context, and duplicate evidence must be idempotent.
        database
            .apply_machine_translation_group(
                &manifest.job_id,
                &[update(Some("目标"))],
                &[dependency(&before), dependency(&before)],
            )
            .await
            .unwrap();
        // The context may change even when the resulting text does not.
        database
            .apply_machine_translation_group(
                &manifest.job_id,
                &[update(Some("目标"))],
                &[dependency(&after)],
            )
            .await
            .unwrap();
        let edges: Vec<(String,)> = sqlx::query_as("SELECT context_segment_id FROM local_translation_dependency_edges WHERE target_segment_id = ?")
            .bind(&target.id).fetch_all(&database.pool).await.unwrap();
        assert_eq!(edges, vec![(after.id.clone(),)]);
        let error = database
            .apply_machine_translation_group(&manifest.job_id, &[update(Some("旧译文"))], &[])
            .await
            .unwrap_err();
        assert!(error.to_string().contains("changed while"));
        let edges: Vec<(String,)> = sqlx::query_as("SELECT context_segment_id FROM local_translation_dependency_edges WHERE target_segment_id = ?")
            .bind(&target.id).fetch_all(&database.pool).await.unwrap();
        assert_eq!(edges, vec![(after.id.clone(),)]);
        database
            .update_segment(
                &manifest.job_id,
                &before.id,
                "不再使用的上下文".into(),
                None,
                0,
                1_000,
            )
            .await
            .unwrap();
        assert!(
            !database
                .list_subtitle_provenance(&manifest.job_id)
                .await
                .unwrap()
                .iter()
                .find(|p| p.segment_id == target.id)
                .unwrap()
                .translation_dependency_stale
        );
        database
            .update_segment(
                &manifest.job_id,
                &after.id,
                "仍被引用的上下文".into(),
                None,
                2_000,
                3_000,
            )
            .await
            .unwrap();
        assert!(
            database
                .list_subtitle_provenance(&manifest.job_id)
                .await
                .unwrap()
                .iter()
                .find(|p| p.segment_id == target.id)
                .unwrap()
                .translation_dependency_stale
        );
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn cue_revisions_mark_context_dependent_translations_stale() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-cue-provenance-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let before = TranscriptSegment::new(0, 1_000, "前の文".to_string());
        let target = TranscriptSegment::new(1_000, 2_000, "対象".to_string());
        let after = TranscriptSegment::new(2_000, 3_000, "後の文".to_string());
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![before.clone(), target.clone(), after.clone()],
            })
            .await
            .unwrap();

        database
            .apply_machine_translation_group(
                &manifest.job_id,
                &[LocalMachineTranslation {
                    expected_source_revision: None,
                    expected_translation_revision: None,
                    segment_id: target.id.clone(),
                    source_text: target.source_text.clone(),
                    expected_translated_text: None,
                    expected_translation_edited: false,
                    translated_text: "目标".to_string(),
                }],
                &[
                    LocalTranslationDependency {
                        segment_id: before.id.clone(),
                        source_text: before.source_text.clone(),
                        expected_source_revision: 0,
                    },
                    LocalTranslationDependency {
                        segment_id: after.id.clone(),
                        source_text: after.source_text.clone(),
                        expected_source_revision: 0,
                    },
                ],
            )
            .await
            .unwrap();
        let provenance = database
            .list_subtitle_provenance(&manifest.job_id)
            .await
            .unwrap();
        let target_provenance = provenance
            .iter()
            .find(|item| item.segment_id == target.id)
            .unwrap();
        assert_eq!(target_provenance.translation_revision, 1);
        assert!(!target_provenance.translation_dependency_stale);

        database
            .update_segment(
                &manifest.job_id,
                &before.id,
                "修正した前の文".to_string(),
                None,
                0,
                1_000,
            )
            .await
            .unwrap();
        let provenance = database
            .list_subtitle_provenance(&manifest.job_id)
            .await
            .unwrap();
        let before_provenance = provenance
            .iter()
            .find(|item| item.segment_id == before.id)
            .unwrap();
        let target_provenance = provenance
            .iter()
            .find(|item| item.segment_id == target.id)
            .unwrap();
        assert_eq!(before_provenance.source_revision, 1);
        assert_eq!(before_provenance.source_kind, "manual");
        assert_eq!(before_provenance.alignment_status, "stale");
        assert!(target_provenance.translation_dependency_stale);

        database
            .update_segment(
                &manifest.job_id,
                &before.id,
                before.source_text.clone(),
                None,
                0,
                1_000,
            )
            .await
            .unwrap();
        database
            .apply_machine_translation_group(
                &manifest.job_id,
                &[LocalMachineTranslation {
                    expected_source_revision: None,
                    expected_translation_revision: None,
                    segment_id: target.id.clone(),
                    source_text: target.source_text.clone(),
                    expected_translated_text: Some("目标".to_string()),
                    expected_translation_edited: false,
                    translated_text: "重新翻译".to_string(),
                }],
                &[LocalTranslationDependency {
                    segment_id: before.id.clone(),
                    source_text: before.source_text.clone(),
                    expected_source_revision: 0,
                }],
            )
            .await
            .unwrap();
        let provenance = database
            .list_subtitle_provenance(&manifest.job_id)
            .await
            .unwrap();
        assert!(
            provenance
                .iter()
                .find(|item| item.segment_id == target.id)
                .unwrap()
                .translation_dependency_stale
        );

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn built_in_glossary_import_deduplicates_grouped_terms() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-builtin-glossary-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();

        database
            .ensure_builtin_glossary(
                "内置词表",
                "test-glossary",
                "v1",
                "ja",
                vec![
                    LocalGlossaryTermInput {
                        source_text: "前世".to_string(),
                        target_text: None,
                        prompt_scope: "content".to_string(),
                        content_group: Some("作品".to_string()),
                    },
                    LocalGlossaryTermInput {
                        source_text: "前世".to_string(),
                        target_text: None,
                        prompt_scope: "content".to_string(),
                        content_group: Some("作品".to_string()),
                    },
                    LocalGlossaryTermInput {
                        source_text: "ナブナ".to_string(),
                        target_text: Some("n-buna".to_string()),
                        prompt_scope: "core".to_string(),
                        content_group: None,
                    },
                ],
            )
            .await
            .unwrap();
        let glossaries = database.list_glossaries().await.unwrap();
        assert_eq!(glossaries.len(), 1);
        assert_eq!(glossaries[0].term_count, 2);
        assert_eq!(glossaries[0].prompt_term_count, 2);
        assert_eq!(glossaries[0].correction_count, 1);
        assert_eq!(glossaries[0].core_term_count, 1);
        assert_eq!(glossaries[0].content_group_count, 1);
        assert_eq!(glossaries[0].builtin_key.as_deref(), Some("test-glossary"));
        assert_eq!(glossaries[0].builtin_version.as_deref(), Some("v1"));
        assert!(database.delete_glossary(&glossaries[0].id).await.is_err());
        database
            .ensure_builtin_glossary(
                "内置词表",
                "test-glossary",
                "v2",
                "ja",
                vec![LocalGlossaryTermInput {
                    source_text: "前世".to_string(),
                    target_text: None,
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        let glossaries = database.list_glossaries().await.unwrap();
        assert_eq!(glossaries.len(), 1);
        assert_eq!(glossaries[0].term_count, 1);
        assert_eq!(glossaries[0].core_term_count, 1);
        assert_eq!(glossaries[0].builtin_version.as_deref(), Some("v2"));

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn legacy_builtin_glossary_is_refreshed_and_builtin_edits_copy_on_write() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-glossary-cow-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let legacy = database
            .save_glossary(
                None,
                "Yorushika".to_string(),
                "ja",
                vec![LocalGlossaryTermInput {
                    source_text: "旧内置项".to_string(),
                    target_text: None,
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        database
            .ensure_builtin_glossary(
                "Yorushika",
                "yorushika",
                "v3",
                "ja",
                vec![LocalGlossaryTermInput {
                    source_text: "新版内置项".to_string(),
                    target_text: None,
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();

        let migrated = database
            .get_glossary(&legacy.glossary.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(migrated.glossary.name, "Yorushika");
        assert_eq!(migrated.glossary.builtin_key.as_deref(), Some("yorushika"));
        assert_eq!(migrated.terms[0].source_text, "新版内置项");
        let builtin = database
            .list_glossaries()
            .await
            .unwrap()
            .into_iter()
            .find(|glossary| glossary.builtin_key.as_deref() == Some("yorushika"))
            .unwrap();
        let custom_copy = database
            .save_glossary(
                Some(&builtin.id),
                builtin.name.clone(),
                "ja",
                vec![LocalGlossaryTermInput {
                    source_text: "从内置修改".to_string(),
                    target_text: None,
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        assert_ne!(custom_copy.glossary.id, builtin.id);
        assert!(custom_copy.glossary.builtin_key.is_none());
        assert_eq!(custom_copy.glossary.name, "Yorushika（自定义）");
        let unchanged_builtin = database.get_glossary(&builtin.id).await.unwrap().unwrap();
        assert_eq!(unchanged_builtin.terms[0].source_text, "新版内置项");

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn gap_candidate_adoption_preserves_unrelated_legacy_overlaps() {
        let root =
            std::env::temp_dir().join(format!("atogaki-legacy-overlap-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![
                    TranscriptSegment::new(0, 1_500, "旧重叠一".into()),
                    TranscriptSegment::new(1_000, 2_000, "旧重叠二".into()),
                    TranscriptSegment::new(4_000, 5_000, "后文".into()),
                ],
            })
            .await
            .unwrap();
        let before = database.list_segments(&job.id()).await.unwrap();
        let candidates = vec![TranscriptSegment::new(2_500, 3_000, "补识别".into())];
        let after = database
            .replace_segment_range(&job.id(), 2_000, 4_000, &[], &candidates, None)
            .await
            .unwrap();
        assert_eq!(after.len(), 4);
        for old in &before {
            let kept = after.iter().find(|cue| cue.id == old.id).unwrap();
            assert_eq!(
                (kept.start_ms, kept.end_ms, &kept.source_text),
                (old.start_ms, old.end_ms, &old.source_text)
            );
        }
        // A genuine boundary cut still refuses adoption without removing work.
        assert!(
            database
                .replace_segment_range(&job.id(), 1_900, 4_000, &after[1..3], &candidates, None)
                .await
                .is_err()
        );
        assert_eq!(database.list_segments(&job.id()).await.unwrap(), after);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn partial_structure_write_failure_rolls_back_edits_and_candidate_adoption() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-structure-rollback-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let job_id = job.id();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![
                    TranscriptSegment::new(0, 1_000, "first".into()),
                    TranscriptSegment::new(1_000, 2_000, "middle".into()),
                    TranscriptSegment::new(2_000, 3_000, "last".into()),
                ],
            })
            .await
            .unwrap();
        let before = database.subtitle_workspace_snapshot(&job_id).await.unwrap();
        let provenance = database.list_subtitle_provenance(&job_id).await.unwrap();
        sqlx::query("INSERT INTO local_learning_items(id,source_language,target_language,item_type,source_text,source_key,created_at_unix,updated_at_unix) VALUES ('rollback-learning','en','zh-Hans','sentence','middle','middle',0,0)")
            .execute(&database.pool).await.unwrap();
        sqlx::query("INSERT INTO local_learning_occurrences(id,learning_item_id,job_id,segment_id,job_display_name_snapshot,segment_source_snapshot,selection_start_utf16,selection_end_utf16,start_ms,end_ms,created_at_unix) VALUES ('rollback-occurrence','rollback-learning',?,?,'test','middle',0,6,1000,2000,0)")
            .bind(&job_id).bind(&before.1[1].id).execute(&database.pool).await.unwrap();
        sqlx::query("INSERT INTO local_translation_dependency_edges(target_segment_id,context_segment_id,context_source_revision) VALUES (?,?,?)")
            .bind(&before.1[2].id).bind(&before.1[1].id).bind(provenance.iter().find(|p| p.segment_id == before.1[1].id).unwrap().source_revision)
            .execute(&database.pool).await.unwrap();
        // Fail after DELETE and at least one successful INSERT. This exercises
        // rollback of partial writes and cascading provenance/learning changes,
        // rather than rejecting invalid input before touching the database.
        sqlx::query("CREATE TRIGGER injected_structure_failure BEFORE INSERT ON local_subtitle_segments WHEN NEW.source_text = 'reject insertion' AND EXISTS (SELECT 1 FROM local_subtitle_segments WHERE job_id = NEW.job_id) BEGIN SELECT RAISE(ABORT, 'injected partial structure failure'); END")
            .execute(&database.pool).await.unwrap();

        for adoption in [false, true] {
            let result = if adoption {
                database
                    .replace_segment_range_with_revision(
                        &job_id,
                        1_000,
                        2_000,
                        &before.1[1..2],
                        &[
                            TranscriptSegment::new(1_000, 1_500, "accepted insertion".into()),
                            TranscriptSegment::new(1_500, 2_000, "reject insertion".into()),
                        ],
                        None,
                        Some(before.0),
                    )
                    .await
            } else {
                database
                    .split_segment(
                        &job_id,
                        &before.1[0].id,
                        500,
                        "accepted insertion".into(),
                        "reject insertion".into(),
                        None,
                        None,
                    )
                    .await
            };
            let error = result.unwrap_err();
            assert!(format!("{error:#}").contains("injected partial structure failure"));
            assert_eq!(
                database.subtitle_workspace_snapshot(&job_id).await.unwrap(),
                before
            );
            assert_eq!(
                database.list_subtitle_provenance(&job_id).await.unwrap(),
                provenance
            );
            let link: Option<String> = sqlx::query_scalar(
                "SELECT segment_id FROM local_learning_occurrences WHERE id = 'rollback-occurrence'",
            )
            .fetch_one(&database.pool)
            .await
            .unwrap();
            assert_eq!(link.as_deref(), Some(before.1[1].id.as_str()));
            let dependencies: Vec<(String, String, i64)> = sqlx::query_as(
                "SELECT target_segment_id,context_segment_id,context_source_revision FROM local_translation_dependency_edges",
            )
            .fetch_all(&database.pool)
            .await
            .unwrap();
            assert_eq!(
                dependencies,
                vec![(
                    before.1[2].id.clone(),
                    before.1[1].id.clone(),
                    provenance
                        .iter()
                        .find(|p| p.segment_id == before.1[1].id)
                        .unwrap()
                        .source_revision,
                )]
            );
            assert!(
                sqlx::query("PRAGMA foreign_key_check")
                    .fetch_all(&database.pool)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
        database.close().await;
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn selected_range_replacement_is_atomic_and_preserves_outside_segments() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-range-replacement-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let first = TranscriptSegment::new(0, 1_000, "前".to_string());
        let middle = TranscriptSegment::new(1_000, 2_000, "循环".to_string());
        let last = TranscriptSegment::new(2_000, 3_000, "后".to_string());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![first.clone(), middle, last.clone()],
            })
            .await
            .unwrap();
        let original = database.list_segments(job.id().as_str()).await.unwrap();
        let affected = vec![original[1].clone()];
        let candidates = vec![
            TranscriptSegment::new(1_000, 1_450, "新的".to_string()),
            TranscriptSegment::new(1_450, 2_000, "识别".to_string()),
        ];

        let updated = database
            .replace_segment_range(
                job.id().as_str(),
                1_000,
                2_000,
                &affected,
                &candidates,
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.len(), 4);
        assert_eq!(updated[0].id, first.id);
        assert_eq!(updated[1].source_text, "新的");
        assert_eq!(updated[2].source_text, "识别");
        assert_eq!(updated[3].id, last.id);
        assert!(updated[1].translated_text.is_none());

        let stale_preview = database
            .replace_segment_range(
                job.id().as_str(),
                1_000,
                2_000,
                &affected,
                &candidates,
                None,
            )
            .await
            .unwrap_err();
        assert!(stale_preview.to_string().contains("preview was open"));

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
