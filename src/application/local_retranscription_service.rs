use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    domain::{TranscriptSegment, glossary, segment},
    infrastructure::{
        job_store::Job,
        local_db::{LocalDatabase, LocalSubtitleSegmentRecord},
        media, whisper,
    },
};

#[derive(Debug, Clone, Serialize)]
pub struct LocalRetranscriptionPreview {
    pub preview_id: String,
    pub job_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub original_segments: Vec<LocalSubtitleSegmentRecord>,
    pub candidate_segments: Vec<TranscriptSegment>,
    pub context_mode: &'static str,
}

#[derive(Debug, Clone)]
struct StoredPreview {
    job_id: String,
    start_ms: i64,
    end_ms: i64,
    original_segments: Vec<LocalSubtitleSegmentRecord>,
    candidate_segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone)]
pub struct LocalRetranscriptionService {
    ffmpeg: PathBuf,
    whisper_cli: PathBuf,
    database: LocalDatabase,
    previews: Arc<Mutex<HashMap<String, StoredPreview>>>,
    recognition_lock: Arc<Mutex<()>>,
}

impl LocalRetranscriptionService {
    pub fn new(ffmpeg: PathBuf, whisper_cli: PathBuf, database: LocalDatabase) -> Self {
        Self {
            ffmpeg,
            whisper_cli,
            database,
            previews: Arc::new(Mutex::new(HashMap::new())),
            recognition_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn preview(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<LocalRetranscriptionPreview> {
        if start_ms < 0 || end_ms <= start_ms {
            return Err(anyhow!("selected range end must be after its start"));
        }
        let job_record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        if !matches!(job_record.status.as_str(), "done" | "failed") {
            return Err(anyhow!(
                "selected-range retranscription is only available after the task stops"
            ));
        }
        let all_segments = self.database.list_segments(job_id).await?;
        let original_segments = all_segments
            .into_iter()
            .filter(|item| item.start_ms < end_ms && item.end_ms > start_ms)
            .collect::<Vec<_>>();
        if original_segments
            .iter()
            .any(|item| item.start_ms < start_ms || item.end_ms > end_ms)
        {
            return Err(anyhow!(
                "selected range cuts through an existing subtitle; use complete subtitle boundaries"
            ));
        }

        let job = Job::open(PathBuf::from(&job_record.storage_dir))?;
        let mut options = job.read_recognition_options()?;
        options.max_context = Some(0);
        let run_id = Uuid::new_v4().to_string();
        let temp_dir = std::env::temp_dir().join(format!("atogaki-retranscription-{run_id}"));
        fs::create_dir_all(&temp_dir)
            .with_context(|| format!("failed to create {}", temp_dir.display()))?;
        let _cleanup = TempDirectory(temp_dir.clone());
        let selected_wav = temp_dir.join("selected.wav");
        let output_prefix = temp_dir.join("whisper");

        let _recognition = self.recognition_lock.lock().await;
        media::extract_wav_range(
            &self.ffmpeg,
            &job.audio_wav,
            &selected_wav,
            start_ms as u64,
            end_ms as u64,
        )
        .await?;
        let raw =
            whisper::transcribe(&self.whisper_cli, &options, &selected_wav, &output_prefix).await?;
        let mut candidate_segments = segment::refine(
            glossary::apply_to_segments(&options, raw)?,
            options.source_language,
        );
        for candidate in &mut candidate_segments {
            candidate.start_ms = candidate.start_ms.saturating_add(start_ms as u64);
            candidate.end_ms = candidate.end_ms.saturating_add(start_ms as u64);
            candidate.start_ms = candidate.start_ms.max(start_ms as u64);
            candidate.end_ms = candidate.end_ms.min(end_ms as u64);
        }
        candidate_segments.retain(|candidate| candidate.end_ms > candidate.start_ms);

        let stored = StoredPreview {
            job_id: job_id.to_string(),
            start_ms,
            end_ms,
            original_segments: original_segments.clone(),
            candidate_segments: candidate_segments.clone(),
        };
        self.previews.lock().await.insert(run_id.clone(), stored);
        Ok(LocalRetranscriptionPreview {
            preview_id: run_id,
            job_id: job_id.to_string(),
            start_ms,
            end_ms,
            original_segments,
            candidate_segments,
            context_mode: "isolated",
        })
    }

    pub async fn confirm(
        &self,
        job_id: &str,
        preview_id: &str,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let preview = self
            .previews
            .lock()
            .await
            .get(preview_id)
            .cloned()
            .ok_or_else(|| anyhow!("retranscription preview expired; generate it again"))?;
        if preview.job_id != job_id {
            return Err(anyhow!(
                "retranscription preview belongs to a different task"
            ));
        }
        let updated = self
            .database
            .replace_segment_range(
                job_id,
                preview.start_ms,
                preview.end_ms,
                &preview.original_segments,
                &preview.candidate_segments,
            )
            .await?;
        self.previews.lock().await.remove(preview_id);
        Ok(updated)
    }
}

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
