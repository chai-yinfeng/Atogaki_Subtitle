//! Translate a verified acoustic prefix without publishing an editable workspace.
//! The same immutable request cache is read by the normal final translation path.
use crate::{
    application::{
        TranscriptionOptions, TranslationFuture, TranslationGroup, TranslationOptions,
        TranslationPlanner, TranslationPlannerCue, TranslationProvider, TranslationProviderStatus,
        TranslationRequest, TranslationResponse, acoustic_plan, execute_translation_group,
        translation_planner::TRANSLATION_AFTER_CONTEXT_MS,
        translation_use_case::validate_translation_results,
    },
    domain::glossary,
    infrastructure::{job_store::Job, local_db::LocalDatabase},
};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, path::PathBuf, sync::Arc};
use tokio::sync::watch;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EarlyTranslationProgress {
    pub translated_cues: usize,
    pub stable_through_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CachedResponse {
    schema_version: u32,
    input_sha256: String,
    response: TranslationResponse,
    request: TranslationRequest,
    sha256: String,
}

#[derive(Debug, Clone)]
struct CachedProvider {
    provider: Arc<dyn TranslationProvider>,
    directory: PathBuf,
    profile_sha256: String,
    profile_snapshot: serde_json::Value,
    // None while processing immutable acoustic evidence. Final workspace cache
    // hits require original source and translation revisions, including context.
    guard: Option<(LocalDatabase, String)>,
}

fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

impl CachedProvider {
    fn key(&self, request: &TranslationRequest) -> Result<String> {
        digest(&serde_json::json!({
            "policy": "stable-prefix-cache-v1", "profile": self.profile_sha256,
            "options": request.options, "before": request.before_context,
            "targets": request.targets, "after": request.after_context,
            "style": request.style_instruction,
        }))
    }
    fn read(&self, key: &str) -> Result<TranslationResponse> {
        let cached: CachedResponse =
            serde_json::from_slice(&fs::read(self.directory.join(format!("{key}.json")))?)?;
        if cached.schema_version != 2
            || cached.input_sha256 != key
            || self.key(&cached.request)? != key
            || cached.sha256 != digest(&(key, &cached.response))?
        {
            bail!("early translation cache identity or checksum mismatch")
        }
        Ok(cached.response)
    }
    async fn revisions_match(&self, request: &TranslationRequest) -> Result<bool> {
        let Some((database, job_id)) = &self.guard else {
            return Ok(true);
        };
        let provenance = database.list_subtitle_provenance(job_id).await?;
        for id in request
            .targets
            .iter()
            .map(|t| &t.segment_id)
            .chain(request.before_context.iter().map(|t| &t.segment_id))
            .chain(request.after_context.iter().map(|t| &t.segment_id))
        {
            let Some(p) = provenance.iter().find(|p| &p.segment_id == id) else {
                return Ok(false);
            };
            if p.source_revision != 0
                || (request.targets.iter().any(|t| &t.segment_id == id)
                    && p.translation_revision != 0)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
impl TranslationProvider for CachedProvider {
    fn status(&self) -> TranslationProviderStatus {
        self.provider.status()
    }
    fn execution_snapshot(&self) -> Option<Arc<dyn TranslationProvider>> {
        Some(Arc::new(self.clone()))
    }
    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async move {
            let key = self.key(&request)?;
            let ids = request
                .targets
                .iter()
                .map(|t| t.segment_id.clone())
                .collect::<Vec<_>>();
            if self.revisions_match(&request).await? {
                if let Ok(response) = self.read(&key) {
                    if validate_translation_results(
                        &self.status().name,
                        &ids,
                        &response.translations,
                    )
                    .is_ok()
                        && response
                            .translations
                            .iter()
                            .all(|t| !t.translated_text.trim().is_empty())
                    {
                        return Ok(response);
                    }
                }
            }
            let outcome = self.provider.translate(request.clone()).await;
            // Actual attempts remain immutable even when a cache is replaced or
            // a later source/context version makes this response unusable.
            let attempt = serde_json::json!({
                "schema_version": 1, "profile_sha256": self.profile_sha256,
                "execution": self.profile_snapshot,
                "input_sha256": key, "request": request,
                "provider_id": self.status().id,
                "response": outcome.as_ref().ok(),
                "error": outcome.as_ref().err().map(|e| format!("{e:#}")),
            });
            acoustic_plan::write_json(
                &self
                    .directory
                    .parent()
                    .expect("cache directory has parent")
                    .join("attempts")
                    .join(format!("{}.json", uuid::Uuid::new_v4())),
                &attempt,
            )?;
            let response = outcome?;
            validate_translation_results(&self.status().name, &ids, &response.translations)?;
            // Empty fragments still use the existing isolated retry policy. An
            // empty or structurally invalid response must never become a hit.
            if response
                .translations
                .iter()
                .all(|t| !t.translated_text.trim().is_empty())
            {
                let cached = CachedResponse {
                    schema_version: 2,
                    input_sha256: key.clone(),
                    sha256: digest(&(&key, &response))?,
                    response: response.clone(),
                    request,
                };
                acoustic_plan::write_json(&self.directory.join(format!("{key}.json")), &cached)?;
            }
            Ok(response)
        })
    }
}

#[derive(Debug, Clone)]
pub struct EarlyTranslationSession {
    job: Job,
    transcription: TranscriptionOptions,
    options: TranslationOptions,
    cache: CachedProvider,
}
impl EarlyTranslationSession {
    pub fn new(job: Job, provider: Arc<dyn TranslationProvider>) -> Result<Option<Self>> {
        let provider = provider.execution_snapshot().unwrap_or(provider);
        let status = provider.status();
        if !status.configured || status.id == "hy-mt2-local" {
            return Ok(None);
        }
        let transcription = job.read_recognition_options()?;
        let protected_terms = transcription
            .glossary
            .as_deref()
            .map(glossary::load)
            .transpose()?
            .map(|g| g.translation_protected_terms())
            .unwrap_or_default();
        let manifest = job
            .read_manifest_if_exists()?
            .ok_or_else(|| anyhow!("task manifest missing"))?;
        let options =
            TranslationOptions::new(transcription.source_language, manifest.target_language)
                .with_protected_terms(protected_terms);
        let profile = if job.dir.join("task-preferences.json").is_file() {
            serde_json::from_slice::<serde_json::Value>(&fs::read(
                job.dir.join("task-preferences.json"),
            )?)?
        } else {
            serde_json::Value::Null
        };
        let profile_snapshot = serde_json::json!({"provider": status, "profile": profile});
        let cache = CachedProvider {
            provider,
            directory: job.dir.join("early-translation/groups"),
            profile_sha256: digest(&profile_snapshot)?,
            profile_snapshot,
            guard: None,
        };
        Ok(Some(Self {
            job,
            transcription,
            options,
            cache,
        }))
    }
    pub fn final_provider(&self, database: LocalDatabase) -> Arc<dyn TranslationProvider> {
        let mut cache = self.cache.clone();
        cache.guard = Some((database, self.job.id()));
        Arc::new(cache)
    }
    pub async fn run(
        &self,
        mut progress: watch::Receiver<u64>,
        mut done: watch::Receiver<bool>,
    ) -> Result<()> {
        // A new attempt must not display an old prefix error while retrying.
        if let Ok(mut state) = self.progress() {
            state.error = None;
            acoustic_plan::write_json(
                &self.job.dir.join("early-translation/progress.json"),
                &state,
            )?;
        }
        // Notification is sent only after plan/configuration validation and a
        // verified chunk. Old evidence cannot trigger uploads before that gate.
        loop {
            tokio::select! {
                result = progress.changed() => { if result.is_err() { return Ok(()); } },
                _ = done.changed() => return Ok(()),
            }
            if *done.borrow() {
                return Ok(());
            }
            if let Err(error) = self.process_prefix().await {
                let mut state = self.progress().unwrap_or_default();
                state.error = Some(format!("{error:#}"));
                acoustic_plan::write_json(
                    &self.job.dir.join("early-translation/progress.json"),
                    &state,
                )?;
                return Err(error);
            }
        }
    }
    pub fn progress(&self) -> Result<EarlyTranslationProgress> {
        Ok(serde_json::from_slice(&fs::read(
            self.job.dir.join("early-translation/progress.json"),
        )?)?)
    }
    pub async fn process_prefix(&self) -> Result<()> {
        let Some((units, frontier)) = acoustic_plan::stable_prefix(&self.job)? else {
            return Ok(());
        };
        let cues = glossary::apply_to_segments(
            &self.transcription,
            acoustic_plan::candidate_cues(&units).transcript_segments(),
        )?;
        let timeline = cues
            .iter()
            .enumerate()
            .map(|(i, cue)| TranslationPlannerCue {
                segment_id: cue.id.clone(),
                segment_index: i as i64,
                start_ms: cue.start_ms as i64,
                end_ms: cue.end_ms as i64,
                source_text: cue.source_text.clone(),
            })
            .collect::<Vec<_>>();
        let targets = cues.iter().map(|c| c.id.clone()).collect::<HashSet<_>>();
        let cutoff = frontier.saturating_sub(TRANSLATION_AFTER_CONTEXT_MS);
        let groups = TranslationPlanner::default()
            .plan(&timeline, &targets)?
            .groups
            .into_iter()
            .filter(|g| {
                g.targets.iter().all(|t| {
                    cues.iter()
                        .any(|c| c.id == t.segment_id && c.end_ms <= cutoff)
                })
            })
            .collect::<Vec<_>>();
        let concurrency = if self.cache.status().id == "deepseek" {
            4
        } else {
            1
        };
        let mut translated = groups
            .iter()
            .filter(|g| self.cached_group(g).is_ok())
            .flat_map(|g| g.targets.iter().map(|t| t.segment_id.clone()))
            .collect::<HashSet<_>>();
        self.write_progress(translated.len(), cutoff, None)?;
        let mut pending = groups.iter().cloned();
        let mut tasks = tokio::task::JoinSet::new();
        let mut first_error = None;
        loop {
            while first_error.is_none() && tasks.len() < concurrency {
                let Some(group) = pending.next() else {
                    break;
                };
                let cache = self.cache.clone();
                let options = self.options.clone();
                tasks.spawn(async move {
                    let ids = group
                        .targets
                        .iter()
                        .map(|t| t.segment_id.clone())
                        .collect::<Vec<_>>();
                    let response = execute_translation_group(&cache, options, group, None).await?;
                    if response
                        .translations
                        .iter()
                        .any(|t| t.translated_text.trim().is_empty())
                    {
                        bail!("提前翻译返回空译文；完整识别后可继续翻译")
                    }
                    Ok::<_, anyhow::Error>(ids)
                });
            }
            let Some(result) = tasks.join_next().await else {
                break;
            };
            match result {
                Ok(Ok(ids)) => {
                    translated.extend(ids);
                    self.write_progress(
                        translated.len(),
                        cutoff,
                        first_error.as_ref().map(|e| format!("{e:#}")),
                    )?;
                }
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(anyhow!("early translation worker stopped: {error}"));
                    }
                }
            }
        }
        self.write_progress(
            translated.len(),
            cutoff,
            first_error.as_ref().map(|e| format!("{e:#}")),
        )?;
        if let Some(error) = first_error {
            return Err(error.context("提前翻译停止，已完成结果保留；识别继续执行"));
        }
        Ok(())
    }
    fn write_progress(
        &self,
        translated_cues: usize,
        stable_through_ms: u64,
        error: Option<String>,
    ) -> Result<()> {
        acoustic_plan::write_json(
            &self.job.dir.join("early-translation/progress.json"),
            &EarlyTranslationProgress {
                translated_cues,
                stable_through_ms,
                error,
            },
        )
    }
    fn cached_group(&self, group: &TranslationGroup) -> Result<TranslationResponse> {
        let request = TranslationRequest {
            options: self.options.clone(),
            before_context: group.before_context.clone(),
            targets: group.targets.clone(),
            after_context: group.after_context.clone(),
            style_instruction: None,
        };
        self.cache.read(&self.cache.key(&request)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        AsrDataLocality, AsrFuture, AsrProvider, AsrProviderCapabilities, AsrProviderStatus,
        AsrRequest, AsrResponse, AsrTimingGranularity, JobSnapshot, LocalWorkspaceService,
        TimedUnit, TimedUnitKind, TimingSource, TranslationResult, TranslationUsage,
        job_manifest::JobManifest, job_status::JobStatus,
    };
    use crate::domain::{LanguageCode, LanguagePair};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    #[derive(Debug)]
    struct Asr {
        calls: AtomicUsize,
        fail: AtomicBool,
    }
    impl AsrProvider for Asr {
        fn status(&self) -> AsrProviderStatus {
            AsrProviderStatus {
                id: "fixture".into(),
                name: "Fixture".into(),
                capabilities: AsrProviderCapabilities {
                    timing_granularities: vec![AsrTimingGranularity::ProviderSegment],
                    vocabulary_biasing: false,
                    diarization: false,
                    streaming: false,
                    data_locality: AsrDataLocality::Local,
                },
            }
        }
        fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if request.scope.start_ms >= 235_000 && self.fail.load(Ordering::SeqCst) {
                    bail!("injected ASR interruption")
                }
                let raw = request.output_prefix.with_extension("json");
                fs::write(&raw, b"{}")?;
                Ok(AsrResponse {
                    provider_id: "fixture".into(),
                    model_identity: "fixture".into(),
                    raw_output_path: raw,
                    legacy_segments: vec![],
                    timed_units: [6_000, 40_000, 80_000, 110_000]
                        .into_iter()
                        .map(|start| TimedUnit {
                            id: uuid::Uuid::new_v4().to_string(),
                            text: format!("Sentence {} {start}.", request.scope.start_ms),
                            kind: TimedUnitKind::ProviderSegment,
                            start_ms: Some(start),
                            end_ms: Some(start + 1000),
                            timing_source: TimingSource::Model,
                            provider_confidence: None,
                            speaker: None,
                        })
                        .collect(),
                })
            })
        }
    }
    #[derive(Debug, Default)]
    struct Provider {
        requests: Mutex<Vec<TranslationRequest>>,
        fail: AtomicBool,
    }
    impl TranslationProvider for Provider {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: "deepseek".into(),
                name: "Fixture".into(),
                configured: true,
                model: Some("fixture-v1".into()),
                endpoint_kind: "test".into(),
                configuration_hint: None,
            }
        }
        fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
            Box::pin(async move {
                self.requests.lock().unwrap().push(request.clone());
                if self.fail.load(Ordering::SeqCst) {
                    bail!("injected network failure")
                }
                Ok(TranslationResponse {
                    translations: request
                        .targets
                        .iter()
                        .map(|t| TranslationResult {
                            segment_id: t.segment_id.clone(),
                            translated_text: format!("译：{}", t.source_text),
                        })
                        .collect(),
                    model: Some("fixture-v1".into()),
                    usage: TranslationUsage::default(),
                })
            })
        }
    }
    #[cfg(unix)]
    async fn fixture() -> (Job, PathBuf, Arc<Asr>, TranscriptionOptions) {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("atogaki-early-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let model = root.join("model");
        fs::write(&model, b"model").unwrap();
        let options = TranscriptionOptions::new(model, LanguageCode::English);
        job.write_recognition_options(&options).unwrap();
        let mut manifest = JobManifest::new(
            &job,
            None,
            None,
            LanguagePair::english_to_simplified_chinese(),
        );
        manifest.mark(JobStatus::Transcribing);
        job.write_manifest(&manifest).unwrap();
        fs::write(&job.audio_wav, b"fixture").unwrap();
        acoustic_plan::write_json(
            &job.dir.join("audio-identity.json"),
            &serde_json::json!({"sha256":"audio","duration_ms":360000}),
        )
        .unwrap();
        let ffmpeg = root.join("ffmpeg");
        fs::write(
            &ffmpeg,
            "#!/bin/sh\nfor destination do :; done\n: > \"$destination\"\n",
        )
        .unwrap();
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
        let asr = Arc::new(Asr {
            calls: AtomicUsize::new(0),
            fail: AtomicBool::new(true),
        });
        assert!(
            acoustic_plan::execute_chunks(&job, &ffmpeg, asr.clone(), &options, "first", None)
                .await
                .is_err()
        );
        (job, ffmpeg, asr, options)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prefix_progress_publishes_each_group_before_the_batch_finishes() {
        #[derive(Debug)]
        struct Gated {
            calls: AtomicUsize,
            release: tokio::sync::Semaphore,
        }
        impl TranslationProvider for Gated {
            fn status(&self) -> TranslationProviderStatus {
                Provider::default().status()
            }
            fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
                Box::pin(async move {
                    if self.calls.fetch_add(1, Ordering::SeqCst) > 0 {
                        self.release.acquire().await.unwrap().forget();
                    }
                    Provider::default().translate(request).await
                })
            }
        }
        let (job, _, _, _) = fixture().await;
        let provider = Arc::new(Gated {
            calls: AtomicUsize::new(0),
            release: tokio::sync::Semaphore::new(0),
        });
        let session = EarlyTranslationSession::new(job.clone(), provider.clone())
            .unwrap()
            .unwrap();
        session
            .write_progress(0, 0, Some("old failure".into()))
            .unwrap();
        let worker = {
            let session = session.clone();
            tokio::spawn(async move { session.process_prefix().await })
        };
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let state = session.progress().unwrap();
                if state.translated_cues == 1 {
                    assert!(
                        state.error.is_none(),
                        "old failure cleared while new request runs"
                    );
                    assert!(!worker.is_finished(), "other groups remain in flight");
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        provider.release.add_permits(8);
        worker.await.unwrap().unwrap();
        assert_eq!(session.progress().unwrap().translated_cues, 3);
        fs::remove_dir_all(job.dir.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prefix_survives_restart_and_corruption_then_reuses_exact_final_inputs() {
        let (job, ffmpeg, asr, options) = fixture().await;
        let provider = Arc::new(Provider::default());
        let session = EarlyTranslationSession::new(job.clone(), provider.clone())
            .unwrap()
            .unwrap();
        session.process_prefix().await.unwrap();
        assert_eq!(session.progress().unwrap().translated_cues, 3);
        assert_eq!(provider.requests.lock().unwrap().len(), 3);
        assert!(
            !job.segments_json.exists(),
            "prefix never publishes workspace"
        );
        let session = EarlyTranslationSession::new(job.clone(), provider.clone())
            .unwrap()
            .unwrap();
        session.process_prefix().await.unwrap();
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            3,
            "restart uses committed cache"
        );
        let cache_file = fs::read_dir(&session.cache.directory)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(cache_file, b"corrupt").unwrap();
        session.process_prefix().await.unwrap();
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            4,
            "only corrupt group repeated"
        );
        asr.fail.store(false, Ordering::SeqCst);
        acoustic_plan::execute_chunks(&job, &ffmpeg, asr.clone(), &options, "resume", None)
            .await
            .unwrap();
        assert_eq!(
            asr.calls.load(Ordering::SeqCst),
            4,
            "two complete chunks were reused"
        );
        let cues = acoustic_plan::candidate_cues(&acoustic_plan::partial_units(&job).unwrap())
            .transcript_segments();
        let mut manifest = job.read_manifest_if_exists().unwrap().unwrap();
        manifest.mark(JobStatus::Done);
        let db = LocalDatabase::open(job.dir.join("index.sqlite"))
            .await
            .unwrap();
        db.sync_snapshot(&JobSnapshot {
            manifest,
            segments: cues,
        })
        .await
        .unwrap();
        let workspace =
            LocalWorkspaceService::with_provider(db.clone(), session.final_provider(db.clone()));
        let result = workspace.translate_pending(&job.id()).await.unwrap();
        assert_eq!(result.translated_count, 12);
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            13,
            "three matching cached groups skip network"
        );
        assert!(result.segments.iter().all(|s| s.translated_text.is_some()));
        db.close().await;
        fs::remove_dir_all(job.dir.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_translation_keeps_asr_and_profile_context_revision_changes_invalidate_cache() {
        let (job, ffmpeg, asr, options) = fixture().await;
        let provider = Arc::new(Provider::default());
        provider.fail.store(true, Ordering::SeqCst);
        let session = EarlyTranslationSession::new(job.clone(), provider.clone())
            .unwrap()
            .unwrap();
        assert!(session.process_prefix().await.is_err());
        assert!(session.progress().unwrap().error.is_some());
        asr.fail.store(false, Ordering::SeqCst);
        acoustic_plan::execute_chunks(&job, &ffmpeg, asr, &options, "resume", None)
            .await
            .unwrap();
        provider.fail.store(false, Ordering::SeqCst);
        session.process_prefix().await.unwrap();
        let group = provider.requests.lock().unwrap().last().unwrap().clone();
        let previous = provider.requests.lock().unwrap().len();
        session.cache.translate(group.clone()).await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), previous);
        let mut changed = group.clone();
        changed
            .after_context
            .push(crate::application::TranslationContextSegment {
                segment_id: "changed".into(),
                source_text: "new context".into(),
            });
        session.cache.translate(changed).await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), previous + 1);
        acoustic_plan::write_json(
            &job.dir.join("task-preferences.json"),
            &serde_json::json!({"style":"changed"}),
        )
        .unwrap();
        let new_session = EarlyTranslationSession::new(job.clone(), provider.clone())
            .unwrap()
            .unwrap();
        new_session.cache.translate(group.clone()).await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), previous + 2);
        let cues = acoustic_plan::candidate_cues(&acoustic_plan::partial_units(&job).unwrap())
            .transcript_segments();
        let db = LocalDatabase::open(job.dir.join("index.sqlite"))
            .await
            .unwrap();
        db.sync_snapshot(&JobSnapshot {
            manifest: job.read_manifest_if_exists().unwrap().unwrap(),
            segments: cues,
        })
        .await
        .unwrap();
        let target = db
            .list_segments(&job.id())
            .await
            .unwrap()
            .into_iter()
            .find(|s| s.id == group.targets[0].segment_id)
            .unwrap();
        db.update_segment(
            &job.id(),
            &target.id,
            "changed then restored".into(),
            None,
            target.start_ms,
            target.end_ms,
        )
        .await
        .unwrap();
        db.update_segment(
            &job.id(),
            &target.id,
            target.source_text.clone(),
            None,
            target.start_ms,
            target.end_ms,
        )
        .await
        .unwrap();
        assert!(
            db.apply_machine_translations(
                &job.id(),
                &[crate::infrastructure::local_db::LocalMachineTranslation {
                    segment_id: target.id.clone(),
                    source_text: target.source_text.clone(),
                    expected_source_revision: Some(0),
                    expected_translation_revision: Some(0),
                    expected_translated_text: None,
                    expected_translation_edited: false,
                    translated_text: "stale in-flight result".into(),
                }]
            )
            .await
            .is_err(),
            "transaction rejects edits even when text is restored"
        );
        session
            .final_provider(db.clone())
            .translate(group)
            .await
            .unwrap();
        assert_eq!(
            provider.requests.lock().unwrap().len(),
            previous + 3,
            "reverting text does not erase revision history"
        );
        db.close().await;
        fs::remove_dir_all(job.dir.parent().unwrap()).unwrap();
    }
}
