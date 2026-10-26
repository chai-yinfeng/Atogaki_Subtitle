use super::*;
use crate::{
    application::{
        AsrInputScope, AsrRun, CandidateCue, CandidateCueSet, job_manifest::JobManifest,
        job_snapshot::JobSnapshot, job_status::JobStatus,
    },
    domain::TranscriptSegment,
    infrastructure::{job_store::Job, local_db::LocalMachineTranslation},
};
use std::{fs, path::PathBuf};

pub(super) async fn fixture() -> (PathBuf, LocalDatabase, State) {
    let root = std::env::temp_dir().join(format!(
        "atogaki-recorded-operations-{}",
        uuid::Uuid::new_v4()
    ));
    let job = Job::create_in(&root).unwrap();
    let mut manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
    manifest.mark(JobStatus::Done);
    let segments = ["left", "right", "consumer"]
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let start = if i < 2 { i as u64 * 1000 } else { 3000 };
            let mut cue = TranscriptSegment::new(start, start + 1000, text.into());
            cue.set_translation(Some(format!("translation {i}")));
            cue
        })
        .collect::<Vec<_>>();
    let db = LocalDatabase::open(root.join("fixture.sqlite"))
        .await
        .unwrap();
    db.sync_snapshot(&JobSnapshot {
        manifest,
        segments: segments.clone(),
    })
    .await
    .unwrap();
    let mut run = AsrRun::new(
        job.id(),
        None,
        "fixture",
        "fixture",
        "synthetic",
        job.audio_wav.clone(),
        AsrInputScope::full(),
        serde_json::json!({}),
    );
    run.succeed();
    db.record_asr_run(&run, &job.asr_runs_dir.join(&run.id))
        .await
        .unwrap();
    let candidates = CandidateCueSet {
        schema_version: 1,
        segmentation_policy: "fixture".into(),
        cues: segments[..2]
            .iter()
            .enumerate()
            .map(|(i, cue)| CandidateCue {
                cue: cue.clone(),
                timed_unit_ids: vec![format!("unit-{i}")],
            })
            .collect(),
    };
    db.assign_asr_cue_provenance(&job.id(), &run.id, &candidates)
        .await
        .unwrap();
    for (i, source, translation) in [(0, 4, 2), (1, 6, 3)] {
        sqlx::query("UPDATE local_subtitle_provenance SET source_revision=?,translation_revision=? WHERE segment_id=?").bind(source).bind(translation).bind(&segments[i].id).execute(&db.pool).await.unwrap();
    }
    for (target, context, revision) in [(2, 0, 4), (2, 1, 6), (0, 1, 6)] {
        sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,?)")
            .bind(&segments[target].id)
            .bind(&segments[context].id)
            .bind(revision)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    let before = state(&db, &job.id()).await;
    (root, db, before)
}
pub(super) async fn state(db: &LocalDatabase, job: &str) -> State {
    let mut tx = db.pool.begin().await.unwrap();
    let state = capture(&mut tx, job).await.unwrap();
    tx.rollback().await.unwrap();
    state
}
pub(super) async fn close(root: PathBuf, db: LocalDatabase) {
    db.close().await;
    fs::remove_dir_all(root).unwrap();
}
async fn cut(db: &LocalDatabase, before: &State) -> LocalStructureEdit {
    let cue = &before.segments[0];
    db.split_segment_recorded(
        &before.job_id,
        before.revision,
        &cue.id,
        500,
        cue.source_text.clone(),
        cue.source_text.clone(),
        cue.translated_text.clone(),
        cue.translated_text.clone(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn cut_join_time_and_three_undos_restore_asr_lineage_and_remap_all_dependencies() {
    let (root, db, before) = fixture().await;
    let mut split = cut(&db, &before).await;
    assert!(
        split
            .translation_context_stale_ids()
            .contains(&before.segments[2].id)
    );
    let original = &before.segments[0];
    for (index, source, translation) in [(0, 4, 2), (2, 0, 0)] {
        let old = &before.segments[index];
        assert!(
            db.apply_machine_translations(
                &before.job_id,
                &[LocalMachineTranslation {
                    segment_id: old.id.clone(),
                    source_text: old.source_text.clone(),
                    expected_translated_text: old.translated_text.clone(),
                    expected_translation_edited: false,
                    expected_source_revision: Some(source),
                    expected_translation_revision: Some(translation),
                    translated_text: "late".into()
                }]
            )
            .await
            .is_err()
        );
    }
    let right_part = &split.segments()[1];
    let adopted_right = &split.segments()[2];
    assert!(
        db.merge_adjacent_segments_recorded(
            &before.job_id,
            before.revision,
            &right_part.id,
            &adopted_right.id,
            "left right".into(),
            Some("merged".into())
        )
        .await
        .is_err()
    );
    let mut join = db
        .merge_adjacent_segments_recorded(
            &before.job_id,
            split.workspace_revision(),
            &right_part.id,
            &adopted_right.id,
            "left right".into(),
            Some("merged".into()),
        )
        .await
        .unwrap();
    let mut invalid = join.segments().to_vec();
    invalid[0].end_ms = 600;
    assert!(
        db.save_segment_timing_recorded(&before.job_id, join.segments(), &invalid)
            .await
            .is_err()
    );
    assert_eq!(state(&db, &before.job_id).await, join.after);
    let mut retimed = join.segments().to_vec();
    retimed[1].end_ms = 1900;
    let timing = db
        .save_segment_timing_recorded(&before.job_id, join.segments(), &retimed)
        .await
        .unwrap();
    let restore_time = db.undo_recorded_structure(&timing).await.unwrap();
    join.rebase_after_undo(&timing, &restore_time).unwrap();
    let restore_join = db.undo_recorded_structure(&join).await.unwrap();
    let restored_right = restore_join
        .state
        .provenance
        .iter()
        .find(|p| p.segment_id == adopted_right.id)
        .unwrap();
    let old_right = before
        .provenance
        .iter()
        .find(|p| p.segment_id == adopted_right.id)
        .unwrap();
    assert_eq!(restored_right.source_run_id, old_right.source_run_id);
    assert_eq!(
        restored_right.timed_unit_ids_json,
        old_right.timed_unit_ids_json
    );
    assert!(restored_right.source_revision > old_right.source_revision);
    split.rebase_after_undo(&join, &restore_join).unwrap();
    let restored = db.undo_recorded_structure(&split).await.unwrap();
    assert_eq!(restored.segments(), before.segments.as_slice());
    assert!(restored.workspace_revision() > before.revision);
    assert!(restored.translation_context_stale_ids().is_empty());
    for old in &before.provenance {
        let now = restored
            .state
            .provenance
            .iter()
            .find(|p| p.segment_id == old.segment_id)
            .unwrap();
        assert_eq!(
            (
                &now.source_kind,
                &now.source_run_id,
                &now.timed_unit_ids_json,
                &now.timing_source,
                &now.alignment_status,
                now.translation_dependency_stale
            ),
            (
                &old.source_kind,
                &old.source_run_id,
                &old.timed_unit_ids_json,
                &old.timing_source,
                &old.alignment_status,
                old.translation_dependency_stale
            )
        );
        assert!(now.source_revision >= old.source_revision);
        assert!(now.translation_revision >= old.translation_revision);
    }
    assert_eq!(restored.state.dependencies.len(), before.dependencies.len());
    for edge in &restored.state.dependencies {
        assert!(
            before
                .dependencies
                .iter()
                .any(|old| old.target_segment_id == edge.target_segment_id
                    && old.context_segment_id == edge.context_segment_id)
        );
        assert_eq!(
            edge.context_source_revision,
            restored
                .state
                .provenance
                .iter()
                .find(|p| p.segment_id == edge.context_segment_id)
                .unwrap()
                .source_revision
        );
    }
    db.update_segment(
        &before.job_id,
        &original.id,
        "new context".into(),
        original.translated_text.clone(),
        original.start_ms,
        original.end_ms,
    )
    .await
    .unwrap();
    assert!(
        db.list_subtitle_provenance(&before.job_id)
            .await
            .unwrap()
            .iter()
            .find(|p| p.segment_id == before.segments[2].id)
            .unwrap()
            .translation_dependency_stale
    );
    close(root, db).await;
}

#[tokio::test]
async fn recorded_forward_metadata_failure_rolls_back_cut_join_and_timing() {
    let (root, db, before) = fixture().await;
    sqlx::query("CREATE TRIGGER fail_recorded_metadata BEFORE UPDATE ON local_subtitle_provenance WHEN (NEW.timing_source='manual' AND OLD.timing_source='model') OR (NEW.source_kind='manual' AND OLD.source_kind='asr') BEGIN SELECT RAISE(ABORT,'injected recorded metadata failure'); END").execute(&db.pool).await.unwrap();
    let cue = &before.segments[0];
    let mut retimed = before.segments.clone();
    retimed[0].end_ms = 900;
    let operations = [
        db.split_segment_recorded(
            &before.job_id,
            before.revision,
            &cue.id,
            500,
            cue.source_text.clone(),
            cue.source_text.clone(),
            cue.translated_text.clone(),
            cue.translated_text.clone(),
        )
        .await,
        db.merge_adjacent_segments_recorded(
            &before.job_id,
            before.revision,
            &cue.id,
            &before.segments[1].id,
            "left right".into(),
            Some("merged".into()),
        )
        .await,
        db.save_segment_timing_recorded(&before.job_id, &before.segments, &retimed)
            .await,
    ];
    for result in operations {
        assert!(
            format!("{:#}", result.unwrap_err()).contains("injected recorded metadata failure")
        );
    }
    assert_eq!(state(&db, &before.job_id).await, before);
    sqlx::query("DROP TRIGGER fail_recorded_metadata")
        .execute(&db.pool)
        .await
        .unwrap();
    let timed = db
        .save_segment_timing_recorded(&before.job_id, &before.segments, &retimed)
        .await
        .unwrap();
    assert_eq!(
        db.undo_recorded_structure(&timed).await.unwrap().segments(),
        before.segments.as_slice()
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
