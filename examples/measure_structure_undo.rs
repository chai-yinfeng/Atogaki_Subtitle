//! Synthetic temporary SQLite benchmark. No models, network or production data.
//! Build first, then time the executable separately to exclude compiler RSS.
use anyhow::{Result, ensure};
use atogaki_subtitle::{
    application::{
        AsrInputScope, AsrRun, CandidateCue, CandidateCueSet, job_manifest::JobManifest,
        job_snapshot::JobSnapshot, job_status::JobStatus,
        structure_undo_history::StructureUndoHistory,
    },
    domain::TranscriptSegment,
    infrastructure::{job_store::Job, local_db::LocalDatabase},
};
use std::{fs, time::Instant};

#[tokio::main]
async fn main() -> Result<()> {
    let count = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "300".into())
        .parse::<usize>()?;
    let bounded = std::env::args().nth(2).as_deref() == Some("bounded");
    ensure!((1..=10_000).contains(&count), "cue count must be 1..10000");
    let root = std::env::temp_dir().join(format!("atogaki-undo-memory-{}", uuid::Uuid::new_v4()));
    let job = Job::create_in(&root)?;
    let db = LocalDatabase::open(root.join("fixture.sqlite")).await?;
    let result: Result<serde_json::Value> = async {
        let mut manifest = JobManifest::new(&job, None, None, Default::default());
        manifest.mark(JobStatus::Done);
        let segments = (0..count).map(|i| {
            let mut cue = TranscriptSegment::new(i as u64 * 2000, i as u64 * 2000 + 1000,
                format!("第 {i} 句：今日は音楽と日常について話します。長い番組の字幕を確認しています。"));
            cue.set_translation(Some(format!("第 {i} 句译文：今天聊聊音乐和日常，继续检查长节目中的上下文与字幕。")));
            cue
        }).collect::<Vec<_>>();
        db.sync_snapshot(&JobSnapshot { manifest, segments: segments.clone() }).await?;
        let mut run = AsrRun::new(job.id(), None, "fixture", "fixture", "synthetic-model",
            job.audio_wav.clone(), AsrInputScope::full(), serde_json::json!({}));
        run.succeed();
        db.record_asr_run(&run, &job.asr_runs_dir.join(&run.id)).await?;
        let candidates = CandidateCueSet {
            schema_version: 1,
            segmentation_policy: "fixture".into(),
            cues: segments.iter().enumerate().map(|(i, cue)| CandidateCue {
                cue: cue.clone(),
                timed_unit_ids: (0..4).map(|j| format!("unit-{i:05}-{j}-0000000000000000")).collect(),
            }).collect(),
        };
        db.assign_asr_cue_provenance(&job.id(), &run.id, &candidates).await?;
        // A linear frozen-context fixture avoids creating a dense N×N group.
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", root.join("fixture.sqlite").display())).await?;
        let mut tx = pool.begin().await?;
        for pair in segments.windows(2) {
            sqlx::query("INSERT INTO local_translation_dependency_edges VALUES (?,?,0)")
                .bind(&pair[1].id).bind(&pair[0].id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        pool.close().await;
        drop(candidates); drop(segments); drop(run);
        let started = Instant::now();
        let mut receipts = Vec::new();
        let mut history = bounded.then(StructureUndoHistory::default);
        let mut sizes = Vec::new();
        let mut discarded = 0;
        let mut before = db.list_segments(&job.id()).await?;
        for i in 0..10 {
            let mut after = before.clone();
            after[0].end_ms = if i % 2 == 0 { 990 } else { 1000 };
            let edit = db.save_segment_timing_recorded(&job.id(), &before, &after).await?;
            sizes.push(edit.retained_bytes());
            before = edit.segments().to_vec();
            if let Some(history) = &mut history {
                discarded += history.record(edit, format!("timing {i}")).discarded_entries;
            } else {
                receipts.push(edit);
            }
            eprintln!("{count} cues: captured {}/10", i + 1);
        }
        let total = sizes.iter().sum::<usize>();
        let stats = history.as_ref().map(StructureUndoHistory::stats);
        let after_clear = history.as_mut().map(|history| { history.clear(); history.stats() });
        let report = serde_json::json!({
            "cue_count": count, "receipt_count": 10,
            "retained_receipt_count": stats.map_or(receipts.len(), |s| s.entries),
            "mode": if bounded { "bounded" } else { "unbounded" },
            "timed_units_per_cue": 4, "dependency_edges": count - 1,
            "receipt_bytes": sizes, "captured_receipt_bytes": total,
            "total_retained_bytes": stats.map_or(total, |s| s.retained_bytes),
            "history_stats": stats, "after_clear": after_clear,
            "discarded_entries": discarded, "capture_seconds": started.elapsed().as_secs_f64(),
            "measurement": "owned capacities, excludes allocator/database/transient overhead"
        });
        drop(receipts);
        Ok(report)
    }.await;
    db.close().await;
    fs::remove_dir_all(root)?;
    println!("{}", serde_json::to_string_pretty(&result?)?);
    Ok(())
}
