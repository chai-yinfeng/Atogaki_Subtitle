use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
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
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalGlossaryRecord {
    pub id: String,
    pub name: String,
    pub source_language: String,
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

#[derive(Debug, Clone, Serialize, FromRow)]
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
}

#[derive(Debug, Clone)]
pub struct LocalMachineTranslation {
    pub segment_id: String,
    pub source_text: String,
    pub translated_text: String,
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
             SET status = 'failed', message = '任务已中断', error_message = ?, updated_at_unix = ?
             WHERE job_id = ? AND status NOT IN ('done', 'failed')",
        )
        .bind(error)
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
                created_at_unix, updated_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
                storage_dir = excluded.storage_dir,
                input_path = excluded.input_path,
                render_output_path = excluded.render_output_path,
                status = excluded.status,
                message = excluded.message,
                error_message = excluded.error_message,
                source_language = excluded.source_language,
                target_language = excluded.target_language,
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
        .bind(to_i64(manifest.updated_at_unix, "updated_at_unix")?)
        .execute(&mut *tx)
        .await
        .context("failed to upsert local task")?;

        if !snapshot.segments.is_empty() {
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
                created_at_unix, updated_at_unix
             FROM local_jobs
             ORDER BY updated_at_unix DESC, job_id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list local tasks")
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<LocalJobRecord>> {
        sqlx::query_as::<_, LocalJobRecord>(
            "SELECT job_id, display_name, storage_dir, input_path, render_output_path, status, message,
                error_message, glossary_id, glossary_name, glossary_snapshot_path,
                source_language, target_language,
                created_at_unix, updated_at_unix
             FROM local_jobs
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local task")
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
        seed_version: &str,
        terms: Vec<LocalGlossaryTermInput>,
    ) -> Result<()> {
        let name = normalized_glossary_name(name)?;
        let mut seen = HashSet::new();
        let mut terms = terms;
        terms.retain(|term| seen.insert((term.source_text.clone(), term.target_text.clone())));
        let terms = normalized_glossary_terms(terms)?;
        let setting_key = format!("builtin_glossary_seeded:{name}:{seed_version}");
        if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_settings WHERE key = ?")
            .bind(&setting_key)
            .fetch_one(&self.pool)
            .await
            .context("failed to check built-in glossary seed state")?
            > 0
        {
            return Ok(());
        }

        let existing_id =
            sqlx::query_scalar::<_, String>("SELECT id FROM local_glossaries WHERE name = ?")
                .bind(&name)
                .fetch_optional(&self.pool)
                .await
                .context("failed to check built-in glossary")?;
        if let Some(glossary_id) = existing_id {
            let mut tx = self.pool.begin().await?;
            for term in terms {
                sqlx::query(
                    "UPDATE local_glossary_terms
                     SET prompt_scope = ?, content_group = ?
                     WHERE glossary_id = ? AND source_text = ?
                       AND ((target_text IS NULL AND ? IS NULL) OR target_text = ?)",
                )
                .bind(term.prompt_scope)
                .bind(term.content_group)
                .bind(&glossary_id)
                .bind(term.source_text)
                .bind(&term.target_text)
                .bind(&term.target_text)
                .execute(&mut *tx)
                .await
                .context("failed to classify built-in glossary term")?;
            }
            tx.commit()
                .await
                .context("failed to classify built-in glossary")?;
        } else {
            self.save_glossary(None, name, "ja", terms).await?;
        }

        sqlx::query(
            "INSERT INTO local_settings (key, value, updated_at_unix)
             VALUES (?, '1', ?)
             ON CONFLICT(key) DO NOTHING",
        )
        .bind(setting_key)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("failed to record built-in glossary seed state")?;
        Ok(())
    }

    pub async fn list_glossaries(&self) -> Result<Vec<LocalGlossaryRecord>> {
        sqlx::query_as::<_, LocalGlossaryRecord>(
            "SELECT g.id, g.name, g.source_language, COUNT(t.id) AS term_count,
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
            "SELECT g.id, g.name, g.source_language, COUNT(t.id) AS term_count,
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
        let name = normalized_glossary_name(&name)?;
        let terms = normalized_glossary_terms(terms)?;
        let glossary_id = glossary_id
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

    pub async fn list_segments(&self, job_id: &str) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
                source_edited, translation_edited, translation_stale
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
                source_edited, translation_edited, translation_stale
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

    pub async fn update_segment_text(
        &self,
        job_id: &str,
        segment_id: &str,
        source_text: String,
        translated_text: Option<String>,
    ) -> Result<LocalSubtitleSegmentRecord> {
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
                source_edited, translation_edited, translation_stale
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
        let source_edited = current.source_edited || source_changed;
        let translation_edited = current.translation_edited || translation_changed;
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
                 translation_stale = ?
             WHERE job_id = ? AND id = ?",
        )
        .bind(&source_text)
        .bind(&translated_text)
        .bind(source_edited)
        .bind(translation_edited)
        .bind(translation_stale)
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
                source_edited, translation_edited, translation_stale
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
                segment.source_text.as_str(),
                segment.translated_text.as_deref(),
                segment.source_edited,
                false,
                segment.translation_stale,
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
                source_edited, translation_edited, translation_stale
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
                source_text,
                translated_text,
                previous
                    .map(|segment| segment.source_edited)
                    .unwrap_or(segment.source_edited),
                translation_edited,
                translation_stale,
            )
            .await?;
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_segment(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
    index: usize,
    segment: &TranscriptSegment,
    source_text: &str,
    translated_text: Option<&str>,
    source_edited: bool,
    translation_edited: bool,
    translation_stale: bool,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO local_subtitle_segments (
            id, job_id, segment_index, start_ms, end_ms, source_text, translated_text,
            source_edited, translation_edited, translation_stale
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&segment.id)
    .bind(job_id)
    .bind(i64::try_from(index).context("subtitle index exceeds SQLite i64")?)
    .bind(to_i64(segment.start_ms, "segment start")?)
    .bind(to_i64(segment.end_ms, "segment end")?)
    .bind(source_text)
    .bind(translated_text)
    .bind(source_edited)
    .bind(translation_edited)
    .bind(translation_stale)
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
            if !seen.insert((source_text.clone(), target_text.clone())) {
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

        let segments = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, segment.id);
        assert_eq!(segments[0].source_text, "テスト");

        let stale = database
            .update_segment_text(
                &manifest.job_id,
                &segment.id,
                "手動修正".to_string(),
                Some("测试".to_string()),
            )
            .await
            .unwrap();
        assert!(stale.source_edited);
        assert!(!stale.translation_edited);
        assert!(stale.translation_stale);

        let edited = database
            .update_segment_text(
                &manifest.job_id,
                &segment.id,
                "手動修正".to_string(),
                Some("人工翻译".to_string()),
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
                "v1",
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
        database.delete_glossary(&glossaries[0].id).await.unwrap();
        database
            .ensure_builtin_glossary(
                "内置词表",
                "v1",
                vec![LocalGlossaryTermInput {
                    source_text: "前世".to_string(),
                    target_text: None,
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        assert!(database.list_glossaries().await.unwrap().is_empty());

        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
