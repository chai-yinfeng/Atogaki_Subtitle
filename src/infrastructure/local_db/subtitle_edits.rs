//! Manual subtitle edits and the structure transaction shared with adoption.
//! Keep transaction ownership at each operation; helpers never begin or commit.
use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow};

use super::{LocalDatabase, LocalSubtitleProvenanceRecord, LocalSubtitleSegmentRecord};

impl LocalDatabase {
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

    pub async fn edit_subtitle_block(
        &self,
        job_id: &str,
        before: &[LocalSubtitleSegmentRecord],
        remove_id: Option<&str>,
        insertion: Option<LocalSubtitleSegmentRecord>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let mut segments = list_segments_in_transaction(&mut tx, job_id).await?;
        if segments != before {
            return Err(anyhow!("字幕已变化，请重新读取后再操作"));
        }
        // Existing translations retain their frozen inputs on insertion. On deletion,
        // only recorded consumers of the removed context can become stale.
        let affected: HashSet<String> = if let Some(id) = remove_id {
            sqlx::query_scalar::<_, String>(
                "SELECT DISTINCT s.id FROM local_translation_dependency_edges e
                 JOIN local_subtitle_segments s ON s.id = e.target_segment_id
                 WHERE e.context_segment_id = ? AND s.job_id = ?
                   AND s.translated_text IS NOT NULL AND s.translation_edited = 0",
            )
            .bind(id)
            .bind(job_id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect()
        } else {
            HashSet::new()
        };
        match (remove_id, insertion) {
            (Some(id), None) => {
                let index = segments
                    .iter()
                    .position(|s| s.id == id)
                    .ok_or_else(|| anyhow!("字幕块不存在"))?;
                segments.remove(index);
            }
            (None, Some(mut block)) => {
                if block.job_id != job_id || segments.iter().any(|s| s.id == block.id) {
                    return Err(anyhow!("invalid new subtitle identity"));
                }
                if block.start_ms < 0
                    || block.end_ms <= block.start_ms
                    || segments
                        .iter()
                        .any(|s| s.start_ms < block.end_ms && s.end_ms > block.start_ms)
                {
                    return Err(anyhow!("插入范围无效或与已有字幕重叠，请选择空白区间"));
                }
                block.source_edited = true;
                block.translation_edited = block
                    .translated_text
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty());
                block.translation_stale = false;
                block.timing_edited = true;
                segments.push(block);
                segments.sort_by_key(|s| (s.start_ms, s.end_ms));
            }
            _ => return Err(anyhow!("choose exactly one subtitle block operation")),
        }
        for s in &mut segments {
            if affected.contains(&s.id) {
                s.translation_stale = true;
            }
        }
        persist_structure_segments(&mut tx, job_id, &mut segments).await?;
        tx.commit().await?;
        Ok(segments)
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

pub(super) async fn ensure_structure_editable(
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

pub(super) async fn list_segments_in_transaction(
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

pub(super) async fn persist_structure_segments(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
    segments: &mut [LocalSubtitleSegmentRecord],
) -> Result<()> {
    let learning_links = sqlx::query_as::<_, (String, String)>("SELECT id, segment_id FROM local_learning_occurrences WHERE job_id = ? AND segment_id IS NOT NULL")
        .bind(job_id).fetch_all(&mut **tx).await?;
    let previous_provenance = sqlx::query_as::<_, LocalSubtitleProvenanceRecord>(
        "SELECT segment_id, job_id, source_kind, source_run_id, timed_unit_ids_json,
                timing_source, alignment_status, source_revision, translation_revision,
                translation_dependency_stale, updated_at_unix
         FROM local_subtitle_provenance WHERE job_id = ?",
    )
    .bind(job_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to preserve subtitle provenance before a structure edit")?
    .into_iter()
    .map(|item| (item.segment_id.clone(), item))
    .collect::<HashMap<_, _>>();
    let previous_dependencies = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT e.target_segment_id, e.context_segment_id, e.context_source_revision
         FROM local_translation_dependency_edges e
         JOIN local_subtitle_segments s ON s.id = e.target_segment_id
         WHERE s.job_id = ?",
    )
    .bind(job_id)
    .fetch_all(&mut **tx)
    .await
    .context("failed to preserve translation dependencies before a structure edit")?;
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
        if let Some(previous) = previous_provenance.get(&segment.id) {
            sqlx::query(
                "UPDATE local_subtitle_provenance
                 SET source_kind = ?, source_run_id = ?, timed_unit_ids_json = ?,
                     timing_source = ?, alignment_status = ?, source_revision = ?,
                     translation_revision = ?, translation_dependency_stale = ?,
                     updated_at_unix = ?
                 WHERE segment_id = ?",
            )
            .bind(&previous.source_kind)
            .bind(&previous.source_run_id)
            .bind(&previous.timed_unit_ids_json)
            .bind(&previous.timing_source)
            .bind(&previous.alignment_status)
            .bind(previous.source_revision)
            .bind(previous.translation_revision)
            .bind(previous.translation_dependency_stale)
            .bind(previous.updated_at_unix)
            .bind(&segment.id)
            .execute(&mut **tx)
            .await
            .context("failed to restore subtitle provenance after a structure edit")?;
        } else {
            sqlx::query(
                "UPDATE local_subtitle_provenance
                 SET source_kind = 'structural', alignment_status = 'stale',
                     updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
                 WHERE segment_id = ?",
            )
            .bind(&segment.id)
            .execute(&mut **tx)
            .await
            .context("failed to mark structural subtitle provenance")?;
        }
    }
    for (occurrence_id, segment_id) in learning_links {
        if ids.contains(&segment_id) {
            sqlx::query("UPDATE local_learning_occurrences SET segment_id = ? WHERE id = ?")
                .bind(segment_id)
                .bind(occurrence_id)
                .execute(&mut **tx)
                .await?;
        }
    }
    for (target_id, context_id, revision) in previous_dependencies {
        if ids.contains(&target_id) && ids.contains(&context_id) {
            sqlx::query(
                "INSERT INTO local_translation_dependency_edges (
                    target_segment_id, context_segment_id, context_source_revision
                 ) VALUES (?, ?, ?)",
            )
            .bind(target_id)
            .bind(context_id)
            .bind(revision)
            .execute(&mut **tx)
            .await
            .context("failed to restore translation dependency after a structure edit")?;
        }
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
