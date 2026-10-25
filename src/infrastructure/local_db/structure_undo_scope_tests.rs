//! Recorded undo contract: adopted lineage, context dependencies and monotonic
//! revisions restore atomically; learning snapshots keep their existing scope.
use super::{LocalDatabase, LocalMachineTranslation, NewLocalLearningSelection};
use crate::{
    application::{
        AsrInputScope, AsrRun, CandidateCue, CandidateCueSet, job_manifest::JobManifest,
        job_snapshot::JobSnapshot, job_status::JobStatus,
    },
    domain::TranscriptSegment,
    infrastructure::job_store::Job,
};
use std::fs;

#[tokio::test]
async fn recorded_structure_undo_restores_lineage_and_dependency_propagation() {
    let root = std::env::temp_dir().join(format!("atogaki-undo-scope-{}", uuid::Uuid::new_v4()));
    let job = Job::create_in(&root).unwrap();
    let job_id = job.id();
    let mut manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
    manifest.mark(JobStatus::Done);
    let database = LocalDatabase::open(root.join("audit.sqlite"))
        .await
        .unwrap();
    let segments = ["context", "target", "unrelated"]
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let mut cue =
                TranscriptSegment::new(i as u64 * 2000, i as u64 * 2000 + 1000, text.into());
            cue.set_translation(Some(format!("translation {i}")));
            cue
        })
        .collect::<Vec<_>>();
    database
        .sync_snapshot(&JobSnapshot {
            manifest,
            segments: segments.clone(),
        })
        .await
        .unwrap();
    let mut run = AsrRun::new(
        job_id.clone(),
        None,
        "fixture",
        "fixture",
        "synthetic-model",
        job.audio_wav.clone(),
        AsrInputScope::full(),
        serde_json::json!({}),
    );
    run.succeed();
    let artifact_dir = job.asr_runs_dir.join(&run.id);
    fs::create_dir_all(&artifact_dir).unwrap();
    let evidence = artifact_dir.join("synthetic-evidence.json");
    fs::write(&evidence, b"{\"synthetic\":true}").unwrap();
    database.record_asr_run(&run, &artifact_dir).await.unwrap();
    let candidate = CandidateCueSet {
        schema_version: 1,
        segmentation_policy: "audit-fixture".into(),
        cues: vec![CandidateCue {
            cue: segments[0].clone(),
            timed_unit_ids: vec!["evidence-unit".into()],
        }],
    };
    let original = database.subtitle_workspace_snapshot(&job_id).await.unwrap();
    database
        .replace_segment_range_with_revision(
            &job_id,
            0,
            1000,
            &original.1[..1],
            &segments[..1],
            Some((&run.id, &candidate)),
            Some(original.0),
        )
        .await
        .unwrap();
    // A valid nonzero revision fixture reveals revision reset on recreation.
    sqlx::query("UPDATE local_subtitle_provenance SET source_revision=7,translation_revision=3 WHERE segment_id=?")
        .bind(&segments[0].id).execute(&database.pool).await.unwrap();
    for (target, context, revision) in [(1, 0, 7), (0, 1, 0), (2, 1, 0)] {
        sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,?)")
            .bind(&segments[target].id)
            .bind(&segments[context].id)
            .bind(revision)
            .execute(&database.pool)
            .await
            .unwrap();
    }
    let learning = database
        .save_learning_selection(NewLocalLearningSelection {
            job_id: job_id.clone(),
            segment_id: segments[0].id.clone(),
            item_type: "sentence".into(),
            selection_start_utf16: 0,
            selection_end_utf16: 7,
        })
        .await
        .unwrap();
    let before = database.list_segments(&job_id).await.unwrap();
    let before_provenance = database.list_subtitle_provenance(&job_id).await.unwrap();
    assert!(database.asr_run_is_protected(&run.id).await.unwrap());

    let receipt = database
        .edit_subtitle_block_recorded(&job_id, &before, Some(&segments[0].id), None)
        .await
        .unwrap();
    assert!(receipt.segments()[0].translation_stale);
    let result = database.undo_recorded_structure(&receipt).await.unwrap();
    let restored = result.segments();
    assert_eq!(
        restored,
        before.as_slice(),
        "visible cue fields do restore exactly"
    );
    let provenance = database.list_subtitle_provenance(&job_id).await.unwrap();
    let recreated = provenance
        .iter()
        .find(|p| p.segment_id == segments[0].id)
        .unwrap();
    let old = before_provenance
        .iter()
        .find(|p| p.segment_id == segments[0].id)
        .unwrap();
    assert_eq!(old.source_run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!((old.source_revision, old.translation_revision), (7, 3));
    assert_eq!(recreated.source_run_id, old.source_run_id);
    assert_eq!(recreated.timed_unit_ids_json, old.timed_unit_ids_json);
    assert_eq!(
        (
            &*recreated.source_kind,
            &*recreated.alignment_status,
            &*recreated.timing_source
        ),
        ("asr", "current", "model")
    );
    assert_eq!(
        (recreated.source_revision, recreated.translation_revision),
        (8, 4)
    );
    let edges: Vec<(String, String, i64)> = sqlx::query_as("SELECT target_segment_id,context_segment_id,context_source_revision FROM local_translation_dependency_edges")
        .fetch_all(&database.pool).await.unwrap();
    assert_eq!(edges.len(), 3);
    assert!(edges.contains(&(segments[1].id.clone(), segments[0].id.clone(), 8)));
    assert!(edges.contains(&(segments[0].id.clone(), segments[1].id.clone(), 0)));
    assert!(edges.contains(&(segments[2].id.clone(), segments[1].id.clone(), 0)));
    let late = database
        .apply_machine_translations(
            &job_id,
            &[LocalMachineTranslation {
                segment_id: segments[0].id.clone(),
                source_text: before[0].source_text.clone(),
                expected_translated_text: before[0].translated_text.clone(),
                expected_translation_edited: false,
                expected_source_revision: Some(7),
                expected_translation_revision: Some(3),
                translated_text: "late result".into(),
            }],
        )
        .await;
    assert!(
        late.is_err(),
        "old revision must not become valid again after undo"
    );
    let late_consumer = database
        .apply_machine_translations(
            &job_id,
            &[LocalMachineTranslation {
                segment_id: segments[1].id.clone(),
                source_text: before[1].source_text.clone(),
                expected_translated_text: before[1].translated_text.clone(),
                expected_translation_edited: false,
                expected_source_revision: Some(0),
                expected_translation_revision: Some(0),
                translated_text: "old consumer result".into(),
            }],
        )
        .await;
    assert!(
        late_consumer.is_err(),
        "restoring dependencies invalidates the consumer's old translation request too"
    );
    let kept = database
        .get_learning_item(&learning.item.id)
        .await
        .unwrap()
        .unwrap();
    assert!(kept.occurrences[0].segment_id.is_none());
    assert_eq!(kept.occurrences[0].segment_source_snapshot, "context");
    assert_eq!(
        (kept.occurrences[0].start_ms, kept.occurrences[0].end_ms),
        (0, 1000)
    );
    assert!(
        database.asr_run_is_protected(&run.id).await.unwrap(),
        "adoption ledger still protects evidence"
    );
    assert_eq!(fs::read(&evidence).unwrap(), b"{\"synthetic\":true}");
    assert_eq!(database.list_asr_runs(&job_id).await.unwrap().len(), 1);

    database
        .update_segment(
            &job_id,
            &segments[0].id,
            "changed context".into(),
            before[0].translated_text.clone(),
            0,
            1000,
        )
        .await
        .unwrap();
    let after_edit = database.list_subtitle_provenance(&job_id).await.unwrap();
    assert!(
        after_edit
            .iter()
            .find(|p| p.segment_id == segments[1].id)
            .unwrap()
            .translation_dependency_stale,
        "restored dependency must propagate later context changes"
    );
    assert!(!database.list_segments(&job_id).await.unwrap()[1].translation_stale);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&database.pool)
            .await
            .unwrap()
            .is_empty()
    );
    database.close().await;
    fs::remove_dir_all(root).unwrap();
}
