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

use crate::{application::job_snapshot::JobSnapshot, domain::TranscriptSegment};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

#[derive(Debug, Clone)]
pub struct LocalDatabase {
    pool: SqlitePool,
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

#[derive(Debug, Clone)]
pub struct LocalMachineTranslation {
    pub segment_id: String,
    pub source_text: String,
    pub translated_text: String,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct LocalTranslationRunRecord {
    pub id: String,
    pub job_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: Option<String>,
    pub endpoint_kind: String,
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
    pub segment_count: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
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
                completed_at_unix = COALESCE(local_jobs.completed_at_unix, excluded.completed_at_unix),
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
                created_at_unix, started_at_unix, completed_at_unix, updated_at_unix
             FROM local_jobs
             ORDER BY updated_at_unix DESC, job_id DESC",
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
                created_at_unix, started_at_unix, completed_at_unix, updated_at_unix
             FROM local_jobs
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local task")
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

    pub async fn apply_machine_translations(
        &self,
        job_id: &str,
        translations: &[LocalMachineTranslation],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        if translations.is_empty() {
            return self.list_segments(job_id).await;
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local translation transaction")?;

        for translation in translations {
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
                 WHERE job_id = ? AND id = ? AND source_text = ?",
            )
            .bind(translated_text)
            .bind(job_id)
            .bind(&translation.segment_id)
            .bind(&translation.source_text)
            .execute(&mut *tx)
            .await
            .context("failed to store local machine translation")?;
            if result.rows_affected() != 1 {
                return Err(anyhow!(
                    "subtitle changed while it was being translated: {}",
                    translation.segment_id
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
                segment_count, input_tokens, output_tokens, completed_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.job_id)
        .bind(&run.provider_id)
        .bind(&run.provider_name)
        .bind(&run.model)
        .bind(&run.endpoint_kind)
        .bind(run.segment_count)
        .bind(run.input_tokens)
        .bind(run.output_tokens)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("failed to record local translation run")?;
        Ok(())
    }

    pub async fn list_translation_runs(
        &self,
        job_id: &str,
    ) -> Result<Vec<LocalTranslationRunRecord>> {
        sqlx::query_as(
            "SELECT id, job_id, provider_id, provider_name, model, endpoint_kind,
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

    pub async fn update_segment(
        &self,
        job_id: &str,
        segment_id: &str,
        source_text: String,
        translated_text: Option<String>,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<LocalSubtitleSegmentRecord> {
        if start_ms < 0 {
            return Err(anyhow!("subtitle start time cannot be negative"));
        }
        if end_ms <= start_ms {
            return Err(anyhow!(
                "subtitle end time must be later than its start time"
            ));
        }
        let source_text = source_text.trim().to_string();
        if source_text.is_empty() {
            return Err(anyhow!("source subtitle text cannot be empty"));
        }
        let translated_text = translated_text
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local subtitle edit transaction")?;
        let current = sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             FROM local_subtitle_segments
             WHERE job_id = ? AND id = ?",
        )
        .bind(job_id)
        .bind(segment_id)
        .fetch_optional(&mut *tx)
        .await
        .context("failed to read local subtitle segment")?
        .ok_or_else(|| anyhow!("subtitle segment not found: {segment_id}"))?;

        let source_changed = current.source_text != source_text;
        let translation_changed = current.translated_text != translated_text;
        let timing_changed = current.start_ms != start_ms || current.end_ms != end_ms;
        let source_edited = current.source_edited || source_changed;
        let translation_edited = current.translation_edited || translation_changed;
        let timing_edited = current.timing_edited || timing_changed;
        let translation_stale = if translation_changed {
            false
        } else if source_changed {
            current.translated_text.is_some()
        } else {
            current.translation_stale
        };

        sqlx::query(
            "UPDATE local_subtitle_segments
             SET source_text = ?, translated_text = ?, source_edited = ?, translation_edited = ?,
                 translation_stale = ?, start_ms = ?, end_ms = ?, timing_edited = ?
             WHERE job_id = ? AND id = ?",
        )
        .bind(&source_text)
        .bind(&translated_text)
        .bind(source_edited)
        .bind(translation_edited)
        .bind(translation_stale)
        .bind(start_ms)
        .bind(end_ms)
        .bind(timing_edited)
        .bind(job_id)
        .bind(segment_id)
        .execute(&mut *tx)
        .await
        .context("failed to update local subtitle segment")?;
        sqlx::query(
            "UPDATE local_jobs
             SET updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(chrono::Utc::now().timestamp())
        .bind(job_id)
        .execute(&mut *tx)
        .await
        .context("failed to update local task edit timestamp")?;

        let updated = sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             FROM local_subtitle_segments
             WHERE job_id = ? AND id = ?",
        )
        .bind(job_id)
        .bind(segment_id)
        .fetch_one(&mut *tx)
        .await
        .context("failed to reload local subtitle segment")?;
        tx.commit()
            .await
            .context("failed to commit local subtitle edit")?;
        Ok(updated)
    }

    pub async fn restore_segment(
        &self,
        snapshot: &LocalSubtitleSegmentRecord,
    ) -> Result<LocalSubtitleSegmentRecord> {
        if snapshot.start_ms < 0 {
            return Err(anyhow!("subtitle start time cannot be negative"));
        }
        if snapshot.end_ms <= snapshot.start_ms {
            return Err(anyhow!(
                "subtitle end time must be later than its start time"
            ));
        }
        let source_text = snapshot.source_text.trim();
        if source_text.is_empty() {
            return Err(anyhow!("source subtitle text cannot be empty"));
        }
        let translated_text = snapshot
            .translated_text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local subtitle restore transaction")?;
        let result = sqlx::query(
            "UPDATE local_subtitle_segments
             SET source_text = ?, translated_text = ?, start_ms = ?, end_ms = ?,
                 source_edited = ?, translation_edited = ?, translation_stale = ?, timing_edited = ?
             WHERE job_id = ? AND id = ?",
        )
        .bind(source_text)
        .bind(translated_text)
        .bind(snapshot.start_ms)
        .bind(snapshot.end_ms)
        .bind(snapshot.source_edited)
        .bind(snapshot.translation_edited)
        .bind(snapshot.translation_stale)
        .bind(snapshot.timing_edited)
        .bind(&snapshot.job_id)
        .bind(&snapshot.id)
        .execute(&mut *tx)
        .await
        .context("failed to restore local subtitle segment")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!("subtitle segment not found: {}", snapshot.id));
        }
        sqlx::query(
            "UPDATE local_jobs
             SET updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(chrono::Utc::now().timestamp())
        .bind(&snapshot.job_id)
        .execute(&mut *tx)
        .await
        .context("failed to update local task restore timestamp")?;
        let restored = sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             FROM local_subtitle_segments
             WHERE job_id = ? AND id = ?",
        )
        .bind(&snapshot.job_id)
        .bind(&snapshot.id)
        .fetch_one(&mut *tx)
        .await
        .context("failed to reload restored local subtitle segment")?;
        tx.commit()
            .await
            .context("failed to commit local subtitle restore")?;
        Ok(restored)
    }

    pub async fn split_segment(
        &self,
        job_id: &str,
        segment_id: &str,
        boundary_ms: i64,
        left_source_text: String,
        right_source_text: String,
        left_translated_text: Option<String>,
        right_translated_text: Option<String>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let left_source_text = normalized_subtitle_text(left_source_text, "left source subtitle")?;
        let right_source_text =
            normalized_subtitle_text(right_source_text, "right source subtitle")?;
        let left_translated_text = normalized_optional_subtitle_text(left_translated_text);
        let right_translated_text = normalized_optional_subtitle_text(right_translated_text);
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin subtitle split transaction")?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let mut segments = list_segments_in_transaction(&mut tx, job_id).await?;
        let index = segments
            .iter()
            .position(|segment| segment.id == segment_id)
            .ok_or_else(|| anyhow!("subtitle segment not found: {segment_id}"))?;
        let original = segments[index].clone();
        if boundary_ms <= original.start_ms || boundary_ms >= original.end_ms {
            return Err(anyhow!(
                "subtitle split boundary must be inside the selected segment"
            ));
        }

        let translations = match (left_translated_text, right_translated_text) {
            (Some(left), Some(right)) => (Some(left), Some(right), true),
            _ => (None, None, false),
        };
        let mut left = original.clone();
        left.end_ms = boundary_ms;
        left.source_text = left_source_text;
        left.translated_text = translations.0;
        left.source_edited = true;
        left.translation_edited = translations.2;
        left.translation_stale = false;
        left.timing_edited = true;
        let mut right = original;
        right.id = uuid::Uuid::new_v4().to_string();
        right.start_ms = boundary_ms;
        right.source_text = right_source_text;
        right.translated_text = translations.1;
        right.source_edited = true;
        right.translation_edited = translations.2;
        right.translation_stale = false;
        right.timing_edited = true;
        segments.splice(index..=index, [left, right]);
        persist_structure_segments(&mut tx, job_id, &mut segments).await?;
        tx.commit()
            .await
            .context("failed to commit subtitle split transaction")?;
        Ok(segments)
    }

    pub async fn merge_adjacent_segments(
        &self,
        job_id: &str,
        left_segment_id: &str,
        right_segment_id: &str,
        source_text: String,
        translated_text: Option<String>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let source_text = normalized_subtitle_text(source_text, "merged source subtitle")?;
        let requested_translation = normalized_optional_subtitle_text(translated_text);
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin subtitle merge transaction")?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let mut segments = list_segments_in_transaction(&mut tx, job_id).await?;
        let left_index = segments
            .iter()
            .position(|segment| segment.id == left_segment_id)
            .ok_or_else(|| anyhow!("subtitle segment not found: {left_segment_id}"))?;
        if left_index + 1 >= segments.len() || segments[left_index + 1].id != right_segment_id {
            return Err(anyhow!("only adjacent subtitle segments can be merged"));
        }
        let left = segments[left_index].clone();
        let right = segments[left_index + 1].clone();
        let both_translated = left.translated_text.is_some() && right.translated_text.is_some();
        let merged_translation = requested_translation.filter(|_| both_translated);
        let mut merged = left.clone();
        merged.start_ms = left.start_ms.min(right.start_ms);
        merged.end_ms = left.end_ms.max(right.end_ms);
        merged.source_text = source_text;
        merged.translated_text = merged_translation.clone();
        merged.source_edited = true;
        merged.translation_edited = merged_translation.is_some();
        merged.translation_stale =
            merged_translation.is_some() && (left.translation_stale || right.translation_stale);
        merged.timing_edited = true;
        segments.splice(left_index..=left_index + 1, [merged]);
        persist_structure_segments(&mut tx, job_id, &mut segments).await?;
        tx.commit()
            .await
            .context("failed to commit subtitle merge transaction")?;
        Ok(segments)
    }

    pub async fn restore_segment_structure(
        &self,
        job_id: &str,
        before_segments: &[LocalSubtitleSegmentRecord],
        after_segments: &[LocalSubtitleSegmentRecord],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin subtitle structure restore transaction")?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let current = list_segments_in_transaction(&mut tx, job_id).await?;
        if current != after_segments {
            return Err(anyhow!(
                "subtitle structure changed after this operation; reload before undoing"
            ));
        }
        let mut restored = before_segments.to_vec();
        persist_structure_segments(&mut tx, job_id, &mut restored).await?;
        tx.commit()
            .await
            .context("failed to commit subtitle structure restore transaction")?;
        Ok(restored)
    }

    pub async fn save_segment_timing(
        &self,
        job_id: &str,
        before_segments: &[LocalSubtitleSegmentRecord],
        after_segments: &[LocalSubtitleSegmentRecord],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        if before_segments.len() != after_segments.len() {
            return Err(anyhow!(
                "timing edits cannot add or remove subtitle segments"
            ));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin subtitle timing transaction")?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let current = list_segments_in_transaction(&mut tx, job_id).await?;
        if current != before_segments {
            return Err(anyhow!(
                "subtitle timing changed after editing started; reload before saving"
            ));
        }

        let mut updated = after_segments.to_vec();
        for (before, after) in before_segments.iter().zip(updated.iter_mut()) {
            if before.id != after.id
                || before.job_id != after.job_id
                || before.segment_index != after.segment_index
                || before.source_text != after.source_text
                || before.translated_text != after.translated_text
                || before.source_edited != after.source_edited
                || before.translation_edited != after.translation_edited
                || before.translation_stale != after.translation_stale
            {
                return Err(anyhow!(
                    "timing edits cannot change subtitle text or structure"
                ));
            }
            after.timing_edited = before.timing_edited
                || before.start_ms != after.start_ms
                || before.end_ms != after.end_ms;
        }
        for index in 0..updated.len().saturating_sub(1) {
            let before_overlap =
                before_segments[index].end_ms > before_segments[index + 1].start_ms;
            let after_overlap = updated[index].end_ms > updated[index + 1].start_ms;
            if after_overlap {
                let overlap_was_unchanged = before_overlap
                    && updated[index].end_ms == before_segments[index].end_ms
                    && updated[index + 1].start_ms == before_segments[index + 1].start_ms;
                if !overlap_was_unchanged {
                    return Err(anyhow!(
                        "subtitle timing edits cannot create or change overlaps on one track"
                    ));
                }
            }
        }
        persist_structure_segments(&mut tx, job_id, &mut updated).await?;
        tx.commit()
            .await
            .context("failed to commit subtitle timing transaction")?;
        Ok(updated)
    }

    pub async fn replace_segments(
        &self,
        job_id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local subtitle transaction")?;
        self.replace_segments_in_transaction(&mut tx, job_id, segments)
            .await?;
        tx.commit()
            .await
            .context("failed to commit local subtitle transaction")
    }

    async fn replace_segments_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        job_id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<()> {
        sqlx::query("DELETE FROM local_subtitle_segments WHERE job_id = ?")
            .bind(job_id)
            .execute(&mut **tx)
            .await
            .context("failed to clear local subtitle segments")?;

        for (index, segment) in segments.iter().enumerate() {
            insert_segment(
                tx,
                job_id,
                index,
                segment,
                to_i64(segment.start_ms, "segment start")?,
                to_i64(segment.end_ms, "segment end")?,
                segment.source_text.as_str(),
                segment.translated_text.as_deref(),
                segment.source_edited,
                false,
                segment.translation_stale,
                false,
            )
            .await?;
        }

        Ok(())
    }

    async fn merge_snapshot_segments_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        job_id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<()> {
        let existing = sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             FROM local_subtitle_segments
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_all(&mut **tx)
        .await
        .context("failed to read existing local subtitle segments")?
        .into_iter()
        .map(|segment| (segment.id.clone(), segment))
        .collect::<HashMap<_, _>>();

        sqlx::query("DELETE FROM local_subtitle_segments WHERE job_id = ?")
            .bind(job_id)
            .execute(&mut **tx)
            .await
            .context("failed to refresh local subtitle segments")?;

        for (index, segment) in segments.iter().enumerate() {
            let previous = existing.get(&segment.id);
            let source_text = previous
                .filter(|segment| segment.source_edited)
                .map(|segment| segment.source_text.as_str())
                .unwrap_or(&segment.source_text);
            let incoming_translation = segment.translated_text.as_deref();
            let (start_ms, end_ms, timing_edited) = match previous {
                Some(previous) if previous.timing_edited => {
                    (previous.start_ms, previous.end_ms, true)
                }
                _ => (
                    to_i64(segment.start_ms, "segment start")?,
                    to_i64(segment.end_ms, "segment end")?,
                    false,
                ),
            };
            let (translated_text, translation_edited, translation_stale) = match previous {
                Some(previous) if previous.translation_edited => (
                    previous.translated_text.as_deref(),
                    true,
                    previous.translation_stale || source_text != previous.source_text,
                ),
                Some(previous)
                    if incoming_translation.is_none() && previous.translated_text.is_some() =>
                {
                    (
                        previous.translated_text.as_deref(),
                        false,
                        previous.translation_stale || source_text != previous.source_text,
                    )
                }
                _ => (incoming_translation, false, segment.translation_stale),
            };
            insert_segment(
                tx,
                job_id,
                index,
                segment,
                start_ms,
                end_ms,
                source_text,
                translated_text,
                previous
                    .map(|segment| segment.source_edited)
                    .unwrap_or(segment.source_edited),
                translation_edited,
                translation_stale,
                timing_edited,
            )
            .await?;
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn normalized_subtitle_text(value: String, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow!("{field} cannot be empty"));
    }
    Ok(value.to_string())
}

fn normalized_optional_subtitle_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn ensure_structure_editable(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
) -> Result<()> {
    let status = sqlx::query_scalar::<_, String>("SELECT status FROM local_jobs WHERE job_id = ?")
        .bind(job_id)
        .fetch_optional(&mut **tx)
        .await
        .context("failed to read subtitle task state")?
        .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
    if !matches!(status.as_str(), "done" | "failed") {
        return Err(anyhow!(
            "subtitle structure can only be edited after transcription stops"
        ));
    }
    Ok(())
}

async fn list_segments_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
) -> Result<Vec<LocalSubtitleSegmentRecord>> {
    sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
        "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
            source_edited, translation_edited, translation_stale, timing_edited
         FROM local_subtitle_segments
         WHERE job_id = ?
         ORDER BY segment_index ASC",
    )
    .bind(job_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to read subtitle structure")
}

async fn persist_structure_segments(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
    segments: &mut [LocalSubtitleSegmentRecord],
) -> Result<()> {
    let mut ids = HashSet::new();
    for (index, segment) in segments.iter_mut().enumerate() {
        if segment.job_id != job_id {
            return Err(anyhow!("subtitle segment belongs to a different task"));
        }
        if !ids.insert(segment.id.clone()) {
            return Err(anyhow!("duplicate subtitle segment id: {}", segment.id));
        }
        if segment.start_ms < 0 || segment.end_ms <= segment.start_ms {
            return Err(anyhow!(
                "invalid subtitle timing for segment {}",
                segment.id
            ));
        }
        segment.source_text =
            normalized_subtitle_text(std::mem::take(&mut segment.source_text), "source subtitle")?;
        segment.translated_text = normalized_optional_subtitle_text(segment.translated_text.take());
        segment.segment_index =
            i64::try_from(index).context("subtitle index exceeds SQLite i64")?;
    }

    sqlx::query("DELETE FROM local_subtitle_segments WHERE job_id = ?")
        .bind(job_id)
        .execute(&mut **tx)
        .await
        .context("failed to replace subtitle structure")?;
    for segment in segments.iter() {
        sqlx::query(
            "INSERT INTO local_subtitle_segments (
                id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale, timing_edited
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&segment.id)
        .bind(job_id)
        .bind(segment.segment_index)
        .bind(segment.start_ms)
        .bind(segment.end_ms)
        .bind(&segment.source_text)
        .bind(&segment.translated_text)
        .bind(segment.source_edited)
        .bind(segment.translation_edited)
        .bind(segment.translation_stale)
        .bind(segment.timing_edited)
        .execute(&mut **tx)
        .await
        .context("failed to insert edited subtitle structure")?;
    }
    sqlx::query(
        "UPDATE local_jobs
         SET subtitle_structure_edited = 1, updated_at_unix = MAX(updated_at_unix, ?)
         WHERE job_id = ?",
    )
    .bind(chrono::Utc::now().timestamp())
    .bind(job_id)
    .execute(&mut **tx)
    .await
    .context("failed to freeze edited subtitle structure")?;
    Ok(())
}

async fn insert_segment(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
    index: usize,
    segment: &TranscriptSegment,
    start_ms: i64,
    end_ms: i64,
    source_text: &str,
    translated_text: Option<&str>,
    source_edited: bool,
    translation_edited: bool,
    translation_stale: bool,
    timing_edited: bool,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO local_subtitle_segments (
            id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
            source_edited, translation_edited, translation_stale, timing_edited
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&segment.id)
    .bind(job_id)
    .bind(i64::try_from(index).context("subtitle index exceeds SQLite i64")?)
    .bind(start_ms)
    .bind(end_ms)
    .bind(source_text)
    .bind(translated_text)
    .bind(source_edited)
    .bind(translation_edited)
    .bind(translation_stale)
    .bind(timing_edited)
    .execute(&mut **tx)
    .await
    .context("failed to insert local subtitle segment")?;
    Ok(())
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
        LocalDatabase, LocalGlossaryTermInput, LocalMachineTranslation, NewLocalRenderJob,
    };
    use crate::{
        application::{
            job_manifest::JobManifest, job_snapshot::JobSnapshot, job_status::JobStatus,
        },
        domain::TranscriptSegment,
        infrastructure::job_store::Job,
    };

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
                        segment_id: first.id.clone(),
                        source_text: first.source_text.clone(),
                        translated_text: "第一句话".to_string(),
                    },
                    LocalMachineTranslation {
                        segment_id: second.id.clone(),
                        source_text: second.source_text.clone(),
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
                    segment_id: first.id,
                    source_text: "已经过时的日文".to_string(),
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
}
