//! Read-only task cards. Checkpoint progress is an index hint, never recovery evidence.
use std::{collections::HashMap, fs, path::PathBuf};

use serde::Serialize;

use crate::{
    application::{acoustic_plan::TaskRecoverySummary, job_manifest::ProcessingTime},
    infrastructure::{
        job_store::Job,
        local_db::{LocalJobRecord, LocalJobTranslationStats},
    },
};

#[derive(Debug, Serialize)]
pub struct JobListSummary {
    #[serde(flatten)]
    pub job: LocalJobRecord,
    pub processing_time: Option<ProcessingTime>,
    pub recovery: Option<TaskRecoverySummary>,
    /// The progress index has not been checked against raw ASR evidence.
    pub recovery_progress_unverified: bool,
    pub translation_status: &'static str,
    pub segment_count: i64,
    pub translated_segment_count: i64,
    pub stale_translation_count: i64,
}

impl JobListSummary {
    pub fn read(
        job: LocalJobRecord,
        stats: Option<LocalJobTranslationStats>,
        ranges: Vec<(i64, i64)>,
    ) -> Self {
        let manifest = Job::open(PathBuf::from(&job.storage_dir))
            .ok()
            .and_then(|job| job.read_manifest_if_exists().ok().flatten());
        let processing_time = manifest.as_ref().and_then(|m| m.processing_time.clone());
        let dir = PathBuf::from(&job.storage_dir);
        let recovery = if job.status == "done" {
            None
        } else {
            let resumable = dir.join("workflow-version.json").is_file()
                || dir.join("asr-checkpoint/plan.json").is_file();
            // Only a small plan header and progress counters. No segments,
            // evidence JSON, model fingerprint or raw provider output is read.
            #[derive(serde::Deserialize)]
            struct PlanHeader {
                duration_ms: u64,
            }
            let duration_ms = fs::read(dir.join("asr-checkpoint/plan.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<PlanHeader>(&bytes).ok())
                .map_or(0, |plan| plan.duration_ms);
            let completed_ranges: Vec<_> = ranges
                .into_iter()
                .filter(|(start, end)| *start >= 0 && end > start && *end as u64 <= duration_ms)
                .map(|(start, end)| (start as u64, end as u64))
                .collect();
            Some(TaskRecoverySummary {
                phase: if resumable {
                    manifest
                        .as_ref()
                        .map(|m| m.failed_stage.unwrap_or(m.status).as_str().to_string())
                        .unwrap_or_else(|| job.status.clone())
                } else {
                    "legacy".into()
                },
                completed_ms: completed_ranges
                    .iter()
                    .map(|(start, end)| end - start)
                    .sum::<u64>()
                    .min(duration_ms),
                duration_ms,
                completed_ranges,
                can_resume: resumable && job.status == "failed",
                blocked_reason: (!resumable).then(|| "该历史任务无法断点继续；可重新运行".into()),
                early_translation: fs::read(dir.join("early-translation/progress.json"))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok()),
            })
        };
        let stats = stats.unwrap_or(LocalJobTranslationStats {
            job_id: job.job_id.clone(),
            segment_count: 0,
            translated_segment_count: 0,
            stale_translation_count: 0,
        });
        let translation_status = if stats.segment_count == 0 {
            "not_ready"
        } else if stats.stale_translation_count > 0 {
            "stale"
        } else if stats.translated_segment_count == 0 {
            "untranslated"
        } else if stats.translated_segment_count < stats.segment_count {
            "partial"
        } else {
            "translated"
        };
        Self {
            recovery_progress_unverified: recovery.is_some(),
            job,
            processing_time,
            recovery,
            translation_status,
            segment_count: stats.segment_count,
            translated_segment_count: stats.translated_segment_count,
            stale_translation_count: stats.stale_translation_count,
        }
    }
}

pub fn by_job(stats: Vec<LocalJobTranslationStats>) -> HashMap<String, LocalJobTranslationStats> {
    stats
        .into_iter()
        .map(|row| (row.job_id.clone(), row))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::JobListSummary;
    use crate::{
        application::{JobSnapshot, job_manifest::JobManifest, job_status::JobStatus},
        domain::{LanguagePair, TranscriptSegment},
        infrastructure::{job_store::Job, local_db::LocalDatabase},
    };
    use std::fs;

    #[tokio::test]
    async fn card_read_does_not_parse_corrupt_segments_or_validate_checkpoint_evidence() {
        let root = std::env::temp_dir().join(format!("atogaki-summary-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root.join("jobs")).unwrap();
        let db = LocalDatabase::open(root.join("state.sqlite"))
            .await
            .unwrap();
        let mut manifest = JobManifest::new(&job, None, None, LanguagePair::default());
        manifest.mark(JobStatus::Transcribing);
        manifest.fail("fixture stopped");
        job.write_manifest(&manifest).unwrap();
        db.sync_snapshot(&JobSnapshot {
            manifest,
            segments: vec![TranscriptSegment::new(0, 1000, "fixture".into())],
        })
        .await
        .unwrap();
        fs::write(&job.segments_json, b"corrupt segments").unwrap();
        let checkpoints = job.dir.join("asr-checkpoint");
        fs::create_dir_all(checkpoints.join("chunk-000000")).unwrap();
        let plan = crate::application::acoustic_plan::AcousticPlan::new(
            120000,
            "audio".into(),
            "config".into(),
        );
        fs::write(
            checkpoints.join("plan.json"),
            serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
        fs::write(
            checkpoints.join("chunk-000000/evidence.json"),
            b"corrupt evidence",
        )
        .unwrap();
        // A directory cannot be hashed as a raw provider-output file. Card reads
        // must still work; the authoritative recovery path rejects this chunk.
        fs::create_dir(checkpoints.join("chunk-000000/provider-output.json")).unwrap();
        let record = db.get_job(&job.id()).await.unwrap().unwrap();
        let stats = db.list_job_translation_stats().await.unwrap().pop();
        let card = JobListSummary::read(record, stats, vec![(0, 120000)]);
        assert_eq!(card.segment_count, 1);
        assert_eq!(card.translation_status, "untranslated");
        assert!(card.recovery_progress_unverified);
        assert_eq!(card.recovery.as_ref().unwrap().completed_ms, 120000);
        assert!(card.recovery.as_ref().unwrap().can_resume);
        assert_eq!(
            crate::application::acoustic_plan::recovery_summary(&job)
                .unwrap()
                .completed_ms,
            0
        );
        assert_eq!(fs::read(&job.segments_json).unwrap(), b"corrupt segments");
        let service = crate::application::LocalTaskService::with_workers_and_database(
            crate::infrastructure::config::AppConfig {
                ffmpeg: "ffmpeg".into(),
                whisper_cli: "whisper-cli".into(),
                deepl_auth_key: None,
            },
            root.join("jobs"),
            1,
            8,
            Some(db.clone()),
        )
        .unwrap();
        // An unchanged task must also open from SQLite despite a damaged
        // historical snapshot; it must not scan the whole task collection.
        service.refresh_persisted_job(&job.id()).await.unwrap();
        let cards = service.list_job_summaries().await.unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].segment_count, 1);
        assert_eq!(cards[0].job.status, "failed");
        assert_eq!(
            db.list_segments(&job.id()).await.unwrap()[0].source_text,
            "fixture"
        );
        drop(service);
        db.close().await;
        fs::remove_dir_all(root).unwrap();
    }
}
