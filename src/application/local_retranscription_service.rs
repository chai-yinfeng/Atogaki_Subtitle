use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::{
    application::{
        AsrInputScope, AsrRequest, AsrRun, CandidateCueSet, OfflineAsrProvider, TimedUnit,
        segment_timed_units,
    },
    domain::{TranscriptSegment, glossary},
    infrastructure::{
        asr_run_store::AsrRunArtifacts,
        job_store::Job,
        local_db::{LocalDatabase, LocalSubtitleSegmentRecord},
        media,
        whisper_asr::WhisperAsrProvider,
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
    asr_provider: Arc<dyn OfflineAsrProvider>,
    database: LocalDatabase,
    previews: Arc<Mutex<HashMap<String, StoredPreview>>>,
    recognition_lock: Arc<Mutex<()>>,
}

impl LocalRetranscriptionService {
    pub fn new(ffmpeg: PathBuf, whisper_cli: PathBuf, database: LocalDatabase) -> Self {
        Self {
            ffmpeg,
            asr_provider: Arc::new(WhisperAsrProvider::new(whisper_cli)),
            database,
            previews: Arc::new(Mutex::new(HashMap::new())),
            recognition_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn with_asr_provider(mut self, provider: Arc<dyn OfflineAsrProvider>) -> Self {
        self.asr_provider = provider;
        self
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
        options.whisper.max_context = Some(0);
        let parent_run_id = self
            .database
            .list_asr_runs(job_id)
            .await?
            .into_iter()
            .find(|run| run.status == "succeeded")
            .map(|run| run.id);
        let provider = self.asr_provider.status();
        let scope = AsrInputScope::selected_range(start_ms as u64, end_ms as u64)?;
        let mut run = AsrRun::new(
            job_id,
            parent_run_id,
            provider.id,
            provider.name,
            options.whisper.model.display().to_string(),
            job_record
                .input_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| job.audio_wav.clone()),
            scope.clone(),
            serde_json::to_value(&options).context("failed to snapshot recognition options")?,
        );
        let artifacts = AsrRunArtifacts::create(&job, &run)?;
        self.database.record_asr_run(&run, &artifacts.dir).await?;

        let run_id = run.id.clone();
        let temp_dir = std::env::temp_dir().join(format!("atogaki-retranscription-{run_id}"));
        fs::create_dir_all(&temp_dir)
            .with_context(|| format!("failed to create {}", temp_dir.display()))?;
        let _cleanup = TempDirectory(temp_dir.clone());
        let selected_wav = temp_dir.join("selected.wav");

        let _recognition = self.recognition_lock.lock().await;
        run.start();
        artifacts.write_run(&run)?;
        self.database.update_asr_run(&run).await?;
        let recognition: Result<(CandidateCueSet, Vec<TimedUnit>)> = async {
            media::extract_wav_range(
                &self.ffmpeg,
                &job.audio_wav,
                &selected_wav,
                start_ms as u64,
                end_ms as u64,
            )
            .await?;
            let response = self
                .asr_provider
                .transcribe(AsrRequest {
                    audio_path: selected_wav,
                    output_prefix: artifacts.provider_output_prefix.clone(),
                    scope,
                    transcription: options.clone(),
                })
                .await?;
            let mut timed_units = response.timed_units;
            for unit in &mut timed_units {
                unit.start_ms = unit
                    .start_ms
                    .map(|value| value.saturating_add(start_ms as u64).max(start_ms as u64));
                unit.end_ms = unit
                    .end_ms
                    .map(|value| value.saturating_add(start_ms as u64).min(end_ms as u64));
            }
            timed_units.retain(|unit| match (unit.start_ms, unit.end_ms) {
                (Some(start), Some(end)) => end > start,
                _ => true,
            });
            let mut cue_set = segment_timed_units(&timed_units, options.source_language)?;
            let candidates = glossary::apply_to_segments(&options, cue_set.transcript_segments())?;
            cue_set.replace_transcript_segments(candidates)?;
            Ok((cue_set, timed_units))
        }
        .await;
        let (cue_set, timed_units) = match recognition {
            Ok(result) => result,
            Err(error) => {
                run.fail(format!("{error:#}"));
                let _ = artifacts.write_run(&run);
                let _ = self.database.update_asr_run(&run).await;
                return Err(error);
            }
        };
        let candidate_segments = cue_set.transcript_segments();
        let completion: Result<()> = async {
            artifacts.write_timed_units(&timed_units)?;
            artifacts.write_candidate_cues(&cue_set)?;
            run.succeed();
            artifacts.write_run(&run)?;
            self.database.update_asr_run(&run).await
        }
        .await;
        if let Err(error) = completion {
            run.fail(format!("{error:#}"));
            let _ = artifacts.write_run(&run);
            let _ = self.database.update_asr_run(&run).await;
            return Err(error);
        }

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
        let job_record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let job = Job::open(PathBuf::from(job_record.storage_dir))?;
        let artifacts = AsrRunArtifacts::open(&job, preview_id)?;
        let cue_set = artifacts.read_candidate_cues()?;
        let updated = self
            .database
            .replace_segment_range(
                job_id,
                preview.start_ms,
                preview.end_ms,
                &preview.original_segments,
                &preview.candidate_segments,
                Some((preview_id, &cue_set)),
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
