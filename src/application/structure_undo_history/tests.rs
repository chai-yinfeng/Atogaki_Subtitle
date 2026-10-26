use super::*;
use crate::{
    application::{job_manifest::JobManifest, job_snapshot::JobSnapshot, job_status::JobStatus},
    domain::TranscriptSegment,
    infrastructure::job_store::Job,
};
use std::{fs, path::PathBuf};
async fn fixture() -> (PathBuf, LocalDatabase, String) {
    let root =
        std::env::temp_dir().join(format!("atogaki-history-session-{}", uuid::Uuid::new_v4()));
    let db = LocalDatabase::open(root.join("fixture.sqlite"))
        .await
        .unwrap();
    let job = add_job(&root, &db).await;
    (root, db, job)
}
async fn add_job(root: &std::path::Path, db: &LocalDatabase) -> String {
    let job = Job::create_in(root).unwrap();
    let mut manifest = JobManifest::new(&job, None, None, Default::default());
    manifest.mark(JobStatus::Done);
    db.sync_snapshot(&JobSnapshot {
        manifest,
        segments: vec![
            TranscriptSegment::new(0, 1000, "context".into()),
            TranscriptSegment::new(2000, 3000, "target".into()),
        ],
    })
    .await
    .unwrap();
    job.id()
}
async fn edit(db: &LocalDatabase, job: &str) -> LocalStructureEdit {
    let before = db.list_segments(job).await.unwrap();
    let mut after = before.clone();
    after[0].end_ms = if before[0].end_ms == 1000 { 990 } else { 1000 };
    db.save_segment_timing_recorded(job, &before, &after)
        .await
        .unwrap()
}
async fn close(root: PathBuf, db: LocalDatabase) {
    db.close().await;
    fs::remove_dir_all(root).unwrap();
}
fn entry_bytes(edit: &LocalStructureEdit, label: &str) -> usize {
    edit.retained_bytes() + label.len() + std::mem::size_of::<Entry>()
        - std::mem::size_of::<LocalStructureEdit>()
}

