use std::path::Path;

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
    pub storage_dir: String,
    pub input_path: Option<String>,
    pub render_output_path: Option<String>,
    pub status: String,
    pub message: String,
    pub error_message: Option<String>,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalSubtitleSegmentRecord {
    pub id: String,
    pub job_id: String,
    pub segment_index: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub ja_text: String,
    pub zh_text: Option<String>,
    pub source_edited: bool,
    pub translation_stale: bool,
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
                error_message, created_at_unix, updated_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
                storage_dir = excluded.storage_dir,
                input_path = excluded.input_path,
                render_output_path = excluded.render_output_path,
                status = excluded.status,
                message = excluded.message,
                error_message = excluded.error_message,
                updated_at_unix = excluded.updated_at_unix",
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
        .bind(to_i64(manifest.created_at_unix, "created_at_unix")?)
        .bind(to_i64(manifest.updated_at_unix, "updated_at_unix")?)
        .execute(&mut *tx)
        .await
        .context("failed to upsert local task")?;

        self.replace_segments_in_transaction(&mut tx, &manifest.job_id, &snapshot.segments)
            .await?;
        tx.commit()
            .await
            .context("failed to commit local task transaction")
    }

    pub async fn list_jobs(&self) -> Result<Vec<LocalJobRecord>> {
        sqlx::query_as::<_, LocalJobRecord>(
            "SELECT job_id, storage_dir, input_path, render_output_path, status, message,
                error_message, created_at_unix, updated_at_unix
             FROM local_jobs
             ORDER BY updated_at_unix DESC, job_id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list local tasks")
    }

    pub async fn list_segments(&self, job_id: &str) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
                source_edited, translation_stale
             FROM local_subtitle_segments
             WHERE job_id = ?
             ORDER BY segment_index ASC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list local subtitle segments")
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
            sqlx::query(
                "INSERT INTO local_subtitle_segments (
                    id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
                    source_edited, translation_stale
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&segment.id)
            .bind(job_id)
            .bind(i64::try_from(index).context("subtitle index exceeds SQLite i64")?)
            .bind(to_i64(segment.start_ms, "segment start")?)
            .bind(to_i64(segment.end_ms, "segment end")?)
            .bind(&segment.ja_text)
            .bind(&segment.zh_text)
            .bind(segment.source_edited)
            .bind(segment.translation_stale)
            .execute(&mut **tx)
            .await
            .context("failed to insert local subtitle segment")?;
        }

        Ok(())
    }
}

fn to_i64(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| anyhow!("{field} exceeds SQLite i64"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::LocalDatabase;
    use crate::{
        application::{
            job_manifest::JobManifest, job_snapshot::JobSnapshot, job_status::JobStatus,
        },
        domain::TranscriptSegment,
        infrastructure::job_store::Job,
    };

    #[tokio::test]
    async fn syncs_task_metadata_and_subtitle_segments() {
        let root =
            std::env::temp_dir().join(format!("atogaki-local-db-test-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let mut manifest = JobManifest::new(&job, None, None);
        manifest.mark(JobStatus::Queued);
        let segment = TranscriptSegment::new(0, 1_000, "テスト".to_string());
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
        assert_eq!(segments[0].ja_text, "テスト");

        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
