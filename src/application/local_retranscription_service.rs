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
    #[serde(default)]
    pub comparison_id: Option<String>,
    #[serde(default)]
    pub workspace_revision: Option<i64>,
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
    artifact_lock: Arc<Mutex<()>>,
    cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    scheduler: Option<Arc<crate::application::stage_scheduler::StageScheduler>>,
    structure_edits: Option<super::structure_edit_session::StructureEditSession>,
}

impl LocalRetranscriptionService {
    pub fn new(ffmpeg: PathBuf, whisper_cli: PathBuf, database: LocalDatabase) -> Self {
        Self {
            ffmpeg,
            asr_provider: Arc::new(WhisperAsrProvider::new(whisper_cli)),
            database,
            recognition_lock: Arc::new(Mutex::new(())),
            artifact_lock: Arc::new(Mutex::new(())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            scheduler: None,
            structure_edits: None,
        }
    }

    /// Inject the workspace's existing session, created from the same database.
    pub fn with_structure_edits(
        mut self,
        session: super::structure_edit_session::StructureEditSession,
    ) -> Self {
        self.structure_edits = Some(session);
        self
    }

    pub fn with_asr_provider(mut self, provider: Arc<dyn AsrProvider>) -> Self {
        self.asr_provider = provider;
        self
    }

    pub fn with_stage_scheduler(
        mut self,
        scheduler: Arc<crate::application::stage_scheduler::StageScheduler>,
    ) -> Self {
        self.scheduler = Some(scheduler);
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
        let (workspace_revision, all_segments) =
            self.database.subtitle_workspace_snapshot(job_id).await?;
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
        crate::application::acoustic_plan::write_json(
            &artifacts.dir.join("candidate.json"),
            &true,
        )?;
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
        crate::application::acoustic_plan::write_json(
            &artifacts.dir.join("progress.json"),
            &serde_json::json!({"phase":"extracting_audio"}),
        )?;
        let recognition_future = async {
            let _stage_permit = if !cloud {
                if let Some(scheduler) = &self.scheduler {
                    Some(scheduler.acquire_asr().await)
                } else {
                    None
                }
            } else {
                None
            };
            media::extract_wav_range(
                &self.ffmpeg,
                &job.audio_wav,
                &selected_wav,
                start_ms as u64,
                end_ms as u64,
            )
            .await?;
            crate::application::acoustic_plan::write_json(
                &artifacts.dir.join("progress.json"),
                &serde_json::json!({"phase":"transcribing"}),
            )?;
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
                crate::application::acoustic_plan::write_json(
                    &artifacts.dir.join("checkpoint-root.json"),
                    &root,
                )?;
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
            run.model_identity = response.model_identity.clone();
            crate::application::acoustic_plan::write_json(
                &artifacts.dir.join("progress.json"),
                &serde_json::json!({"phase":"refining_segments"}),
            )?;
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
        if let Ok(bytes) = fs::read(artifacts.dir.join("checkpoint-root.json")) {
            if let Ok(root) = serde_json::from_slice::<PathBuf>(&bytes) {
                if let Ok(scratch) = Job::open(root) {
                    if let Ok(summary) =
                        crate::application::acoustic_plan::recovery_summary(&scratch)
                    {
                        crate::application::acoustic_plan::write_json(
                            &artifacts.dir.join("checkpoints.json"),
                            &summary,
                        )?;
                    }
                }
            }
        }
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
            if metadata.0 == "whisper.cpp" {
                let root: PathBuf =
                    serde_json::from_slice(&fs::read(artifacts.dir.join("checkpoint-root.json"))?)?;
                signals.extend(crate::application::acoustic_plan::stitching_signals(
                    &Job::open(root)?,
                    job_id,
                    &run.id,
                    run.created_at_unix,
                    start_ms as u64,
                )?);
            }
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
            workspace_revision: Some(workspace_revision),
            preview_id: run_id,
            comparison_id: None,
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
            model_identity: run.model_identity.clone(),
            timing_granularities: metadata.3,
            configuration: metadata.4,
        };
        crate::application::acoustic_plan::write_json(
            &artifacts.dir.join("preview.json"),
            &preview,
        )?;
        crate::application::acoustic_plan::write_json(
            &artifacts.dir.join("progress.json"),
            &serde_json::json!({"phase":"candidate_ready"}),
        )?;
        Ok(preview)
    }

    pub async fn reopen(&self, job_id: &str, run_id: &str) -> Result<LocalRetranscriptionPreview> {
        let _guard = self.artifact_lock.lock().await;
        self.read_preview(job_id, run_id).await
    }

    async fn read_preview(
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
        if artifacts.dir.join("released.json").exists() {
            return Err(anyhow!("候选产物已清理，无法重新打开"));
        }
        let preview: LocalRetranscriptionPreview = if artifacts.dir.join("preview.json").is_file() {
            serde_json::from_slice(&fs::read(artifacts.dir.join("preview.json"))?)?
        } else {
            let run = artifacts.read_run()?;
            if run.status != crate::application::AsrRunStatus::Succeeded {
                return Err(anyhow!("候选尚未完成"));
            }
            let candidates = artifacts.read_candidate_cues()?.transcript_segments();
            let (workspace_revision, all) =
                self.database.subtitle_workspace_snapshot(job_id).await?;
            let start_ms = run.scope.start_ms as i64;
            let end_ms = run
                .scope
                .end_ms
                .or_else(|| crate::application::acoustic_plan::wav_duration(&job.audio_wav).ok())
                .map(|v| i64::try_from(v).context("candidate input duration is too large"))
                .transpose()?
                .unwrap_or_else(|| {
                    all.iter()
                        .map(|s| s.end_ms)
                        .chain(candidates.iter().map(|s| s.end_ms as i64))
                        .max()
                        .unwrap_or(start_ms)
                });
            let units: Vec<TimedUnit> = fs::read(&artifacts.timed_units_json)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            let mut timing_granularities = Vec::new();
            for unit in units {
                let granularity = match unit.kind {
                    crate::application::TimedUnitKind::ProviderSegment => {
                        crate::application::AsrTimingGranularity::ProviderSegment
                    }
                    crate::application::TimedUnitKind::Token => {
                        crate::application::AsrTimingGranularity::Token
                    }
                    crate::application::TimedUnitKind::Word => {
                        crate::application::AsrTimingGranularity::Word
                    }
                };
                if !timing_granularities.contains(&granularity) {
                    timing_granularities.push(granularity);
                }
            }
            let original_segments = all
                .into_iter()
                .filter(|s| s.start_ms < end_ms && s.end_ms > start_ms)
                .collect();
            let preview = LocalRetranscriptionPreview {
                workspace_revision: Some(workspace_revision),
                preview_id: preview_id.into(),
                comparison_id: None,
                job_id: job_id.into(),
                start_ms,
                end_ms,
                original_segments,
                candidate_segments: candidates,
                context_mode: "provider-managed".into(),
                provider_id: run.provider_id,
                provider_name: run.provider_name,
                model_identity: run.model_identity,
                timing_granularities,
                configuration: run.config_snapshot,
            };
            crate::application::acoustic_plan::write_json(
                &artifacts.dir.join("preview.json"),
                &preview,
            )?;
            preview
        };
        if preview.job_id != job_id || preview.preview_id != preview_id {
            return Err(anyhow!("candidate identity mismatch"));
        }
        Ok(preview)
    }

    pub async fn refresh_comparison(
        &self,
        job_id: &str,
        run_id: &str,
    ) -> Result<LocalRetranscriptionPreview> {
        let _guard = self.artifact_lock.lock().await;
        let mut preview = self.read_preview(job_id, run_id).await?;
        let (revision, segments) = self.database.subtitle_workspace_snapshot(job_id).await?;
        preview.workspace_revision = Some(revision);
        preview.original_segments = segments
            .into_iter()
            .filter(|s| s.start_ms < preview.end_ms && s.end_ms > preview.start_ms)
            .collect();
        let comparison = uuid::Uuid::new_v4().to_string();
        preview.comparison_id = Some(comparison.clone());
        let record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        let job = Job::open(record.storage_dir.into())?;
        let artifacts = AsrRunArtifacts::open(&job, run_id)?;
        crate::application::acoustic_plan::write_json(
            &artifacts
                .dir
                .join("comparisons")
                .join(format!("{comparison}.json")),
            &preview,
        )?;
        Ok(preview)
    }

    /// Keep audit metadata; release only an unused candidate's owned payloads.
    pub async fn release_candidate(&self, job_id: &str, run_id: &str) -> Result<u64> {
        let _recognition = self
            .recognition_lock
            .try_lock()
            .map_err(|_| anyhow!("候选正在处理，请稍后清理"))?;
        let _guard = self.artifact_lock.lock().await;
        let record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        let job = Job::open(record.storage_dir.into())?;
        let runs = self.database.list_asr_runs(job_id).await?;
        if runs
            .iter()
            .any(|r| matches!(r.status.as_str(), "running" | "queued"))
        {
            return Err(anyhow!("当前任务仍有候选正在运行"));
        }
        let run = runs
            .iter()
            .find(|r| r.id == run_id)
            .ok_or_else(|| anyhow!("candidate not found in this task"))?;
        let artifacts = AsrRunArtifacts::open(&job, &run.id)?;
        if !artifacts.dir.join("candidate.json").exists()
            && !artifacts.dir.join("progress.json").exists()
            && !artifacts.dir.join("released.json").exists()
        {
            return Err(anyhow!("原始识别结果不能作为未采用候选清理"));
        }
        if self.database.asr_run_is_protected(run_id).await?
            || artifacts.dir.join("adopted.json").exists()
        {
            return Err(anyhow!("该候选已被采用，需保留来源证据"));
        }
        let allowed = job.asr_runs_dir.canonicalize()?;
        let directory = artifacts.dir.canonicalize()?;
        if directory.parent() != Some(allowed.as_path())
            || fs::symlink_metadata(&artifacts.dir)?
                .file_type()
                .is_symlink()
        {
            return Err(anyhow!("candidate artifact path is not owned by this task"));
        }
        // A durable tombstone blocks adoption before any payload is removed. Retrying is safe.
        crate::application::acoustic_plan::write_json(&artifacts.dir.join("released.json"), &true)?;
        let mut freed = 0;
        if let Ok(bytes) = fs::read(artifacts.dir.join("checkpoint-root.json")) {
            let root: PathBuf = serde_json::from_slice(&bytes)?;
            if root.exists() {
                let root = root.canonicalize()?;
                let cache_root = job.dir.join("candidate-checkpoints").canonicalize()?;
                if root.parent() != Some(cache_root.as_path()) {
                    return Err(anyhow!("checkpoint path is outside this task"));
                }
                let shared = runs.iter().filter(|r| r.id != run_id).any(|other| {
                    let Ok(a) = AsrRunArtifacts::open(&job, &other.id) else {
                        return true;
                    };
                    if a.dir.join("released.json").exists() {
                        return false;
                    }
                    fs::read(a.dir.join("checkpoint-root.json"))
                        .ok()
                        .and_then(|v| serde_json::from_slice::<PathBuf>(&v).ok())
                        .and_then(|p| p.canonicalize().ok())
                        .is_some_and(|p| p == root)
                });
                if !shared {
                    freed += remove_owned_payload(&root)?;
                }
            }
        }
        for entry in fs::read_dir(&artifacts.dir)? {
            let entry = entry?;
            if matches!(
                entry.file_name().to_str(),
                Some("run.json" | "released.json" | "checkpoint-root.json")
            ) {
                continue;
            }
            freed += remove_owned_payload(&entry.path())?;
        }
        Ok(freed)
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
        self.confirm_comparison(job_id, preview_id, None).await
    }
    pub async fn confirm_comparison(
        &self,
        job_id: &str,
        preview_id: &str,
        comparison_id: Option<&str>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let _guard = self.artifact_lock.lock().await;
        let (preview, revision, artifacts, cue_set) = self
            .prepare_adoption(job_id, preview_id, comparison_id)
            .await?;
        let updated = self
            .database
            .replace_segment_range_with_revision(
                job_id,
                preview.start_ms,
                preview.end_ms,
                &preview.original_segments,
                &preview.candidate_segments,
                Some((preview_id, &cue_set)),
                Some(revision),
            )
            .await?;
        self.finish_adoption_marks(preview_id, &artifacts).await?;
        Ok(updated)
    }

    pub async fn confirm_comparison_recorded(
        &self,
        job_id: &str,
        preview_id: &str,
        comparison_id: Option<&str>,
    ) -> Result<super::structure_edit_session::StructureEditResult> {
        let session = self
            .structure_edits
            .as_ref()
            .ok_or_else(|| anyhow!("shared structure edit session is not configured"))?;
        // Lock order: artifact gate -> edit session -> SQLite. Undo never takes artifact gate.
        let _guard = self.artifact_lock.lock().await;
        let (preview, revision, artifacts, cue_set) = self
            .prepare_adoption(job_id, preview_id, comparison_id)
            .await?;
        let mut updated = session
            .adopt(
                job_id,
                preview.start_ms,
                preview.end_ms,
                &preview.original_segments,
                &preview.candidate_segments,
                preview_id,
                &cue_set,
                revision,
            )
            .await?;
        // The adoption ledger has already committed with the subtitles and receipt.
        // Losing a supplementary marker must not hide the committed result/token.
        if let Err(error) = self.finish_adoption_marks(preview_id, &artifacts).await {
            updated
                .warnings
                .push(format!("字幕已采用，但候选状态标记未完成：{error}"));
        }
        Ok(updated)
    }

    async fn prepare_adoption(
        &self,
        job_id: &str,
        preview_id: &str,
        comparison_id: Option<&str>,
    ) -> Result<(
        LocalRetranscriptionPreview,
        i64,
        AsrRunArtifacts,
        CandidateCueSet,
    )> {
        let preview = if let Some(comparison) = comparison_id {
            uuid::Uuid::parse_str(comparison).context("invalid comparison id")?;
            let record = self
                .database
                .get_job(job_id)
                .await?
                .ok_or_else(|| anyhow!("task not found"))?;
            let job = Job::open(record.storage_dir.into())?;
            let artifacts = AsrRunArtifacts::open(&job, preview_id)?;
            if artifacts.dir.join("released.json").exists() {
                return Err(anyhow!("候选已清理"));
            }
            let preview: LocalRetranscriptionPreview = serde_json::from_slice(&fs::read(
                artifacts
                    .dir
                    .join("comparisons")
                    .join(format!("{comparison}.json")),
            )?)?;
            if preview.preview_id != preview_id
                || preview.comparison_id.as_deref() != Some(comparison)
            {
                return Err(anyhow!("comparison identity mismatch"));
            }
            preview
        } else {
            self.read_preview(job_id, preview_id).await?
        };
        if preview.job_id != job_id {
            return Err(anyhow!(
                "retranscription preview belongs to a different task"
            ));
        }
        let revision = preview
            .workspace_revision
            .ok_or_else(|| anyhow!("旧候选缺少工作区版本；请先以当前字幕重新对照，再明确采用"))?;
        let job_record = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let job = Job::open(PathBuf::from(job_record.storage_dir))?;
        let artifacts = AsrRunArtifacts::open(&job, preview_id)?;
        let cue_set = artifacts.read_candidate_cues()?;
        Ok((preview, revision, artifacts, cue_set))
    }

    async fn finish_adoption_marks(
        &self,
        preview_id: &str,
        artifacts: &AsrRunArtifacts,
    ) -> Result<()> {
        self.database
            .mark_asr_repair_attempts_for_run(preview_id, RepairAttemptStatus::Adopted)
            .await?;
        crate::application::acoustic_plan::write_json(&artifacts.dir.join("adopted.json"), &true)?;
        Ok(())
    }
}

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn remove_owned_payload(path: &std::path::Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        let mut bytes = 0;
        for entry in fs::read_dir(path)? {
            bytes += remove_owned_payload(&entry?.path())?;
        }
        fs::remove_dir(path)?;
        Ok(bytes)
    } else {
        fs::remove_file(path)?;
        Ok(metadata.len())
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
    async fn recorded_adoption_shares_workspace_history_and_keeps_token_after_marker_failure() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("atogaki-recorded-service-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let ffmpeg = root.join("ffmpeg");
        fs::write(
            &ffmpeg,
            "#!/bin/sh\nfor destination do :; done\n: > \"$destination\"\n",
        )
        .unwrap();
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
        let original = TranscriptSegment::new(0, 1000, "元の文".into());
        job.write_segments(&[original]).unwrap();
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
        let workspace = LocalWorkspaceService::new(db.clone());
        let session = workspace.structure_edits();
        let service = LocalRetranscriptionService::new(ffmpeg, "unused".into(), db.clone())
            .with_asr_provider(Arc::new(Fixture));
        let preview = service.preview(&job.id(), 0, 1000).await.unwrap();
        let before = db.list_segments(&job.id()).await.unwrap();
        assert!(
            service
                .confirm_comparison_recorded(&job.id(), &preview.preview_id, None)
                .await
                .unwrap_err()
                .to_string()
                .contains("not configured")
        );
        assert_eq!(db.list_segments(&job.id()).await.unwrap(), before);
        assert_eq!(session.stats().await.entries, 0);
        let service = service.with_structure_edits(session.clone());
        let artifacts = AsrRunArtifacts::open(&job, &preview.preview_id).unwrap();
        // A directory blocks the supplemental JSON write after the SQLite commit.
        fs::create_dir(artifacts.dir.join("adopted.json")).unwrap();
        let result = service
            .confirm_comparison_recorded(&job.id(), &preview.preview_id, None)
            .await
            .unwrap();
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("字幕已采用"));
        assert_eq!(result.segments[0].source_text, "候補");
        assert_eq!(session.latest(&job.id()).await, result.undo.latest);
        assert!(db.asr_run_is_protected(&preview.preview_id).await.unwrap());
        let committed = db.subtitle_workspace_snapshot(&job.id()).await.unwrap();
        assert!(
            service
                .confirm_comparison_recorded(&job.id(), &preview.preview_id, None)
                .await
                .is_err()
        );
        assert_eq!(
            db.subtitle_workspace_snapshot(&job.id()).await.unwrap(),
            committed
        );
        assert_eq!(session.stats().await.entries, 1);
        let undo = workspace
            .structure_edits()
            .undo(&job.id(), &result.undo.latest.unwrap().token)
            .await
            .unwrap();
        assert_eq!(undo.segments, before);
        assert!(db.asr_run_is_protected(&preview.preview_id).await.unwrap());
        assert!(
            service
                .release_candidate(&job.id(), &preview.preview_id)
                .await
                .is_err()
        );
        assert_eq!(session.stats().await.retained_bytes, 0);
        db.close().await;
        fs::remove_dir_all(root).unwrap();
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
        let count = db.list_asr_runs(&job.id()).await.unwrap().len();
        let new = reopened
            .refresh_comparison(&job.id(), &preview.preview_id)
            .await
            .unwrap();
        assert_eq!(db.list_asr_runs(&job.id()).await.unwrap().len(), count);
        let workspace = LocalWorkspaceService::new(db.clone());
        workspace
            .update_subtitle(&job.id(), &original.id, "另一版".into(), None, 0, 1000)
            .await
            .unwrap();
        workspace
            .update_subtitle(&job.id(), &original.id, "人工修正".into(), None, 0, 1000)
            .await
            .unwrap();
        assert_eq!(
            db.list_segments(&job.id()).await.unwrap(),
            new.original_segments
        );
        assert!(
            reopened
                .confirm_comparison(&job.id(), &new.preview_id, new.comparison_id.as_deref())
                .await
                .unwrap_err()
                .to_string()
                .contains("revision changed")
        );
        let new = reopened
            .refresh_comparison(&job.id(), &preview.preview_id)
            .await
            .unwrap();
        assert_eq!(
            reopened
                .confirm_comparison(&job.id(), &new.preview_id, new.comparison_id.as_deref())
                .await
                .unwrap()[0]
                .source_text,
            "候補"
        );
        assert!(
            reopened
                .confirm_comparison(&job.id(), &new.preview_id, new.comparison_id.as_deref())
                .await
                .is_err()
        );
        // The durable ledger protects adopted evidence even if its optional marker is lost.
        let adopted_artifacts = AsrRunArtifacts::open(&job, &preview.preview_id).unwrap();
        fs::remove_file(adopted_artifacts.dir.join("adopted.json")).unwrap();
        assert!(db.asr_run_is_protected(&preview.preview_id).await.unwrap());
        assert!(
            reopened
                .release_candidate(&job.id(), &preview.preview_id)
                .await
                .is_err()
        );
        let before_cleanup = db.list_segments(&job.id()).await.unwrap();
        let unused = reopened.preview(&job.id(), 0, 1000).await.unwrap();
        let guard = reopened.artifact_lock.lock().await;
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                reopened.release_candidate(&job.id(), &unused.preview_id)
            )
            .await
            .is_err()
        );
        drop(guard);
        let comparison = reopened
            .refresh_comparison(&job.id(), &unused.preview_id)
            .await
            .unwrap();
        let unused2 = reopened.preview(&job.id(), 0, 1000).await.unwrap();
        let a = AsrRunArtifacts::open(&job, &unused.preview_id).unwrap();
        let b = AsrRunArtifacts::open(&job, &unused2.preview_id).unwrap();
        let cache = job.dir.join("candidate-checkpoints").join("shared-fixture");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("audio.wav"), vec![0; 1024]).unwrap();
        for artifact in [&a, &b] {
            crate::application::acoustic_plan::write_json(
                &artifact.dir.join("checkpoint-root.json"),
                &cache,
            )
            .unwrap();
        }
        assert!(
            reopened
                .release_candidate(&job.id(), &unused.preview_id)
                .await
                .unwrap()
                > 0
        );
        assert!(
            cache.exists(),
            "another candidate still owns this shared cache"
        );
        assert!(a.run_json.exists() && a.dir.join("released.json").exists());
        assert!(
            reopened
                .reopen(&job.id(), &unused.preview_id)
                .await
                .is_err()
        );
        assert!(
            reopened
                .confirm_comparison(
                    &job.id(),
                    &unused.preview_id,
                    comparison.comparison_id.as_deref()
                )
                .await
                .is_err()
        );
        assert_eq!(
            reopened
                .release_candidate(&job.id(), &unused.preview_id)
                .await
                .unwrap(),
            0
        );
        assert!(
            reopened
                .release_candidate(&job.id(), &unused2.preview_id)
                .await
                .unwrap()
                >= 1024
        );
        assert!(!cache.exists());
        assert_eq!(db.list_segments(&job.id()).await.unwrap(), before_cleanup);
        let outside = root.join("outside-evidence");
        fs::write(&outside, b"keep").unwrap();
        let unsafe_candidate = reopened.preview(&job.id(), 0, 1000).await.unwrap();
        let unsafe_artifact = AsrRunArtifacts::open(&job, &unsafe_candidate.preview_id).unwrap();
        crate::application::acoustic_plan::write_json(
            &unsafe_artifact.dir.join("checkpoint-root.json"),
            &outside,
        )
        .unwrap();
        assert!(
            reopened
                .release_candidate(&job.id(), &unsafe_candidate.preview_id)
                .await
                .is_err()
        );
        assert_eq!(fs::read(&outside).unwrap(), b"keep");
        // Full-run scope includes trailing silence, not merely the last cue.
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&64036_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&16000_u32.to_le_bytes());
        wav.extend_from_slice(&32000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&64000_u32.to_le_bytes());
        wav.resize(64044, 0);
        fs::write(&job.audio_wav, wav).unwrap();
        let original_artifacts = AsrRunArtifacts::open(&job, &preview.preview_id).unwrap();
        let mut full = original_artifacts.read_run().unwrap();
        full.id = uuid::Uuid::new_v4().to_string();
        full.scope = AsrInputScope::full();
        let full_artifacts = AsrRunArtifacts::create(&job, &full).unwrap();
        full_artifacts
            .write_candidate_cues(&original_artifacts.read_candidate_cues().unwrap())
            .unwrap();
        fs::copy(
            &original_artifacts.timed_units_json,
            &full_artifacts.timed_units_json,
        )
        .unwrap();
        let full_preview = reopened.reopen(&job.id(), &full.id).await.unwrap();
        assert!(
            reopened
                .release_candidate(&job.id(), &full.id)
                .await
                .is_err(),
            "reopening original ASR must not make it disposable"
        );
        assert_eq!(full_preview.end_ms, 2000);
        assert_eq!(
            full_preview.timing_granularities,
            vec![crate::application::AsrTimingGranularity::ProviderSegment]
        );
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&fs::read(full_artifacts.dir.join("preview.json")).unwrap())
                .unwrap();
        legacy.as_object_mut().unwrap().remove("workspace_revision");
        crate::application::acoustic_plan::write_json(
            &full_artifacts.dir.join("preview.json"),
            &legacy,
        )
        .unwrap();
        assert!(
            reopened
                .confirm(&job.id(), &full.id)
                .await
                .unwrap_err()
                .to_string()
                .contains("旧候选缺少工作区版本")
        );
        assert!(
            reopened
                .refresh_comparison(&job.id(), &full.id)
                .await
                .unwrap()
                .workspace_revision
                .is_some()
        );
        db.close().await;
        fs::remove_dir_all(root).unwrap();
    }
}
