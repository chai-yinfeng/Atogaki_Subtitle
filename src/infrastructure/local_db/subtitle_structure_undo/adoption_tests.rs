use super::operation_tests::{close, fixture, state};
use super::*;
use crate::{
    application::{AsrInputScope, AsrRun, CandidateCue},
    infrastructure::local_db::NewLocalLearningSelection,
};

async fn candidate_run(
    db: &LocalDatabase,
    root: &std::path::Path,
    job_id: &str,
    segment: TranscriptSegment,
) -> (AsrRun, CandidateCueSet) {
    // Only the indexed synthetic artifact path is needed by this repository fixture.
    let directory = root.join("synthetic-runs");
    let mut run = AsrRun::new(
        job_id.to_owned(),
        None,
        "fixture",
        "fixture",
        "new-model",
        root.join("synthetic.wav"),
        AsrInputScope::full(),
        serde_json::json!({}),
    );
    run.succeed();
    db.record_asr_run(&run, &directory.join(&run.id))
        .await
        .unwrap();
    (
        run,
        CandidateCueSet {
            schema_version: 1,
            segmentation_policy: "fixture".into(),
            cues: vec![CandidateCue {
                cue: segment,
                timed_unit_ids: vec!["new-unit".into()],
            }],
        },
    )
}

#[tokio::test]
async fn recorded_adoption_and_undo_restore_original_sources_but_keep_adopted_evidence_protected() {
    for retain_id in [false, true] {
        let (root, db, before) = fixture().await;
        let learning = db
            .save_learning_selection(NewLocalLearningSelection {
                job_id: before.job_id.clone(),
                segment_id: before.segments[1].id.clone(),
                item_type: "sentence".into(),
                selection_start_utf16: 0,
                selection_end_utf16: 5,
            })
            .await
            .unwrap();
        let mut cue = TranscriptSegment::new(0, 2000, "candidate text".into());
        if retain_id {
            cue.id = before.segments[0].id.clone();
        }
        let (run, evidence) = candidate_run(&db, &root, &before.job_id, cue).await;
        let cues = evidence.transcript_segments();
        let receipt = db
            .replace_segment_range_recorded(
                &before.job_id,
                0,
                2000,
                &before.segments[..2],
                &cues,
                Some((&run.id, &evidence)),
                before.revision,
            )
            .await
            .unwrap();
        assert_eq!(receipt.segments().len(), 2);
        assert!(
            receipt
                .translation_context_stale_ids()
                .contains(&before.segments[2].id)
        );
        let provenance = receipt
            .after
            .provenance
            .iter()
            .find(|p| p.segment_id == cues[0].id)
            .unwrap();
        assert_eq!(provenance.source_run_id.as_deref(), Some(run.id.as_str()));
        assert_eq!(
            (
                &*provenance.source_kind,
                &*provenance.timing_source,
                &*provenance.alignment_status
            ),
            ("asr", "model", "current")
        );
        assert_eq!(provenance.timed_unit_ids_json, "[\"new-unit\"]");
        if retain_id {
            assert!(
                provenance.source_revision
                    > before
                        .provenance
                        .iter()
                        .find(|p| p.segment_id == cues[0].id)
                        .unwrap()
                        .source_revision
            );
        }
        assert!(db.asr_run_is_protected(&run.id).await.unwrap());
        let restored = db.undo_recorded_structure(&receipt).await.unwrap();
        assert_eq!(restored.segments(), before.segments.as_slice());
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
                    &now.alignment_status
                ),
                (
                    &old.source_kind,
                    &old.source_run_id,
                    &old.timed_unit_ids_json,
                    &old.timing_source,
                    &old.alignment_status
                )
            );
            assert!(now.source_revision >= old.source_revision);
        }
        assert_eq!(restored.state.dependencies.len(), before.dependencies.len());
        for edge in &restored.state.dependencies {
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
        assert!(
            db.asr_run_is_protected(&run.id).await.unwrap(),
            "adoption ledger is not undone"
        );
        assert_eq!(db.list_asr_runs(&before.job_id).await.unwrap().len(), 2);
        let kept = db
            .get_learning_item(&learning.item.id)
            .await
            .unwrap()
            .unwrap();
        assert!(kept.occurrences[0].segment_id.is_none());
        assert_eq!(kept.occurrences[0].segment_source_snapshot, "right");
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&db.pool)
                .await
                .unwrap()
                .is_empty()
        );
        close(root, db).await;
    }
}

