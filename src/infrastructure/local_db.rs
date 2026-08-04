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
    pub storage_dir: String,
    pub input_path: Option<String>,
    pub render_output_path: Option<String>,
    pub status: String,
    pub message: String,
    pub error_message: Option<String>,
    pub glossary_id: Option<String>,
    pub glossary_name: Option<String>,
    pub glossary_snapshot_path: Option<String>,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalGlossaryRecord {
    pub id: String,
    pub name: String,
    pub term_count: i64,
    pub prompt_term_count: i64,
    pub correction_count: i64,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LocalGlossaryTermRecord {
    pub id: String,
    pub glossary_id: String,
    pub source_text: String,
    pub target_text: Option<String>,
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
                error_message, glossary_id, glossary_name, glossary_snapshot_path,
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
            "SELECT job_id, storage_dir, input_path, render_output_path, status, message,
                error_message, glossary_id, glossary_name, glossary_snapshot_path,
                created_at_unix, updated_at_unix
             FROM local_jobs
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to read local task")
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

    pub async fn ensure_builtin_glossary(&self, name: &str, file_text: &str) -> Result<()> {
        let name = normalized_glossary_name(name)?;
        let setting_key = format!("builtin_glossary_seeded:{name}");
        if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_settings WHERE key = ?")
            .bind(&setting_key)
            .fetch_one(&self.pool)
            .await
            .context("failed to check built-in glossary seed state")?
            > 0
        {
            return Ok(());
        }

        if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_glossaries WHERE name = ?")
            .bind(&name)
            .fetch_one(&self.pool)
            .await
            .context("failed to check built-in glossary")?
            == 0
        {
            let mut terms = file_text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| {
                    line.split_once("=>")
                        .or_else(|| line.split_once('\t'))
                        .map(|(source, target)| LocalGlossaryTermInput {
                            source_text: source.trim().to_string(),
                            target_text: Some(target.trim().to_string()),
                        })
                        .unwrap_or_else(|| LocalGlossaryTermInput {
                            source_text: line.to_string(),
                            target_text: None,
                        })
                })
                .collect::<Vec<_>>();
            let mut seen = HashSet::new();
            terms.retain(|term| seen.insert((term.source_text.clone(), term.target_text.clone())));
            self.save_glossary(None, name, terms).await?;
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
            "SELECT g.id, g.name, COUNT(t.id) AS term_count,
                COALESCE(SUM(CASE WHEN t.id IS NOT NULL AND t.target_text IS NULL THEN 1 ELSE 0 END), 0) AS prompt_term_count,
                COALESCE(SUM(CASE WHEN t.target_text IS NOT NULL THEN 1 ELSE 0 END), 0) AS correction_count,
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
            "SELECT g.id, g.name, COUNT(t.id) AS term_count,
                COALESCE(SUM(CASE WHEN t.id IS NOT NULL AND t.target_text IS NULL THEN 1 ELSE 0 END), 0) AS prompt_term_count,
                COALESCE(SUM(CASE WHEN t.target_text IS NOT NULL THEN 1 ELSE 0 END), 0) AS correction_count,
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
            "SELECT id, glossary_id, source_text, target_text
             FROM local_glossary_terms
             WHERE glossary_id = ?
             ORDER BY CASE WHEN target_text IS NULL THEN 0 ELSE 1 END,
                lower(source_text), id",
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
            "INSERT INTO local_glossaries (id, name, created_at_unix, updated_at_unix)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at_unix = excluded.updated_at_unix",
        )
        .bind(&glossary_id)
        .bind(&name)
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
                    (id, glossary_id, source_text, target_text)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&glossary_id)
            .bind(term.source_text)
            .bind(term.target_text)
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
                 SET ja_text = ?, source_edited = 1,
                     translation_stale = CASE WHEN zh_text IS NULL THEN translation_stale ELSE 1 END
                 WHERE job_id = ? AND id = ? AND ja_text = ?",
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
            if !seen.insert((source_text.clone(), target_text.clone())) {
                return Err(anyhow!("duplicate glossary term: {source_text}"));
            }
            Ok(LocalGlossaryTermInput {
                source_text,
                target_text,
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
                "# grouped terms\n前世\n前世\nナブナ => n-buna\nナブナ => n-buna\n",
            )
            .await
            .unwrap();
        let glossaries = database.list_glossaries().await.unwrap();
        assert_eq!(glossaries.len(), 1);
        assert_eq!(glossaries[0].term_count, 2);
        assert_eq!(glossaries[0].prompt_term_count, 1);
        assert_eq!(glossaries[0].correction_count, 1);
        database.delete_glossary(&glossaries[0].id).await.unwrap();
        database
            .ensure_builtin_glossary("内置词表", "前世\n")
            .await
            .unwrap();
        assert!(database.list_glossaries().await.unwrap().is_empty());

        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
