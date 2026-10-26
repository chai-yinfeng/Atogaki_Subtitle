use super::*;
use crate::application::{
    LocalWorkspaceService, UnconfiguredTranslationProvider, job_manifest::JobManifest,
    job_snapshot::JobSnapshot, job_status::JobStatus,
};
use std::{fs, path::PathBuf};

async fn fixture() -> (PathBuf, LocalDatabase, String) {
    let root = std::env::temp_dir().join(format!("atogaki-edit-session-{}", uuid::Uuid::new_v4()));
    let job = Job::create_in(&root).unwrap();
    let mut manifest = JobManifest::new(&job, None, None, Default::default());
    manifest.mark(JobStatus::Done);
    let db = LocalDatabase::open(root.join("fixture.sqlite"))
        .await
        .unwrap();
    db.sync_snapshot(&JobSnapshot {
        manifest,
        segments: vec![
            TranscriptSegment::new(0, 1000, "context".into()),
            TranscriptSegment::new(2000, 3000, "target".into()),
        ],
    })
    .await
    .unwrap();
    (root, db, job.id())
}
async fn close(root: PathBuf, db: LocalDatabase) {
    db.close().await;
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn workspace_clones_and_provider_changes_share_history_and_continuous_undo() {
    let (root, db, job) = fixture().await;
    let workspace = LocalWorkspaceService::new(db.clone());
    let clone = workspace
        .clone()
        .using_provider(Arc::new(UnconfiguredTranslationProvider));
    let a = workspace.structure_edits();
    let b = clone.structure_edits();
    let before = db.list_segments(&job).await.unwrap();
    let mut after = before.clone();
    after[0].end_ms = 900;
    let timing = a.save_timing(&job, &before, &after).await.unwrap();
    assert_eq!(b.latest(&job).await, timing.undo.latest);
    assert_eq!(timing.segments, db.list_segments(&job).await.unwrap());
    let removed = b
        .edit_block(&job, &timing.segments, Some(&before[0].id), None)
        .await
        .unwrap();
    assert_eq!(a.stats().await.entries, 2);
    let undo = a
        .undo(&job, &removed.undo.latest.unwrap().token)
        .await
        .unwrap();
    assert_eq!(undo.segments, timing.segments);
    assert_eq!(
        undo.latest.as_ref().unwrap().expected_revision,
        undo.workspace_revision
    );
    let undo = b.undo(&job, &undo.latest.unwrap().token).await.unwrap();
    assert_eq!(undo.segments, before);
    assert_eq!(a.stats().await.retained_bytes, 0);
    close(root, db).await;
}
#[tokio::test]
async fn concurrent_writes_from_same_snapshot_commit_once_and_record_once() {
    let (root, db, job) = fixture().await;
    let session = StructureEditSession::new(db.clone());
    let before = db.list_segments(&job).await.unwrap();
    let mut after = before.clone();
    after[0].end_ms = 900;
    let (a, b) = tokio::join!(
        session.save_timing(&job, &before, &after),
        session.save_timing(&job, &before, &after)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(session.stats().await.entries, 1);
    let result = a.or(b).unwrap();
    assert_eq!(session.latest(&job).await, result.undo.latest);
    let undo = session
        .undo(&job, &result.undo.latest.unwrap().token)
        .await
        .unwrap();
    assert_eq!(undo.segments, before);
    close(root, db).await;
}
#[tokio::test]
async fn held_session_gate_blocks_database_write_and_history_access() {
    let (root, db, job) = fixture().await;
    let session = StructureEditSession::new(db.clone());
    let before = db.list_segments(&job).await.unwrap();
    let mut after = before.clone();
    after[0].end_ms = 900;
    let guard = session.history.lock().await;
    let worker = session.clone();
    let task_job = job.clone();
    let task_before = before.clone();
    let mut task =
        tokio::spawn(async move { worker.save_timing(&task_job, &task_before, &after).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), &mut task)
            .await
            .is_err()
    );
    assert_eq!(db.list_segments(&job).await.unwrap(), before);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), session.latest(&job))
            .await
            .is_err()
    );
    drop(guard);
    let result = task.await.unwrap().unwrap();
    assert!(result.undo.latest.is_some());
    assert_eq!(session.forget_job(&job).await, 1);
    assert_eq!(session.stats().await.retained_bytes, 0);
    assert!(
        session
            .undo(&job, &result.undo.latest.unwrap().token)
            .await
            .is_err()
    );
    close(root, db).await;
}
#[tokio::test]
async fn media_preflight_and_transaction_failure_never_publish_tokens() {
    let (root, db, job) = fixture().await;
    let session = StructureEditSession::new(db.clone());
    let before = db.list_segments(&job).await.unwrap();
    let mut insertion = before[0].clone();
    insertion.id = uuid::Uuid::new_v4().to_string();
    // The fixture deliberately has no normalized media: both entry points reject insertion.
    assert!(
        session
            .edit_block(&job, &before, None, Some(insertion.clone()))
            .await
            .is_err()
    );
    assert!(
        LocalWorkspaceService::new(db.clone())
            .edit_subtitle_block(&job, &before, None, Some(insertion))
            .await
            .is_err()
    );
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite:{}", root.join("fixture.sqlite").display()))
            .await
            .unwrap();
    sqlx::query("CREATE TRIGGER fail_session BEFORE INSERT ON local_subtitle_segments BEGIN SELECT RAISE(ABORT,'fixture commit failure'); END").execute(&pool).await.unwrap();
    assert!(
        session
            .edit_block(&job, &before, Some(&before[0].id), None)
            .await
            .is_err()
    );
    assert!(session.latest(&job).await.is_none());
    assert_eq!(session.stats().await.entries, 0);
    assert_eq!(db.list_segments(&job).await.unwrap(), before);
    sqlx::query("DROP TRIGGER fail_session")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let success = session
        .edit_block(&job, &before, Some(&before[0].id), None)
        .await
        .unwrap();
    assert!(success.undo.latest.is_some());
    session.clear().await;
    assert!(session.latest(&job).await.is_none());
    assert_eq!(db.list_segments(&job).await.unwrap(), success.segments);
    close(root, db).await;
}

#[tokio::test]
async fn state_pairs_rows_and_revision_and_hides_history_invalidated_by_text_save() {
    let (root, db, job) = fixture().await;
    let session = StructureEditSession::new(db.clone());
    let initial = session.state(&job).await.unwrap();
    assert!(initial.latest.is_none());
    let mut after = initial.segments.clone();
    after[0].end_ms = 900;
    let saved = session
        .save_timing(&job, &initial.segments, &after)
        .await
        .unwrap();
    let state = session.state(&job).await.unwrap();
    assert_eq!(state.segments, saved.segments);
    assert_eq!(state.workspace_revision, saved.workspace_revision);
    assert_eq!(state.latest, saved.undo.latest);
    let cue = &state.segments[0];
    db.update_segment(
        &job,
        &cue.id,
        "new text".into(),
        cue.translated_text.clone(),
        cue.start_ms,
        cue.end_ms,
    )
    .await
    .unwrap();
    let changed = session.state(&job).await.unwrap();
    assert!(changed.workspace_revision > state.workspace_revision);
    assert_eq!(changed.segments[0].source_text, "new text");
    assert!(changed.latest.is_none());
    assert!(
        session
            .undo(&job, &state.latest.unwrap().token)
            .await
            .is_err()
    );
    assert_eq!(session.stats().await.entries, 1); // conflicts retain the receipt, but don't advertise it as current.
    assert!(session.state("missing").await.is_err());
    close(root, db).await;
}
