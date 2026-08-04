use std::{collections::HashMap, path::Path};

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
    pub translation_edited: bool,
    pub translation_stale: bool,
}

#[derive(Debug, Clone)]
pub struct LocalMachineTranslation {
    pub segment_id: String,
    pub source_text: String,
    pub translated_text: String,
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
            "SELECT job_id, storage_dir, input_path, render_output_path, status, message,
                error_message, created_at_unix, updated_at_unix
             FROM local_jobs
             ORDER BY updated_at_unix DESC, job_id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list local tasks")
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<LocalJobRecord>> {
        sqlx::query_as::<_, LocalJobRecord>(
            "SELECT job_id, storage_dir, input_path, render_output_path, status, message,
                error_message, created_at_unix, updated_at_unix
             FROM local_jobs
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local task")
    }

    pub async fn list_segments(&self, job_id: &str) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
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
            "SELECT id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
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
                 SET zh_text = ?, translation_edited = 0, translation_stale = 0
                 WHERE job_id = ? AND id = ? AND ja_text = ?",
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

    pub async fn update_segment_text(
        &self,
        job_id: &str,
        segment_id: &str,
        ja_text: String,
        zh_text: Option<String>,
    ) -> Result<LocalSubtitleSegmentRecord> {
        let ja_text = ja_text.trim().to_string();
        if ja_text.is_empty() {
            return Err(anyhow!("Japanese subtitle text cannot be empty"));
        }
        let zh_text = zh_text
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local subtitle edit transaction")?;
        let current = sqlx::query_as::<_, LocalSubtitleSegmentRecord>(
            "SELECT id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
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

        let source_changed = current.ja_text != ja_text;
        let translation_changed = current.zh_text != zh_text;
        let source_edited = current.source_edited || source_changed;
        let translation_edited = current.translation_edited || translation_changed;
        let translation_stale = if translation_changed {
            false
        } else if source_changed {
            current.zh_text.is_some()
        } else {
            current.translation_stale
        };

        sqlx::query(
            "UPDATE local_subtitle_segments
             SET ja_text = ?, zh_text = ?, source_edited = ?, translation_edited = ?,
                 translation_stale = ?
             WHERE job_id = ? AND id = ?",
        )
        .bind(&ja_text)
        .bind(&zh_text)
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
            "SELECT id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
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
                segment.ja_text.as_str(),
                segment.zh_text.as_deref(),
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
            "SELECT id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
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
            let ja_text = previous
                .filter(|segment| segment.source_edited)
                .map(|segment| segment.ja_text.as_str())
                .unwrap_or(&segment.ja_text);
            let incoming_translation = segment.zh_text.as_deref();
            let (zh_text, translation_edited, translation_stale) = match previous {
                Some(previous) if previous.translation_edited => (
                    previous.zh_text.as_deref(),
                    true,
                    previous.translation_stale || ja_text != previous.ja_text,
                ),
                Some(previous) if incoming_translation.is_none() && previous.zh_text.is_some() => (
                    previous.zh_text.as_deref(),
                    false,
                    previous.translation_stale || ja_text != previous.ja_text,
                ),
                _ => (incoming_translation, false, segment.translation_stale),
            };
            insert_segment(
                tx,
                job_id,
                index,
                segment,
                ja_text,
                zh_text,
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
    ja_text: &str,
    zh_text: Option<&str>,
    source_edited: bool,
    translation_edited: bool,
    translation_stale: bool,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO local_subtitle_segments (
            id, job_id, segment_index, start_ms, end_ms, ja_text, zh_text,
            source_edited, translation_edited, translation_stale
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&segment.id)
    .bind(job_id)
    .bind(i64::try_from(index).context("subtitle index exceeds SQLite i64")?)
    .bind(to_i64(segment.start_ms, "segment start")?)
    .bind(to_i64(segment.end_ms, "segment end")?)
    .bind(ja_text)
    .bind(zh_text)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{LocalDatabase, LocalMachineTranslation};
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
        assert_eq!(segments[0].ja_text, "テスト");

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
        regenerated.ja_text = "再生成された文".to_string();
        regenerated.zh_text = Some("重新生成的翻译".to_string());
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![regenerated],
            })
            .await
            .unwrap();
        let preserved = database.list_segments(&edited.job_id).await.unwrap();
        assert_eq!(preserved[0].ja_text, "手動修正");
        assert_eq!(preserved[0].zh_text.as_deref(), Some("人工翻译"));
        assert!(preserved[0].source_edited);
        assert!(preserved[0].translation_edited);

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
        let manifest = JobManifest::new(&job, None, None);
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
                        source_text: first.ja_text.clone(),
                        translated_text: "第一句话".to_string(),
                    },
                    LocalMachineTranslation {
                        segment_id: second.id.clone(),
                        source_text: second.ja_text.clone(),
                        translated_text: "下一句话".to_string(),
                    },
                ],
            )
            .await
            .unwrap();
        let translated = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(translated[0].zh_text.as_deref(), Some("第一句话"));
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
        assert_eq!(preserved[0].zh_text.as_deref(), Some("第一句话"));

        let mut changed = first.clone();
        changed.ja_text = "変更された文".to_string();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![changed, second],
            })
            .await
            .unwrap();
        let stale = database.list_segments(&manifest.job_id).await.unwrap();
        assert_eq!(stale[0].zh_text.as_deref(), Some("第一句话"));
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
        assert_eq!(unchanged[0].zh_text.as_deref(), Some("第一句话"));

        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
