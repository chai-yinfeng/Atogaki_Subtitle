//! Snapshot publication and explicit ASR candidate adoption share the existing
//! workspace transaction. Never commit a helper independently of its caller.
use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow};

use crate::{application::CandidateCueSet, domain::TranscriptSegment};

use super::{
    LocalDatabase, LocalSubtitleProvenanceRecord, LocalSubtitleSegmentRecord,
    subtitle_edits::{
        ensure_structure_editable, list_segments_in_transaction, persist_structure_segments,
    },
    to_i64,
};

impl LocalDatabase {
    /// Recover initial publication metadata only for untouched legacy rows.
    /// Explicit candidate adoption uses the strict transactional helper below.
    pub async fn assign_asr_cue_provenance(
        &self,
        job_id: &str,
        run_id: &str,
        candidates: &CandidateCueSet,
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin ASR cue provenance transaction")?;
        for candidate in &candidates.cues {
            sqlx::query(
                "UPDATE local_subtitle_provenance
                 SET source_kind = 'asr', source_run_id = ?, timed_unit_ids_json = ?,
                     timing_source = 'model', alignment_status = 'current',
                     updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
                 WHERE job_id = ? AND segment_id = ? AND source_kind = 'legacy'
                   AND source_revision = 0 AND timing_source = 'legacy'
                   AND EXISTS (SELECT 1 FROM local_subtitle_segments s
                               WHERE s.id = local_subtitle_provenance.segment_id AND s.job_id = ?
                                 AND s.source_edited = 0 AND s.timing_edited = 0
                                 AND s.source_text = ? AND s.start_ms = ? AND s.end_ms = ?)",
            )
            .bind(run_id)
            .bind(serde_json::to_string(&candidate.timed_unit_ids)?)
            .bind(job_id)
            .bind(&candidate.cue.id)
            .bind(job_id)
            .bind(&candidate.cue.source_text)
            .bind(to_i64(candidate.cue.start_ms, "candidate start")?)
            .bind(to_i64(candidate.cue.end_ms, "candidate end")?)
            .execute(&mut *tx)
            .await
            .context("failed to recover initial ASR provenance")?;
        }
        tx.commit()
            .await
            .context("failed to commit ASR cue provenance transaction")
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

    /// Replaces exactly the subtitle blocks fully contained in a user-selected
    /// range. The expected snapshot makes preview confirmation fail safely if
    /// another edit happened while the preview was open.
    pub async fn replace_segment_range(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
        expected_segments: &[LocalSubtitleSegmentRecord],
        candidates: &[TranscriptSegment],
        asr_provenance: Option<(&str, &CandidateCueSet)>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.replace_segment_range_with_revision(
            job_id,
            start_ms,
            end_ms,
            expected_segments,
            candidates,
            asr_provenance,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn replace_segment_range_with_revision(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
        expected_segments: &[LocalSubtitleSegmentRecord],
        candidates: &[TranscriptSegment],
        asr_provenance: Option<(&str, &CandidateCueSet)>,
        expected_revision: Option<i64>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        if start_ms < 0 || end_ms <= start_ms {
            return Err(anyhow!("invalid retranscription range"));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin selected-range replacement transaction")?;
        ensure_structure_editable(&mut tx, job_id).await?;
        if let Some(expected) = expected_revision {
            let current: i64 =
                sqlx::query_scalar("SELECT workspace_revision FROM local_jobs WHERE job_id = ?")
                    .bind(job_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if current != expected {
                return Err(anyhow!(
                    "subtitle workspace revision changed; refresh the comparison before adoption"
                ));
            }
        }
        let current = list_segments_in_transaction(&mut tx, job_id).await?;
        let affected = current
            .iter()
            .filter(|segment| segment.start_ms < end_ms && segment.end_ms > start_ms)
            .cloned()
            .collect::<Vec<_>>();
        if affected != expected_segments {
            return Err(anyhow!(
                "subtitles changed while the retranscription preview was open"
            ));
        }
        if affected
            .iter()
            .any(|segment| segment.start_ms < start_ms || segment.end_ms > end_ms)
        {
            return Err(anyhow!(
                "selected range cuts through an existing subtitle; align or expand the range first"
            ));
        }

        let mut replacements = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let candidate_start = to_i64(candidate.start_ms, "candidate start")?;
            let candidate_end = to_i64(candidate.end_ms, "candidate end")?;
            if candidate_start < start_ms
                || candidate_end > end_ms
                || candidate_end <= candidate_start
            {
                return Err(anyhow!(
                    "retranscription candidate is outside the selected range"
                ));
            }
            replacements.push(LocalSubtitleSegmentRecord {
                id: candidate.id.clone(),
                job_id: job_id.to_string(),
                segment_index: 0,
                start_ms: candidate_start,
                end_ms: candidate_end,
                source_text: candidate.source_text.clone(),
                translated_text: None,
                source_edited: false,
                translation_edited: false,
                translation_stale: false,
                timing_edited: false,
            });
        }
        replacements.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
        if replacements
            .windows(2)
            .any(|pair| pair[0].end_ms > pair[1].start_ms)
        {
            return Err(anyhow!("retranscription candidates overlap on one track"));
        }

        let mut updated = current
            .into_iter()
            .filter(|segment| segment.end_ms <= start_ms || segment.start_ms >= end_ms)
            .collect::<Vec<_>>();
        // Legacy ASR overlaps outside this scope are not caused by adoption.
        if replacements.iter().any(|candidate| {
            updated.iter().any(|existing| {
                candidate.start_ms < existing.end_ms && candidate.end_ms > existing.start_ms
            })
        }) {
            return Err(anyhow!(
                "selected-range replacement would overlap an adjacent subtitle"
            ));
        }
        updated.extend(replacements);
        updated.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
        persist_structure_segments(&mut tx, job_id, &mut updated).await?;
        if let Some((run_id, candidates)) = asr_provenance {
            assign_asr_cue_provenance_in_transaction(&mut tx, job_id, run_id, candidates).await?;
            sqlx::query("INSERT OR IGNORE INTO local_asr_candidate_adoptions(run_id) VALUES (?)")
                .bind(run_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit()
            .await
            .context("failed to commit selected-range replacement")?;
        Ok(updated)
    }

    pub(super) async fn merge_snapshot_segments_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        job_id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<()> {
        let existing_provenance = sqlx::query_as::<_, LocalSubtitleProvenanceRecord>(
            "SELECT segment_id, job_id, source_kind, source_run_id, timed_unit_ids_json,
                    timing_source, alignment_status, source_revision, translation_revision,
                    translation_dependency_stale, updated_at_unix
             FROM local_subtitle_provenance WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_all(&mut **tx)
        .await
        .context("failed to preserve subtitle provenance during snapshot sync")?
        .into_iter()
        .map(|item| (item.segment_id.clone(), item))
        .collect::<HashMap<_, _>>();
        let existing_dependencies = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT e.target_segment_id, e.context_segment_id, e.context_source_revision
             FROM local_translation_dependency_edges e
             JOIN local_subtitle_segments s ON s.id = e.target_segment_id
             WHERE s.job_id = ?",
        )
        .bind(job_id)
        .fetch_all(&mut **tx)
        .await
        .context("failed to preserve translation dependencies during snapshot sync")?;
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
            if let Some(provenance) = existing_provenance.get(&segment.id) {
                sqlx::query(
                    "UPDATE local_subtitle_provenance
                     SET source_kind = ?, source_run_id = ?, timed_unit_ids_json = ?,
                         timing_source = ?, alignment_status = ?, source_revision = ?,
                         translation_revision = ?, translation_dependency_stale = ?,
                         updated_at_unix = ?
                     WHERE segment_id = ?",
                )
                .bind(&provenance.source_kind)
                .bind(&provenance.source_run_id)
                .bind(&provenance.timed_unit_ids_json)
                .bind(&provenance.timing_source)
                .bind(&provenance.alignment_status)
                .bind(provenance.source_revision)
                .bind(provenance.translation_revision)
                .bind(provenance.translation_dependency_stale)
                .bind(provenance.updated_at_unix)
                .bind(&segment.id)
                .execute(&mut **tx)
                .await
                .context("failed to restore subtitle provenance during snapshot sync")?;
            }
        }
        let retained_ids = segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect::<HashSet<_>>();
        for (target_id, context_id, revision) in existing_dependencies {
            if retained_ids.contains(target_id.as_str())
                && retained_ids.contains(context_id.as_str())
            {
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
                .context("failed to restore translation dependency during snapshot sync")?;
            }
        }

        Ok(())
    }
}

async fn assign_asr_cue_provenance_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    job_id: &str,
    run_id: &str,
    candidates: &CandidateCueSet,
) -> Result<()> {
    for candidate in &candidates.cues {
        let timed_unit_ids_json = serde_json::to_string(&candidate.timed_unit_ids)
            .context("failed to encode cue evidence ids")?;
        let result = sqlx::query(
            "UPDATE local_subtitle_provenance
             SET source_kind = 'asr', source_run_id = ?, timed_unit_ids_json = ?,
                 timing_source = 'model', alignment_status = 'current',
                 updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE job_id = ? AND segment_id = ?",
        )
        .bind(run_id)
        .bind(timed_unit_ids_json)
        .bind(job_id)
        .bind(&candidate.cue.id)
        .execute(&mut **tx)
        .await
        .context("failed to attach ASR cue provenance")?;
        if result.rows_affected() != 1 {
            return Err(anyhow!(
                "ASR candidate cue is not in the current workspace: {}",
                candidate.cue.id
            ));
        }
    }
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
