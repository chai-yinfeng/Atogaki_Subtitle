//! Trusted, transaction-captured undo state. No metadata is deserialized from IPC.
use super::subtitle_edits::{
    edit_block_in_transaction, ensure_structure_editable, list_segments_in_transaction,
    merge_in_transaction, normalized_optional_subtitle_text, normalized_subtitle_text,
    persist_structure_segments, split_in_transaction, timing_in_transaction,
};
use super::subtitle_publication::replace_range_in_transaction;
use super::{LocalDatabase, LocalSubtitleProvenanceRecord, LocalSubtitleSegmentRecord};
use crate::{application::CandidateCueSet, domain::TranscriptSegment};
use anyhow::{Result, ensure};
use sqlx::{FromRow, Sqlite, Transaction};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
struct Dependency {
    target_segment_id: String,
    context_segment_id: String,
    context_source_revision: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    job_id: String,
    revision: i64,
    segments: Vec<LocalSubtitleSegmentRecord>,
    provenance: Vec<LocalSubtitleProvenanceRecord>,
    dependencies: Vec<Dependency>,
}
impl State {
    fn heap_bytes(&self) -> usize {
        self.job_id.capacity()
            + self.segments.capacity() * std::mem::size_of::<LocalSubtitleSegmentRecord>()
            + self.provenance.capacity() * std::mem::size_of::<LocalSubtitleProvenanceRecord>()
            + self.dependencies.capacity() * std::mem::size_of::<Dependency>()
            + self
                .segments
                .iter()
                .map(|cue| {
                    cue.id.capacity()
                        + cue.job_id.capacity()
                        + cue.source_text.capacity()
                        + cue.translated_text.as_ref().map_or(0, String::capacity)
                })
                .sum::<usize>()
            + self
                .provenance
                .iter()
                .map(|cue| {
                    cue.segment_id.capacity()
                        + cue.job_id.capacity()
                        + cue.source_kind.capacity()
                        + cue.source_run_id.as_ref().map_or(0, String::capacity)
                        + cue.timed_unit_ids_json.capacity()
                        + cue.timing_source.capacity()
                        + cue.alignment_status.capacity()
                })
                .sum::<usize>()
            + self
                .dependencies
                .iter()
                .map(|edge| edge.target_segment_id.capacity() + edge.context_segment_id.capacity())
                .sum::<usize>()
    }
    fn context_stale_ids(&self) -> Vec<String> {
        self.provenance
            .iter()
            .filter(|cue| cue.translation_dependency_stale)
            .map(|cue| cue.segment_id.clone())
            .collect()
    }
}
/// Opaque backend receipt, constructible only by a committed repository edit.
#[derive(Debug, Clone)]
pub struct LocalStructureEdit {
    id: String,
    before: State,
    after: State,
}
/// Actual committed restore result, used to safely rebind a continuous older edit.
#[derive(Debug, Clone)]
pub struct LocalStructureUndoResult {
    undone_id: String,
    state: State,
}
impl LocalStructureEdit {
    /// Owned snapshot allocation estimate, including spare Vec/String capacity.
    /// Excludes allocator bookkeeping, database caches and transient restore state.
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.id.capacity()
            + self.before.heap_bytes()
            + self.after.heap_bytes()
    }
    pub fn follows(&self, previous: &Self) -> bool {
        self.before == previous.after
    }
    pub fn token(&self) -> &str {
        &self.id
    }
    pub fn job_id(&self) -> &str {
        &self.after.job_id
    }
    pub fn segments(&self) -> &[LocalSubtitleSegmentRecord] {
        &self.after.segments
    }
    pub fn workspace_revision(&self) -> i64 {
        self.after.revision
    }
    pub fn translation_context_stale_ids(&self) -> Vec<String> {
        self.after.context_stale_ids()
    }
    pub fn rebase_after_undo(
        &mut self,
        undone: &Self,
        result: &LocalStructureUndoResult,
    ) -> Result<()> {
        ensure!(
            result.undone_id == undone.id && self.after == undone.before,
            "structure undo history is not continuous"
        );
        self.after = result.state.clone();
        Ok(())
    }
}
impl LocalStructureUndoResult {
    pub fn segments(&self) -> &[LocalSubtitleSegmentRecord] {
        &self.state.segments
    }
    pub fn workspace_revision(&self) -> i64 {
        self.state.revision
    }
    pub fn translation_context_stale_ids(&self) -> Vec<String> {
        self.state.context_stale_ids()
    }
}