#[tokio::test]
async fn ten_entry_limit_and_continuous_undos_keep_expected_revision_and_free_all_nodes() {
    let (root, db, job) = fixture().await;
    let mut history = StructureUndoHistory::with_budget(usize::MAX);
    for i in 0..12 {
        let result = history.record(edit(&db, &job).await, format!("edit {i}"));
        assert!(!result.oversized);
        assert_eq!(result.discarded_entries, usize::from(i >= 10));
        assert!(history.stats().entries <= 10);
    }
    assert_eq!(history.stats().entries, 10);
    for i in (2..12).rev() {
        let top = history.latest(&job).unwrap();
        assert_eq!(top.label, format!("edit {i}"));
        let result = history.undo(&db, &job, &top.token).await.unwrap();
        assert_eq!(result.discarded_entries, 0);
        if let Some(next) = result.latest {
            assert_eq!(next.expected_revision, result.workspace_revision);
        }
    }
    assert_eq!(history.stats().retained_bytes, 0);
    assert_eq!(history.stats().jobs, 0);
    assert!(history.heads.is_empty());
    assert_eq!(db.list_segments(&job).await.unwrap()[0].end_ms, 1000);
    close(root, db).await;
}
#[tokio::test]
async fn global_budget_evicts_oldest_head_across_jobs_without_breaking_latest_undo() {
    let (root, db, a) = fixture().await;
    let b = add_job(&root, &db).await;
    let first = edit(&db, &a).await;
    let second = edit(&db, &b).await;
    let budget = entry_bytes(&first, "A") + entry_bytes(&second, "B");
    let mut history = StructureUndoHistory::with_budget(budget);
    history.record(first, "A".into());
    history.record(second, "B".into());
    assert_eq!(history.stats().entries, 2);
    let outcome = history.record(edit(&db, &a).await, "C".into());
    assert_eq!(outcome.discarded_entries, 1);
    assert!(history.stats().retained_bytes <= budget);
    assert_eq!(history.stats().entries, 2);
    let a_top = history.latest(&a).unwrap();
    assert_eq!(a_top.label, "C");
    let result = history.undo(&db, &a, &a_top.token).await.unwrap();
    assert!(result.latest.is_none());
    assert_eq!(history.latest(&b).unwrap().label, "B");
    assert_eq!(history.forget_job(&b), 1);
    assert_eq!(history.stats().retained_bytes, 0);
    assert!(history.heads.is_empty());
    close(root, db).await;
}
#[tokio::test]
async fn oversized_committed_edit_drops_only_its_job_history_and_exposes_no_stale_token() {
    let (root, db, a) = fixture().await;
    let b = add_job(&root, &db).await;
    let first = edit(&db, &a).await;
    let budget = entry_bytes(&first, "A") * 3;
    let mut history = StructureUndoHistory::with_budget(budget);
    let old = history.record(first, "A".into()).latest.unwrap();
    history.record(edit(&db, &b).await, "B".into());
    let result = history.record(edit(&db, &a).await, "x".repeat(budget + 1));
    assert!(result.oversized);
    assert!(result.latest.is_none());
    assert_eq!(result.discarded_entries, 1);
    assert!(history.undo(&db, &a, &old.token).await.is_err());
    assert!(history.latest(&b).is_some());
    assert_eq!(db.list_segments(&a).await.unwrap()[0].end_ms, 1000);
    assert!(history.stats().retained_bytes <= budget);
    history.clear();
    assert!(history.heads.is_empty());
    assert_eq!(history.stats().jobs, 0);
    assert_eq!(history.stats().retained_bytes, 0);
    close(root, db).await;
}
#[tokio::test]
async fn wrong_job_outdated_and_replayed_tokens_never_touch_the_database() {
    let (root, db, a) = fixture().await;
    let b = add_job(&root, &db).await;
    let mut history = StructureUndoHistory::default();
    let first = history
        .record(edit(&db, &a).await, "A".into())
        .latest
        .unwrap();
    let second = history
        .record(edit(&db, &a).await, "B".into())
        .latest
        .unwrap();
    let before = db.subtitle_workspace_snapshot(&a).await.unwrap();
    assert!(history.undo(&db, &a, &first.token).await.is_err());
    assert!(history.undo(&db, &b, &second.token).await.is_err());
    assert_eq!(db.subtitle_workspace_snapshot(&a).await.unwrap(), before);
    history.undo(&db, &a, &second.token).await.unwrap();
    assert!(history.undo(&db, &a, &second.token).await.is_err());
    close(root, db).await;
}
#[tokio::test]
async fn intervening_text_edit_starts_new_chain_and_preserves_the_new_text_on_undo() {
    let (root, db, job) = fixture().await;
    let mut history = StructureUndoHistory::default();
    history.record(edit(&db, &job).await, "first".into());
    let cue = db.list_segments(&job).await.unwrap().remove(0);
    db.update_segment(
        &job,
        &cue.id,
        "new text".into(),
        cue.translated_text,
        cue.start_ms,
        cue.end_ms,
    )
    .await
    .unwrap();
    let result = history.record(edit(&db, &job).await, "second".into());
    assert_eq!(result.discarded_entries, 1);
    let restored = history
        .undo(&db, &job, &result.latest.unwrap().token)
        .await
        .unwrap();
    assert_eq!(restored.segments[0].source_text, "new text");
    assert!(restored.latest.is_none());
    close(root, db).await;
}
#[tokio::test]
async fn failed_undo_retains_receipt_for_retry_and_success_releases_it() {
    let (root, db, job) = fixture().await;
    let mut history = StructureUndoHistory::default();
    let top = history
        .record(edit(&db, &job).await, "edit".into())
        .latest
        .unwrap();
    let stats = history.stats();
    let before = db.subtitle_workspace_snapshot(&job).await.unwrap();
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite:{}", root.join("fixture.sqlite").display()))
            .await
            .unwrap();
    sqlx::query("CREATE TRIGGER fail_history_undo BEFORE INSERT ON local_subtitle_segments BEGIN SELECT RAISE(ABORT,'fixture undo failure'); END").execute(&pool).await.unwrap();
    assert!(history.undo(&db, &job, &top.token).await.is_err());
    assert_eq!(history.stats(), stats);
    assert_eq!(history.latest(&job).unwrap(), top);
    assert_eq!(db.subtitle_workspace_snapshot(&job).await.unwrap(), before);
    sqlx::query("DROP TRIGGER fail_history_undo")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    history.undo(&db, &job, &top.token).await.unwrap();
    assert_eq!(history.stats().entries, 0);
    assert_eq!(history.stats().retained_bytes, 0);
    close(root, db).await;
}
#[tokio::test]
async fn clearing_or_recreating_session_does_not_restore_sqlite_or_reuse_old_tokens() {
    let (root, db, job) = fixture().await;
    let mut history = StructureUndoHistory::default();
    let token = history
        .record(edit(&db, &job).await, "edit".into())
        .latest
        .unwrap()
        .token;
    let saved = db.subtitle_workspace_snapshot(&job).await.unwrap();
    history.clear();
    assert!(history.undo(&db, &job, &token).await.is_err());
    let mut restarted = StructureUndoHistory::default();
    assert!(restarted.undo(&db, &job, &token).await.is_err());
    assert_eq!(db.subtitle_workspace_snapshot(&job).await.unwrap(), saved);
    assert_eq!(restarted.stats().retained_bytes, 0);
    close(root, db).await;
}