#[tokio::test]
async fn adoption_with_identical_cue_but_new_lineage_advances_source_and_invalidates_context() {
    let (root, db, before) = fixture().await;
    let old = &before.segments[0];
    let mut cue = TranscriptSegment::new(0, 1000, old.source_text.clone());
    cue.id = old.id.clone();
    let (run, evidence) = candidate_run(&db, &root, &before.job_id, cue).await;
    let receipt = db
        .replace_segment_range_recorded(
            &before.job_id,
            0,
            1000,
            &before.segments[..1],
            &evidence.transcript_segments(),
            Some((&run.id, &evidence)),
            before.revision,
        )
        .await
        .unwrap();
    let new = receipt
        .after
        .provenance
        .iter()
        .find(|p| p.segment_id == old.id)
        .unwrap();
    let old_p = before
        .provenance
        .iter()
        .find(|p| p.segment_id == old.id)
        .unwrap();
    assert!(new.source_revision > old_p.source_revision);
    assert_eq!(new.source_kind, "asr");
    assert!(
        receipt
            .translation_context_stale_ids()
            .contains(&before.segments[2].id)
    );
    assert_eq!(
        db.undo_recorded_structure(&receipt)
            .await
            .unwrap()
            .segments(),
        before.segments.as_slice()
    );
    close(root, db).await;
}

#[tokio::test]
async fn adoption_failures_roll_back_replacement_provenance_and_protection_ledger_together() {
    let (root, db, before) = fixture().await;
    let (run, evidence) = candidate_run(
        &db,
        &root,
        &before.job_id,
        TranscriptSegment::new(0, 2000, "new candidate".into()),
    )
    .await;
    let cues = evidence.transcript_segments();
    for (trigger, diagnostic) in [
        (
            "CREATE TRIGGER fail_adoption BEFORE INSERT ON local_asr_candidate_adoptions BEGIN SELECT RAISE(ABORT,'ledger failure'); END",
            "ledger failure",
        ),
        (
            "CREATE TRIGGER fail_adoption BEFORE UPDATE ON local_subtitle_provenance WHEN NEW.translation_dependency_stale=1 BEGIN SELECT RAISE(ABORT,'post-ledger metadata failure'); END",
            "post-ledger metadata failure",
        ),
    ] {
        sqlx::query(trigger).execute(&db.pool).await.unwrap();
        let error = db
            .replace_segment_range_recorded(
                &before.job_id,
                0,
                2000,
                &before.segments[..2],
                &cues,
                Some((&run.id, &evidence)),
                before.revision,
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(diagnostic));
        assert_eq!(state(&db, &before.job_id).await, before);
        assert!(!db.asr_run_is_protected(&run.id).await.unwrap());
        sqlx::query("DROP TRIGGER fail_adoption")
            .execute(&db.pool)
            .await
            .unwrap();
    }
    let receipt = db
        .replace_segment_range_recorded(
            &before.job_id,
            0,
            2000,
            &before.segments[..2],
            &cues,
            Some((&run.id, &evidence)),
            before.revision,
        )
        .await
        .unwrap();
    assert!(db.asr_run_is_protected(&run.id).await.unwrap());
    db.undo_recorded_structure(&receipt).await.unwrap();
    assert!(db.asr_run_is_protected(&run.id).await.unwrap());
    close(root, db).await;
}

#[tokio::test]
async fn stale_revision_boundary_and_mismatched_provenance_reject_without_adoption() {
    let (root, db, before) = fixture().await;
    let (run, evidence) = candidate_run(
        &db,
        &root,
        &before.job_id,
        TranscriptSegment::new(0, 2000, "new candidate".into()),
    )
    .await;
    let cues = evidence.transcript_segments();
    assert!(
        db.replace_segment_range_recorded(
            &before.job_id,
            0,
            2000,
            &before.segments[..2],
            &cues,
            Some((&run.id, &evidence)),
            before.revision - 1
        )
        .await
        .is_err()
    );
    assert!(
        db.replace_segment_range_recorded(
            &before.job_id,
            0,
            1500,
            &before.segments[..2],
            &cues,
            Some((&run.id, &evidence)),
            before.revision
        )
        .await
        .is_err()
    );
    let mut mismatched = evidence.clone();
    mismatched.cues[0].cue.source_text = "different evidence".into();
    assert!(
        db.replace_segment_range_recorded(
            &before.job_id,
            0,
            2000,
            &before.segments[..2],
            &cues,
            Some((&run.id, &mismatched)),
            before.revision
        )
        .await
        .is_err()
    );
    assert_eq!(state(&db, &before.job_id).await, before);
    assert!(!db.asr_run_is_protected(&run.id).await.unwrap());
    close(root, db).await;
}
