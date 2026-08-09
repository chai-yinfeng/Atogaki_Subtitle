#![allow(dead_code)]

pub mod models;

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::{
    domain::TranscriptSegment,
    infrastructure::db::models::{
        FileRecord, GlossaryRecord, GlossaryTermRecord, JobEventRecord, JobRecord,
        SubtitleSegmentRecord, UserRecord,
    },
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct CreateFile {
    pub user_id: Uuid,
    pub job_id: Option<Uuid>,
    pub kind: String,
    pub path: String,
    pub original_name: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("failed to connect to Postgres")?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        MIGRATOR
            .run(&self.pool)
            .await
            .context("failed to run database migrations")
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        password_hash: Option<&str>,
    ) -> Result<UserRecord> {
        let user = sqlx::query_as::<_, UserRecord>(
            "INSERT INTO users (id, email, display_name, password_hash)
             VALUES ($1, lower($2), $3, $4)
             RETURNING id, email, display_name, password_hash, created_at, updated_at",
        )
        .bind(Uuid::new_v4())
        .bind(email.trim())
        .bind(display_name.trim())
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
        .context("failed to create user")?;

        Ok(user)
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<UserRecord>> {
        let user = sqlx::query_as::<_, UserRecord>(
            "SELECT id, email, display_name, password_hash, created_at, updated_at
             FROM users
             WHERE email = lower($1)",
        )
        .bind(email.trim())
        .fetch_optional(&self.pool)
        .await
        .context("failed to find user by email")?;

        Ok(user)
    }

    pub async fn create_file(&self, file: CreateFile) -> Result<FileRecord> {
        let record = sqlx::query_as::<_, FileRecord>(
            "INSERT INTO files (
                id, user_id, job_id, kind, path, original_name, mime_type, size_bytes
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, user_id, job_id, kind, path, original_name, mime_type, size_bytes, created_at",
        )
        .bind(Uuid::new_v4())
        .bind(file.user_id)
        .bind(file.job_id)
        .bind(file.kind)
        .bind(file.path)
        .bind(file.original_name)
        .bind(file.mime_type)
        .bind(file.size_bytes)
        .fetch_one(&self.pool)
        .await
        .context("failed to create file record")?;

        Ok(record)
    }

    pub async fn create_job(
        &self,
        user_id: Uuid,
        local_job_id: &str,
        storage_dir: &Path,
    ) -> Result<JobRecord> {
        let job = sqlx::query_as::<_, JobRecord>(
            "INSERT INTO jobs (id, user_id, local_job_id, status, message, storage_dir)
             VALUES ($1, $2, $3, 'queued', 'queued', $4)
             RETURNING id, user_id, local_job_id, status, message, storage_dir,
                input_file_id, render_output_file_id, error_message, created_at, updated_at,
                started_at, completed_at",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(local_job_id)
        .bind(storage_dir.display().to_string())
        .fetch_one(&self.pool)
        .await
        .context("failed to create job")?;

        self.append_job_event(job.id, &job.status, &job.message, None)
            .await?;
        Ok(job)
    }

    pub async fn list_jobs_for_user(&self, user_id: Uuid) -> Result<Vec<JobRecord>> {
        let jobs = sqlx::query_as::<_, JobRecord>(
            "SELECT id, user_id, local_job_id, status, message, storage_dir,
                input_file_id, render_output_file_id, error_message, created_at, updated_at,
                started_at, completed_at
             FROM jobs
             WHERE user_id = $1
             ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list jobs")?;

        Ok(jobs)
    }

    pub async fn get_job_for_user(&self, job_id: Uuid, user_id: Uuid) -> Result<Option<JobRecord>> {
        let job = sqlx::query_as::<_, JobRecord>(
            "SELECT id, user_id, local_job_id, status, message, storage_dir,
                input_file_id, render_output_file_id, error_message, created_at, updated_at,
                started_at, completed_at
             FROM jobs
             WHERE id = $1 AND user_id = $2",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to get job")?;

        Ok(job)
    }

    pub async fn update_job_status(
        &self,
        job_id: Uuid,
        status: &str,
        message: &str,
        error_message: Option<&str>,
    ) -> Result<JobRecord> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;

        let job = sqlx::query_as::<_, JobRecord>(
            "UPDATE jobs
             SET status = $2,
                 message = $3,
                 error_message = $4,
                 updated_at = now(),
                 started_at = CASE WHEN started_at IS NULL AND $2 <> 'queued' THEN now() ELSE started_at END,
                 completed_at = CASE WHEN $2 IN ('done', 'failed', 'cancelled') THEN now() ELSE completed_at END
             WHERE id = $1
             RETURNING id, user_id, local_job_id, status, message, storage_dir,
                input_file_id, render_output_file_id, error_message, created_at, updated_at,
                started_at, completed_at",
        )
        .bind(job_id)
        .bind(status)
        .bind(message)
        .bind(error_message)
        .fetch_one(&mut *tx)
        .await
        .with_context(|| format!("failed to update job status for {job_id}"))?;

        insert_job_event(&mut tx, job.id, status, message, error_message).await?;
        tx.commit().await.context("failed to commit transaction")?;

        Ok(job)
    }

    pub async fn append_job_event(
        &self,
        job_id: Uuid,
        status: &str,
        message: &str,
        error_message: Option<&str>,
    ) -> Result<JobEventRecord> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;
        let event = insert_job_event(&mut tx, job_id, status, message, error_message).await?;
        tx.commit().await.context("failed to commit transaction")?;
        Ok(event)
    }

    pub async fn list_job_events_for_user(
        &self,
        job_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<JobEventRecord>> {
        let events = sqlx::query_as::<_, JobEventRecord>(
            "SELECT e.id, e.job_id, e.status, e.message, e.error_message, e.created_at
             FROM job_events e
             JOIN jobs j ON j.id = e.job_id
             WHERE e.job_id = $1 AND j.user_id = $2
             ORDER BY e.created_at ASC",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list job events")?;

        Ok(events)
    }

    pub async fn replace_segments(
        &self,
        job_id: Uuid,
        segments: &[TranscriptSegment],
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;

        sqlx::query("DELETE FROM subtitle_segments WHERE job_id = $1")
            .bind(job_id)
            .execute(&mut *tx)
            .await
            .context("failed to clear existing subtitle segments")?;

        for (index, segment) in segments.iter().enumerate() {
            sqlx::query(
                "INSERT INTO subtitle_segments (
                    id, job_id, segment_index, start_ms, end_ms, source_text, translated_text
                 )
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(Uuid::new_v4())
            .bind(job_id)
            .bind(i32::try_from(index).context("segment index exceeds i32")?)
            .bind(ms_to_i64(segment.start_ms)?)
            .bind(ms_to_i64(segment.end_ms)?)
            .bind(&segment.source_text)
            .bind(&segment.translated_text)
            .execute(&mut *tx)
            .await
            .context("failed to insert subtitle segment")?;
        }

        tx.commit().await.context("failed to commit transaction")
    }

    pub async fn list_segments_for_user(
        &self,
        job_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<SubtitleSegmentRecord>> {
        let segments = sqlx::query_as::<_, SubtitleSegmentRecord>(
            "SELECT s.id, s.job_id, s.segment_index, s.start_ms, s.end_ms,
                s.source_text, s.translated_text, s.created_at, s.updated_at
             FROM subtitle_segments s
             JOIN jobs j ON j.id = s.job_id
             WHERE s.job_id = $1 AND j.user_id = $2
             ORDER BY s.segment_index ASC",
        )
        .bind(job_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list subtitle segments")?;

        Ok(segments)
    }

    pub async fn create_glossary(&self, user_id: Uuid, name: &str) -> Result<GlossaryRecord> {
        let glossary = sqlx::query_as::<_, GlossaryRecord>(
            "INSERT INTO glossaries (id, user_id, name)
             VALUES ($1, $2, $3)
             RETURNING id, user_id, name, created_at, updated_at",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(name.trim())
        .fetch_one(&self.pool)
        .await
        .context("failed to create glossary")?;

        Ok(glossary)
    }

    pub async fn list_glossaries_for_user(&self, user_id: Uuid) -> Result<Vec<GlossaryRecord>> {
        let glossaries = sqlx::query_as::<_, GlossaryRecord>(
            "SELECT id, user_id, name, created_at, updated_at
             FROM glossaries
             WHERE user_id = $1
             ORDER BY name ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list glossaries")?;

        Ok(glossaries)
    }

    pub async fn replace_glossary_terms(
        &self,
        glossary_id: Uuid,
        terms: &[(String, Option<String>)],
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;

        sqlx::query("DELETE FROM glossary_terms WHERE glossary_id = $1")
            .bind(glossary_id)
            .execute(&mut *tx)
            .await
            .context("failed to clear glossary terms")?;

        for (source, target) in terms {
            sqlx::query(
                "INSERT INTO glossary_terms (id, glossary_id, source_text, target_text)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(Uuid::new_v4())
            .bind(glossary_id)
            .bind(source.trim())
            .bind(target.as_ref().map(|value| value.trim()))
            .execute(&mut *tx)
            .await
            .context("failed to insert glossary term")?;
        }

        tx.commit().await.context("failed to commit transaction")
    }

    pub async fn list_glossary_terms_for_user(
        &self,
        glossary_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<GlossaryTermRecord>> {
        let terms = sqlx::query_as::<_, GlossaryTermRecord>(
            "SELECT t.id, t.glossary_id, t.source_text, t.target_text, t.created_at, t.updated_at
             FROM glossary_terms t
             JOIN glossaries g ON g.id = t.glossary_id
             WHERE t.glossary_id = $1 AND g.user_id = $2
             ORDER BY t.source_text ASC",
        )
        .bind(glossary_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list glossary terms")?;

        Ok(terms)
    }
}

async fn insert_job_event(
    tx: &mut Transaction<'_, Postgres>,
    job_id: Uuid,
    status: &str,
    message: &str,
    error_message: Option<&str>,
) -> Result<JobEventRecord> {
    let event = sqlx::query_as::<_, JobEventRecord>(
        "INSERT INTO job_events (id, job_id, status, message, error_message)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, job_id, status, message, error_message, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(job_id)
    .bind(status)
    .bind(message)
    .bind(error_message)
    .fetch_one(&mut **tx)
    .await
    .context("failed to insert job event")?;

    Ok(event)
}

fn ms_to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| anyhow!("millisecond value exceeds i64"))
}
