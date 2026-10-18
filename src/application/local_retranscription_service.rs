use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};

use crate::{
    application::{
        AsrInputScope, AsrProvider, AsrRequest, AsrRun, CandidateCueSet, RepairAttemptStatus,
        TimedUnit, detect_quality_signals, segment_timed_units,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRetranscriptionPreview {
    pub preview_id: String,
    pub job_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub original_segments: Vec<LocalSubtitleSegmentRecord>,
    pub candidate_segments: Vec<TranscriptSegment>,
    pub context_mode: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model_identity: String,
    pub timing_granularities: Vec<crate::application::AsrTimingGranularity>,
    pub configuration: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct LocalRetranscriptionService {
    ffmpeg: PathBuf,
    asr_provider: Arc<dyn AsrProvider>,
    database: LocalDatabase,
    recognition_lock: Arc<Mutex<()>>,
    cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
}

impl LocalRetranscriptionService {
    pub fn new(ffmpeg: PathBuf, whisper_cli: PathBuf, database: LocalDatabase) -> Self {
        Self {
            ffmpeg,
            asr_provider: Arc::new(WhisperAsrProvider::new(whisper_cli)),
            database,
            recognition_lock: Arc::new(Mutex::new(())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_asr_provider(mut self, provider: Arc<dyn AsrProvider>) -> Self {
        self.asr_provider = provider;
        self
    }

    pub async fn cancel(&self, job_id: &str) -> Result<()> {
        let cancellations = self.cancellations.lock().await;
        cancellations
            .get(job_id)
            .ok_or_else(|| anyhow!("没有运行中的候选"))?
            .send_replace(true);
        Ok(())
    }

    pub async fn preview(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<LocalRetranscriptionPreview> {
        self.preview_with_provider(
            job_id,
            start_ms,
            end_ms,
            Arc::clone(&self.asr_provider),
            false,
        )
        .await
    }

    pub async fn preview_with_provider(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
        asr_provider: Arc<dyn AsrProvider>,
        cloud_audio_upload_authorized: bool,
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
        let provider = asr_provider.status();
        let model_identity = if provider.id == "whisper.cpp" {
            options.whisper.model.display().to_string()
        } else {
            provider.name.clone()
        };
        let scope = if provider.id == "whisper.cpp"
            && start_ms == 0
            && crate::application::acoustic_plan::wav_duration(&job.audio_wav)? == end_ms as u64
        {
            AsrInputScope::full()
        } else {
            AsrInputScope::selected_range(start_ms as u64, end_ms as u64)?
        };
        let config_snapshot = if provider.id == "gemini-transcribe" {
            serde_json::json!({
                "language": options.source_language,
                "mode": "verbatim",
                "word_timestamps": true,
                "speaker_diarization": false,
                "cloud_audio_upload_authorized": cloud_audio_upload_authorized,
            })
        } else {
            serde_json::to_value(&options).context("failed to snapshot recognition options")?
        };
        let metadata = (
            provider.id.clone(),
            provider.name.clone(),
            model_identity.clone(),
            provider.capabilities.timing_granularities.clone(),
            config_snapshot.clone(),
        );
        let mut run = AsrRun::new(
            job_id,
            parent_run_id,
            provider.id,
            provider.name,
            model_identity,
            job_record
                .input_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| job.audio_wav.clone()),
            scope.clone(),
            config_snapshot,
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
        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        self.cancellations
            .lock()
            .await
            .insert(job_id.into(), cancel_tx);
        let cloud = metadata.0 == "gemini-transcribe";
        let recognition_future = async {
            media::extract_wav_range(
                &self.ffmpeg,
                &job.audio_wav,
                &selected_wav,
                start_ms as u64,
                end_ms as u64,
            )
            .await?;
            let response = if metadata.0 == "whisper.cpp" {
                use sha2::{Digest, Sha256};
                let key = serde_json::to_vec(
                    &serde_json::json!({"start":start_ms,"end":end_ms,"configuration":options,"model_sha256":crate::application::acoustic_plan::file_digest(&options.whisper.model)?}),
                )?;
                let root = job
                    .dir
                    .join("candidate-checkpoints")
                    .join(format!("{:x}", Sha256::digest(key)));
                let scratch = Job::create(Some(&root))?;
                crate::application::acoustic_plan::prepare_audio(
                    &self.ffmpeg,
                    &selected_wav,
                    &scratch,
                )
                .await?;
                crate::application::acoustic_plan::execute_chunks(
                    &scratch,
                    &self.ffmpeg,
                    Arc::clone(&asr_provider),
                    &options,
                    &run.id,
                    None,
                )
                .await?
            } else {
                asr_provider
                    .transcribe(AsrRequest {
                        audio_path: selected_wav,
                        output_prefix: artifacts.provider_output_prefix.clone(),
                        scope,
                        cloud_audio_upload_authorized,
                        transcription: options.clone(),
                    })
                    .await?
            };
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
            let mut cue_set = if metadata.0 == "whisper.cpp" {
                crate::application::acoustic_plan::candidate_cues(&timed_units)
            } else {
                segment_timed_units(&timed_units, options.source_language)?
            };
            let candidates = glossary::apply_to_segments(&options, cue_set.transcript_segments())?;
            cue_set.replace_transcript_segments(candidates)?;
            Ok((cue_set, timed_units))
        };
        let recognition: Result<(CandidateCueSet, Vec<TimedUnit>)> = if cloud {
            // Let the provider finish its upload deletion before returning cancellation.
            let result = recognition_future.await;
            if *cancel_rx.borrow() {
                Err(anyhow!("候选已取消；云端上传清理已执行"))
            } else {
                result
            }
        } else {
            tokio::select! { result=recognition_future => result, _=cancel_rx.changed()=>Err(anyhow!("候选已取消；已有检查点保留")) }
        };
        self.cancellations.lock().await.remove(job_id);
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
            let mut signals =
                detect_quality_signals(job_id, &run.id, &cue_set, run.created_at_unix);
            signals.extend(crate::application::acoustic_plan::boundary_signals(
                job_id,
                &run.id,
                &timed_units,
                run.created_at_unix,
            ));
            artifacts.write_quality_signals(&signals)?;
            self.database
                .replace_asr_quality_signals(&run.id, &signals)
                .await?;
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

        let preview = LocalRetranscriptionPreview {
            preview_id: run_id,
            job_id: job_id.into(),
            start_ms,
            end_ms,
            original_segments,
            candidate_segments,
            context_mode: if metadata.0 == "whisper.cpp" {
                "isolated".into()
            } else {
                "provider-managed".into()
            },
            provider_id: metadata.0,
            provider_name: metadata.1,
            model_identity: metadata.2,
            timing_granularities: metadata.3,
            configuration: metadata.4,
        };
        crate::application::acoustic_plan::write_json(
            &artifacts.dir.join("preview.json"),
            &preview,
        )?;
        Ok(preview)
    }

    pub async fn reopen(
        &self,
        job_id: &str,
        preview_id: &str,
    ) -> Result<LocalRetranscriptionPreview> {
        let record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        let job = Job::open(record.storage_dir.into())?;
        let artifacts = AsrRunArtifacts::open(&job, preview_id)?;
        let preview: LocalRetranscriptionPreview =
            serde_json::from_slice(&fs::read(artifacts.dir.join("preview.json"))?)?;
        if preview.job_id != job_id || preview.preview_id != preview_id {
            return Err(anyhow!("candidate identity mismatch"));
        }
        Ok(preview)
    }

    pub async fn preview_full(&self, job_id: &str) -> Result<LocalRetranscriptionPreview> {
        let record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        let job = Job::open(record.storage_dir.into())?;
        let duration = crate::application::acoustic_plan::wav_duration(&job.audio_wav)?;
        self.preview(job_id, 0, i64::try_from(duration)?).await
    }

    pub async fn confirm(
        &self,
        job_id: &str,
        preview_id: &str,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let preview = self.reopen(job_id, preview_id).await?;
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
        self.database
            .mark_asr_repair_attempts_for_run(preview_id, RepairAttemptStatus::Adopted)
            .await?;
        crate::application::acoustic_plan::write_json(&artifacts.dir.join("adopted.json"), &true)?;
        Ok(updated)
    }
}

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            AsrFuture, AsrProviderStatus, AsrResponse, LocalWorkspaceService, TimedUnitKind,
            TimingSource, TranscriptionOptions,
        },
        application::{
            job_manifest::JobManifest, job_snapshot::JobSnapshot, job_status::JobStatus,
        },
        domain::{LanguagePair, TranscriptSegment},
    };
    #[derive(Debug)]
    struct Fixture;
    impl AsrProvider for Fixture {
        fn status(&self) -> AsrProviderStatus {
            let mut status = WhisperAsrProvider::new("unused").status();
            status.id = "fixture".into();
            status.name = "Fixture".into();
            status
        }
        fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
            Box::pin(async move {
                let raw = request.output_prefix.with_extension("json");
                fs::write(&raw, b"{}")?;
                Ok(AsrResponse {
                    provider_id: "fixture".into(),
                    model_identity: "fixture".into(),
                    raw_output_path: raw,
                    legacy_segments: vec![],
                    timed_units: vec![TimedUnit {
                        id: "candidate-unit".into(),
                        text: "候補".into(),
                        kind: TimedUnitKind::ProviderSegment,
                        start_ms: Some(100),
                        end_ms: Some(900),
                        timing_source: TimingSource::Model,
                        provider_confidence: None,
                        speaker: None,
                    }],
                })
            })
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn persisted_candidate_reopens_after_restart_and_rejects_new_edits() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "atogaki-persisted-candidate-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let ffmpeg = root.join("ffmpeg");
        fs::write(
            &ffmpeg,
            "#!/bin/sh\nfor destination do :; done\n: > \"$destination\"\n",
        )
        .unwrap();
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
        let original = TranscriptSegment::new(0, 1000, "元の文".into());
        job.write_segments(&[original.clone()]).unwrap();
        job.write_recognition_options(&TranscriptionOptions::japanese(root.join("model")))
            .unwrap();
        fs::write(&job.audio_wav, b"fixture").unwrap();
        let mut manifest = JobManifest::new(&job, None, None, LanguagePair::default());
        manifest.mark(JobStatus::Done);
        job.write_manifest(&manifest).unwrap();
        let db = LocalDatabase::open(root.join("state.sqlite"))
            .await
            .unwrap();
        db.sync_snapshot(&JobSnapshot::load(&job.dir).unwrap())
            .await
            .unwrap();
        let service = LocalRetranscriptionService::new(ffmpeg.clone(), "unused".into(), db.clone())
            .with_asr_provider(Arc::new(Fixture));
        let preview = service.preview(&job.id(), 0, 1000).await.unwrap();
        drop(service);
        let reopened = LocalRetranscriptionService::new(ffmpeg, "unused".into(), db.clone())
            .with_asr_provider(Arc::new(Fixture));
        assert_eq!(
            reopened
                .reopen(&job.id(), &preview.preview_id)
                .await
                .unwrap()
                .provider_name,
            "Fixture"
        );
        LocalWorkspaceService::new(db.clone())
            .update_subtitle(&job.id(), &original.id, "人工修正".into(), None, 0, 1000)
            .await
            .unwrap();
        assert!(
            reopened
                .confirm(&job.id(), &preview.preview_id)
                .await
                .is_err()
        );
        assert_eq!(
            db.list_segments(&job.id()).await.unwrap()[0].source_text,
            "人工修正"
        );
        let new = reopened.preview(&job.id(), 0, 1000).await.unwrap();
        assert_eq!(
            reopened.confirm(&job.id(), &new.preview_id).await.unwrap()[0].source_text,
            "候補"
        );
        assert!(reopened.confirm(&job.id(), &new.preview_id).await.is_err());
        db.close().await;
        fs::remove_dir_all(root).unwrap();
    }
}