async fn capture(tx: &mut Transaction<'_, Sqlite>, job_id: &str) -> Result<State> {
    let revision = sqlx::query_scalar("SELECT workspace_revision FROM local_jobs WHERE job_id=?")
        .bind(job_id)
        .fetch_one(&mut **tx)
        .await?;
    let segments = list_segments_in_transaction(tx, job_id).await?;
    let provenance = sqlx::query_as("SELECT segment_id,job_id,source_kind,source_run_id,timed_unit_ids_json,timing_source,alignment_status,source_revision,translation_revision,translation_dependency_stale,updated_at_unix FROM local_subtitle_provenance WHERE job_id=? ORDER BY segment_id")
        .bind(job_id).fetch_all(&mut **tx).await?;
    let dependencies = sqlx::query_as("SELECT e.target_segment_id,e.context_segment_id,e.context_source_revision FROM local_translation_dependency_edges e JOIN local_subtitle_segments s ON s.id=e.target_segment_id WHERE s.job_id=? ORDER BY e.target_segment_id,e.context_segment_id")
        .bind(job_id).fetch_all(&mut **tx).await?;
    Ok(State {
        job_id: job_id.into(),
        revision,
        segments,
        provenance,
        dependencies,
    })
}
async fn advance(tx: &mut Transaction<'_, Sqlite>, job_id: &str) -> Result<()> {
    let current: i64 =
        sqlx::query_scalar("SELECT workspace_revision FROM local_jobs WHERE job_id=?")
            .bind(job_id)
            .fetch_one(&mut **tx)
            .await?;
    let next = next_revision(current, current, true)?;
    sqlx::query("UPDATE local_jobs SET workspace_revision=? WHERE job_id=?")
        .bind(next)
        .bind(job_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
fn next_revision(saved: i64, current: i64, changed: bool) -> Result<i64> {
    let high = saved.max(current);
    Ok(if changed {
        high.checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("subtitle revision overflow"))?
    } else {
        high
    })
}
fn source_matches(a: &LocalSubtitleSegmentRecord, b: &LocalSubtitleSegmentRecord) -> bool {
    a.source_text == b.source_text
        && a.start_ms == b.start_ms
        && a.end_ms == b.end_ms
        && a.source_edited == b.source_edited
        && a.timing_edited == b.timing_edited
}
fn lineage_matches(a: &LocalSubtitleProvenanceRecord, b: &LocalSubtitleProvenanceRecord) -> bool {
    a.source_kind == b.source_kind
        && a.source_run_id == b.source_run_id
        && a.timed_unit_ids_json == b.timed_unit_ids_json
        && a.timing_source == b.timing_source
        && a.alignment_status == b.alignment_status
}
fn dependencies_by_target(edges: &[Dependency]) -> HashMap<&str, Vec<&Dependency>> {
    let mut targets: HashMap<&str, Vec<&Dependency>> = HashMap::new();
    for edge in edges {
        targets
            .entry(&edge.target_segment_id)
            .or_default()
            .push(edge);
    }
    targets
}

/// Apply cue-trigger semantics that DELETE/INSERT reconstruction cannot fire.
/// Only this opt-in path changes versions; legacy callers keep their contract.
async fn finish_recorded(tx: Transaction<'_, Sqlite>, before: State) -> Result<LocalStructureEdit> {
    finish_recorded_with_adoption(tx, before, &HashSet::new()).await
}
async fn finish_recorded_with_adoption(
    mut tx: Transaction<'_, Sqlite>,
    before: State,
    adopted_ids: &HashSet<String>,
) -> Result<LocalStructureEdit> {
    let changed = capture(&mut tx, &before.job_id).await?;
    let old_cues = before
        .segments
        .iter()
        .map(|cue| (cue.id.as_str(), cue))
        .collect::<HashMap<_, _>>();
    let current_cues = changed
        .segments
        .iter()
        .map(|cue| (cue.id.as_str(), cue))
        .collect::<HashMap<_, _>>();
    let old_provenance = before
        .provenance
        .iter()
        .map(|p| (p.segment_id.as_str(), p))
        .collect::<HashMap<_, _>>();
    let current_provenance = changed
        .provenance
        .iter()
        .map(|p| (p.segment_id.as_str(), p))
        .collect::<HashMap<_, _>>();
    let changed_contexts = before
        .segments
        .iter()
        .filter(|cue| {
            current_cues.get(cue.id.as_str()).is_none_or(|now| {
                !source_matches(cue, now)
                    || !lineage_matches(
                        old_provenance[cue.id.as_str()],
                        current_provenance[cue.id.as_str()],
                    )
            })
        })
        .map(|cue| cue.id.as_str())
        .collect::<HashSet<_>>();
    let consumers = before
        .dependencies
        .iter()
        .filter(|edge| changed_contexts.contains(edge.context_segment_id.as_str()))
        .filter_map(|edge| current_cues.get(edge.target_segment_id.as_str()).copied())
        .filter(|cue| cue.translated_text.is_some() && !cue.translation_edited)
        .map(|cue| cue.id.as_str())
        .collect::<HashSet<_>>();
    let old_edges = dependencies_by_target(&before.dependencies);
    let new_edges = dependencies_by_target(&changed.dependencies);
    for cue in &changed.segments {
        let id = cue.id.as_str();
        let Some(old) = old_cues.get(id).copied() else {
            continue;
        };
        let saved = old_provenance[id];
        let current = current_provenance[id];
        let source_changed = !source_matches(old, cue) || !lineage_matches(saved, current);
        let translation_replaced = old.translated_text != cue.translated_text
            || old.translation_edited != cue.translation_edited;
        let translation_changed = translation_replaced
            || old.translation_stale != cue.translation_stale
            || old_edges.get(id) != new_edges.get(id)
            || consumers.contains(id);
        if !source_changed && !translation_changed {
            continue;
        }
        let source_revision = next_revision(
            saved.source_revision,
            current.source_revision,
            source_changed,
        )?;
        let translation_revision = next_revision(
            saved.translation_revision,
            current.translation_revision,
            translation_changed,
        )?;
        let source_kind = if old.source_text != cue.source_text && !adopted_ids.contains(id) {
            "manual"
        } else {
            &current.source_kind
        };
        let timing_source = if (old.start_ms != cue.start_ms || old.end_ms != cue.end_ms)
            && !adopted_ids.contains(id)
        {
            "manual"
        } else {
            &current.timing_source
        };
        let alignment = if old.source_text != cue.source_text && !adopted_ids.contains(id) {
            "stale"
        } else {
            &current.alignment_status
        };
        let dependency_stale = !translation_replaced
            && (current.translation_dependency_stale || consumers.contains(id));
        sqlx::query("UPDATE local_subtitle_provenance SET source_kind=?,timing_source=?,alignment_status=?,source_revision=?,translation_revision=?,translation_dependency_stale=?,updated_at_unix=? WHERE job_id=? AND segment_id=?")
            .bind(source_kind).bind(timing_source).bind(alignment).bind(source_revision).bind(translation_revision).bind(dependency_stale)
            .bind(chrono::Utc::now().timestamp().max(current.updated_at_unix)).bind(&before.job_id).bind(id).execute(&mut *tx).await?;
        if translation_replaced {
            sqlx::query("DELETE FROM local_translation_dependency_edges WHERE target_segment_id=?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
    }
    // Explicit advancement also covers deletion to an empty workspace.
    advance(&mut tx, &before.job_id).await?;
    let after = capture(&mut tx, &before.job_id).await?;
    tx.commit().await?;
    Ok(LocalStructureEdit {
        id: uuid::Uuid::new_v4().to_string(),
        before,
        after,
    })
}

impl LocalDatabase {
    /// Candidate identity is checked before the shared adoption transaction.
    /// Provenance assignment and its permanent protection ledger commit with the receipt.
    #[allow(clippy::too_many_arguments)]
    pub async fn replace_segment_range_recorded(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
        expected_segments: &[LocalSubtitleSegmentRecord],
        candidates: &[TranscriptSegment],
        asr_provenance: Option<(&str, &CandidateCueSet)>,
        expected_revision: i64,
    ) -> Result<LocalStructureEdit> {
        ensure!(
            start_ms >= 0 && end_ms > start_ms,
            "invalid retranscription range"
        );
        let adopted_ids = if let Some((_, evidence)) = asr_provenance {
            ensure!(
                evidence.cues.len() == candidates.len()
                    && evidence.cues.iter().zip(candidates).all(|(e, cue)| {
                        e.cue.id == cue.id
                            && e.cue.start_ms == cue.start_ms
                            && e.cue.end_ms == cue.end_ms
                            && e.cue.source_text == cue.source_text
                    }),
                "candidate provenance does not match the replacement cues"
            );
            candidates.iter().map(|cue| cue.id.clone()).collect()
        } else {
            HashSet::new()
        };
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let before = capture(&mut tx, job_id).await?;
        replace_range_in_transaction(
            &mut tx,
            job_id,
            start_ms,
            end_ms,
            expected_segments,
            candidates,
            asr_provenance,
            Some(expected_revision),
        )
        .await?;
        finish_recorded_with_adoption(tx, before, &adopted_ids).await
    }
    /// Opt-in recorded path; legacy callers keep their original return contract.
    pub async fn edit_subtitle_block_recorded(
        &self,
        job_id: &str,
        expected: &[LocalSubtitleSegmentRecord],
        remove_id: Option<&str>,
        insertion: Option<LocalSubtitleSegmentRecord>,
    ) -> Result<LocalStructureEdit> {
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let before = capture(&mut tx, job_id).await?;
        edit_block_in_transaction(&mut tx, job_id, expected, remove_id, insertion).await?;
        finish_recorded(tx, before).await
    }

    pub async fn split_segment_recorded(
        &self,
        job_id: &str,
        expected_revision: i64,
        segment_id: &str,
        boundary_ms: i64,
        left_source_text: String,
        right_source_text: String,
        left_translated_text: Option<String>,
        right_translated_text: Option<String>,
    ) -> Result<LocalStructureEdit> {
        let left_source_text = normalized_subtitle_text(left_source_text, "left source subtitle")?;
        let right_source_text =
            normalized_subtitle_text(right_source_text, "right source subtitle")?;
        let left_translated_text = normalized_optional_subtitle_text(left_translated_text);
        let right_translated_text = normalized_optional_subtitle_text(right_translated_text);
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let before = capture(&mut tx, job_id).await?;
        ensure!(
            before.revision == expected_revision,
            "subtitle revision changed before Cut"
        );
        split_in_transaction(
            &mut tx,
            job_id,
            segment_id,
            boundary_ms,
            left_source_text,
            right_source_text,
            left_translated_text,
            right_translated_text,
        )
        .await?;
        finish_recorded(tx, before).await
    }

    pub async fn merge_adjacent_segments_recorded(
        &self,
        job_id: &str,
        expected_revision: i64,
        left_id: &str,
        right_id: &str,
        source_text: String,
        translated_text: Option<String>,
    ) -> Result<LocalStructureEdit> {
        let source_text = normalized_subtitle_text(source_text, "merged source subtitle")?;
        let translated_text = normalized_optional_subtitle_text(translated_text);
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let before = capture(&mut tx, job_id).await?;
        ensure!(
            before.revision == expected_revision,
            "subtitle revision changed before Join"
        );
        merge_in_transaction(
            &mut tx,
            job_id,
            left_id,
            right_id,
            source_text,
            translated_text,
        )
        .await?;
        finish_recorded(tx, before).await
    }

    pub async fn save_segment_timing_recorded(
        &self,
        job_id: &str,
        before_segments: &[LocalSubtitleSegmentRecord],
        after_segments: &[LocalSubtitleSegmentRecord],
    ) -> Result<LocalStructureEdit> {
        ensure!(
            before_segments.len() == after_segments.len(),
            "timing edits cannot add or remove subtitle segments"
        );
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let before = capture(&mut tx, job_id).await?;
        timing_in_transaction(&mut tx, job_id, before_segments, after_segments).await?;
        finish_recorded(tx, before).await
    }

    pub async fn undo_recorded_structure(
        &self,
        edit: &LocalStructureEdit,
    ) -> Result<LocalStructureUndoResult> {
        let job_id = edit.job_id();
        let mut tx = self.pool.begin().await?;
        ensure_structure_editable(&mut tx, job_id).await?;
        let current = capture(&mut tx, job_id).await?;
        ensure!(
            current == edit.after,
            "subtitle workspace or metadata changed; cannot undo this structure edit"
        );
        let mut restored = edit.before.segments.clone();
        persist_structure_segments(&mut tx, job_id, &mut restored).await?;
        let current_cues = current
            .segments
            .iter()
            .map(|s| (s.id.as_str(), s))
            .collect::<HashMap<_, _>>();
        let current_provenance = current
            .provenance
            .iter()
            .map(|p| (p.segment_id.as_str(), p))
            .collect::<HashMap<_, _>>();
        let before_cues = restored
            .iter()
            .map(|s| (s.id.as_str(), s))
            .collect::<HashMap<_, _>>();
        let mut revisions = HashMap::new();
        let current_dependencies = dependencies_by_target(&current.dependencies);
        let before_dependencies = dependencies_by_target(&edit.before.dependencies);
        for saved in &edit.before.provenance {
            let id = saved.segment_id.as_str();
            let cue = before_cues[id];
            let old = current_provenance.get(id).copied();
            let old_cue = current_cues.get(id).copied();
            let source_changed = old_cue.is_none_or(|s| !source_matches(s, cue))
                || old.is_none_or(|p| !lineage_matches(p, saved));
            let deps_changed = current_dependencies.get(id) != before_dependencies.get(id);
            let translation_changed = old_cue.is_none_or(|s| {
                s.translated_text != cue.translated_text
                    || s.translation_edited != cue.translation_edited
                    || s.translation_stale != cue.translation_stale
            }) || old.is_none_or(|p| {
                p.translation_dependency_stale != saved.translation_dependency_stale
            }) || deps_changed;
            let source_revision = next_revision(
                saved.source_revision,
                old.map_or(0, |p| p.source_revision),
                source_changed,
            )?;
            let translation_revision = next_revision(
                saved.translation_revision,
                old.map_or(0, |p| p.translation_revision),
                translation_changed,
            )?;
            revisions.insert(id, (saved.source_revision, source_revision));
            sqlx::query("UPDATE local_subtitle_provenance SET source_kind=?,source_run_id=?,timed_unit_ids_json=?,timing_source=?,alignment_status=?,source_revision=?,translation_revision=?,translation_dependency_stale=?,updated_at_unix=? WHERE job_id=? AND segment_id=?")
                .bind(&saved.source_kind).bind(&saved.source_run_id).bind(&saved.timed_unit_ids_json).bind(&saved.timing_source).bind(&saved.alignment_status)
                .bind(source_revision).bind(translation_revision).bind(saved.translation_dependency_stale)
                .bind(chrono::Utc::now().timestamp().max(saved.updated_at_unix).max(old.map_or(0, |p| p.updated_at_unix)))
                .bind(job_id).bind(id).execute(&mut *tx).await?;
        }
        sqlx::query("DELETE FROM local_translation_dependency_edges WHERE target_segment_id IN (SELECT id FROM local_subtitle_segments WHERE job_id=?)")
            .bind(job_id).execute(&mut *tx).await?;
        for edge in &edit.before.dependencies {
            let &(before_revision, new_revision) = revisions
                .get(edge.context_segment_id.as_str())
                .ok_or_else(|| {
                    anyhow::anyhow!("undo dependency context is outside its captured job")
                })?;
            let revision = if edge.context_source_revision == before_revision {
                new_revision
            } else {
                edge.context_source_revision
            };
            sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,?)")
                .bind(&edge.target_segment_id)
                .bind(&edge.context_segment_id)
                .bind(revision)
                .execute(&mut *tx)
                .await?;
        }
        advance(&mut tx, job_id).await?;
        let state = capture(&mut tx, job_id).await?;
        tx.commit().await?;
        Ok(LocalStructureUndoResult {
            undone_id: edit.id.clone(),
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            job_manifest::JobManifest, job_snapshot::JobSnapshot, job_status::JobStatus,
        },
        domain::TranscriptSegment,
        infrastructure::job_store::Job,
    };
    use std::{fs, path::PathBuf};

    async fn fixture() -> (
        PathBuf,
        LocalDatabase,
        String,
        Vec<LocalSubtitleSegmentRecord>,
    ) {
        let root =
            std::env::temp_dir().join(format!("atogaki-recorded-undo-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let job_id = job.id();
        let mut manifest =
            JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("fixture.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: vec![
                    TranscriptSegment::new(0, 1000, "context".into()),
                    TranscriptSegment::new(2000, 3000, "target".into()),
                ],
            })
            .await
            .unwrap();
        let before = database.list_segments(&job_id).await.unwrap();
        (root, database, job_id, before)
    }
    async fn state(database: &LocalDatabase, job: &str) -> State {
        let mut tx = database.pool.begin().await.unwrap();
        let result = capture(&mut tx, job).await.unwrap();
        tx.rollback().await.unwrap();
        result
    }
    async fn close(root: PathBuf, database: LocalDatabase) {
        database.close().await;
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rejects_metadata_only_changes_and_leaves_changed_workspace_intact() {
        let (root, db, job, cues) = fixture().await;
        let receipt = db
            .edit_subtitle_block_recorded(&job, &cues, Some(&cues[0].id), None)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE local_subtitle_provenance SET alignment_status='stale' WHERE segment_id=?",
        )
        .bind(&cues[1].id)
        .execute(&db.pool)
        .await
        .unwrap();
        let changed = state(&db, &job).await;
        assert_eq!(
            changed.revision, receipt.after.revision,
            "metadata change alone does not use cue triggers"
        );
        assert!(db.undo_recorded_structure(&receipt).await.is_err());
        assert_eq!(state(&db, &job).await, changed);
        close(root, db).await;
    }

    #[tokio::test]
    async fn partial_restore_failure_rolls_back_revision_provenance_and_dependencies() {
        let (root, db, job, cues) = fixture().await;
        sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,0)")
            .bind(&cues[1].id)
            .bind(&cues[0].id)
            .execute(&db.pool)
            .await
            .unwrap();
        let receipt = db
            .edit_subtitle_block_recorded(&job, &cues, Some(&cues[0].id), None)
            .await
            .unwrap();
        let expected = state(&db, &job).await;
        sqlx::query("CREATE TRIGGER fail_recorded_restore BEFORE INSERT ON local_subtitle_segments WHEN NEW.source_text='target' AND EXISTS(SELECT 1 FROM local_subtitle_segments WHERE source_text='context') BEGIN SELECT RAISE(ABORT,'injected after first restore row'); END").execute(&db.pool).await.unwrap();
        let error = db.undo_recorded_structure(&receipt).await.unwrap_err();
        assert!(format!("{error:#}").contains("injected after first restore row"));
        assert_eq!(state(&db, &job).await, expected);
        sqlx::query("DROP TRIGGER fail_recorded_restore")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER fail_restored_edge BEFORE INSERT ON local_translation_dependency_edges BEGIN SELECT RAISE(ABORT,'injected after provenance restore'); END")
            .execute(&db.pool).await.unwrap();
        let error = db.undo_recorded_structure(&receipt).await.unwrap_err();
        assert!(format!("{error:#}").contains("injected after provenance restore"));
        assert_eq!(state(&db, &job).await, expected);
        sqlx::query("DROP TRIGGER fail_restored_edge")
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.undo_recorded_structure(&receipt)
                .await
                .unwrap()
                .segments(),
            cues.as_slice()
        );
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&db.pool)
                .await
                .unwrap()
                .is_empty()
        );
        close(root, db).await;
    }

    #[tokio::test]
    async fn continuous_undos_rebase_only_from_their_own_committed_result_including_empty_job() {
        let (root, db, job, cues) = fixture().await;
        let mut older = db
            .edit_subtitle_block_recorded(&job, &cues, Some(&cues[0].id), None)
            .await
            .unwrap();
        let newer = db
            .edit_subtitle_block_recorded(&job, older.segments(), Some(&cues[1].id), None)
            .await
            .unwrap();
        assert!(newer.segments().is_empty());
        assert!(
            newer.after.revision > older.after.revision,
            "delete-to-empty must advance revision"
        );
        let result = db.undo_recorded_structure(&newer).await.unwrap();
        assert!(
            db.undo_recorded_structure(&older).await.is_err(),
            "old counters cannot bypass rebase"
        );
        assert!(
            older.rebase_after_undo(&older.clone(), &result).is_err(),
            "result identity must match the undone receipt"
        );
        older.rebase_after_undo(&newer, &result).unwrap();
        let restored = db.undo_recorded_structure(&older).await.unwrap();
        assert_eq!(restored.segments(), cues.as_slice());
        assert!(
            restored
                .state
                .provenance
                .iter()
                .all(|p| p.source_revision >= 1)
        );
        assert!(
            db.undo_recorded_structure(&older).await.is_err(),
            "successful receipt cannot be replayed"
        );
        close(root, db).await;
    }

    #[tokio::test]
    async fn stale_dependency_revision_is_not_refreshed_when_context_is_recreated() {
        let (root, db, job, cues) = fixture().await;
        sqlx::query("UPDATE local_subtitle_provenance SET source_revision=3 WHERE segment_id=?")
            .bind(&cues[0].id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE local_subtitle_provenance SET translation_dependency_stale=1 WHERE segment_id=?").bind(&cues[1].id).execute(&db.pool).await.unwrap();
        sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,2)")
            .bind(&cues[1].id)
            .bind(&cues[0].id)
            .execute(&db.pool)
            .await
            .unwrap();
        let receipt = db
            .edit_subtitle_block_recorded(&job, &cues, Some(&cues[0].id), None)
            .await
            .unwrap();
        let result = db.undo_recorded_structure(&receipt).await.unwrap();
        assert_eq!(result.state.dependencies[0].context_source_revision, 2);
        assert_eq!(
            result
                .state
                .provenance
                .iter()
                .find(|p| p.segment_id == cues[0].id)
                .unwrap()
                .source_revision,
            4
        );
        assert!(
            result
                .state
                .provenance
                .iter()
                .find(|p| p.segment_id == cues[1].id)
                .unwrap()
                .translation_dependency_stale
        );
        close(root, db).await;
    }
}

#[cfg(test)]
mod operation_tests;

#[cfg(test)]
mod adoption_tests;
