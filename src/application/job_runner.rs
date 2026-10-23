use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, anyhow};

use crate::{
    application::{
        AsrInputScope, AsrProvider, AsrRequest, AsrRun, TranscriptionOptions,
        detect_quality_signals,
        job_manifest::JobManifest,
        job_snapshot::JobSnapshot,
        job_spec::{
            ApplyGlossarySpec, ExportSpec, ProcessSpec, RenderSpec, RerenderSpec, TranscribeSpec,
            TranslateSpec,
        },
        job_status::JobStatus,
        segment_timed_units, translate_transcript,
    },
    domain::{LanguageCode, LanguagePair, glossary, subtitle},
    infrastructure::{
        asr_run_store::AsrRunArtifacts, config::AppConfig, deepl::DeepLTranslationProvider,
        job_store::Job, local_db::LocalDatabase, media, whisper_asr::WhisperAsrProvider,
    },
};

pub struct JobRunner {
    config: AppConfig,
    asr_provider: Arc<dyn AsrProvider>,
    database: Option<LocalDatabase>,
    asr_progress: Option<tokio::sync::watch::Sender<u64>>,
}

impl JobRunner {
    pub fn new(config: AppConfig) -> Self {
        let asr_provider = Arc::new(WhisperAsrProvider::new(config.whisper_cli.clone()));
        Self {
            config,
            asr_provider,
            database: None,
            asr_progress: None,
        }
    }

    pub fn with_database(mut self, database: LocalDatabase) -> Self {
        self.database = Some(database);
        self
    }

    pub fn with_asr_progress(mut self, progress: tokio::sync::watch::Sender<u64>) -> Self {
        self.asr_progress = Some(progress);
        self
    }

    pub fn with_asr_provider(mut self, provider: Arc<dyn AsrProvider>) -> Self {
        self.asr_provider = provider;
        self
    }

    /// Reads the durable state of a task without invoking any media tooling.
    /// This is the query entry point for UI task lists and subtitle editors.
    pub fn snapshot(&self, job_dir: impl AsRef<std::path::Path>) -> Result<JobSnapshot> {
        JobSnapshot::load(job_dir)
    }

    pub async fn transcribe(&self, spec: TranscribeSpec) -> Result<Job> {
        let job = Job::create(spec.output_dir.as_deref())?;
        let languages = LanguagePair::new(
            spec.transcription.source_language,
            LanguageCode::SimplifiedChinese,
        )?;
        let mut manifest =
            self.manifest_for_job(&job, Some(spec.input.clone()), None, languages)?;
        self.mark(&job, &mut manifest, JobStatus::Created)?;

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::ExtractingAudio)?;
            let wav = crate::application::acoustic_plan::prepare_audio(
                &self.config.ffmpeg,
                &spec.input,
                &job,
            )
            .await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let refined = self
                .run_initial_asr(&job, &spec.input, &wav, &spec.transcription)
                .await?;
            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;
            job.write_segments(&refined)?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.source_srt, &refined, subtitle::SubtitleTrack::Source)?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result)
    }

    pub async fn process(&self, spec: ProcessSpec) -> Result<Job> {
        let job = Job::create(spec.output_dir.as_deref())?;
        let languages = LanguagePair::new(
            spec.translation.source_language,
            spec.translation.target_language,
        )?;
        if languages.source != spec.transcription.source_language {
            return Err(anyhow!(
                "transcription language {} does not match translation source {}",
                spec.transcription.source_language,
                languages.source
            ));
        }
        let mut manifest = self.manifest_for_job(
            &job,
            Some(spec.input.clone()),
            spec.render_output.clone(),
            languages,
        )?;
        self.mark(&job, &mut manifest, JobStatus::Created)?;

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::ExtractingAudio)?;
            let wav = crate::application::acoustic_plan::prepare_audio(
                &self.config.ffmpeg,
                &spec.input,
                &job,
            )
            .await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let mut segments = self
                .run_initial_asr(&job, &spec.input, &wav, &spec.transcription)
                .await?;
            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.source_srt, &segments, subtitle::SubtitleTrack::Source)?;

            if let Some(key) = spec.deepl_auth_key.or(self.config.deepl_auth_key.clone()) {
                self.mark(&job, &mut manifest, JobStatus::Translating)?;
                let provider = DeepLTranslationProvider::new(Some(key));
                translate_transcript(&provider, &spec.translation, &mut segments).await?;

                self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
                self.write_translated_outputs(&job, &segments)?;

                if let Some(render_output) = spec.render_output.as_deref() {
                    self.mark(&job, &mut manifest, JobStatus::RenderingVideo)?;
                    media::render_subtitles(
                        &self.config.ffmpeg,
                        &spec.input,
                        &job.bilingual_ass,
                        &job.bilingual_srt,
                        render_output,
                        &spec.render,
                    )
                    .await?;
                }
            } else {
                eprintln!("DeepL key missing; wrote source-language transcript only.");
            }

            job.write_segments(&segments)?;
            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result)
    }

    pub async fn translate(&self, spec: TranslateSpec) -> Result<Job> {
        let job = Job::open(spec.job_dir)?;
        let mut manifest = self.manifest_for_existing_job(&job)?;
        manifest.source_language = spec.translation.source_language;
        manifest.target_language = spec.translation.target_language;
        let result = async {
            let mut segments = job.read_segments()?;
            let key = spec
                .deepl_auth_key
                .or(self.config.deepl_auth_key.clone())
                .ok_or_else(|| {
                    anyhow!("DeepL key missing. Set DEEPL_AUTH_KEY or pass --deepl-auth-key")
                })?;

            self.mark(&job, &mut manifest, JobStatus::Translating)?;
            let provider = DeepLTranslationProvider::new(Some(key));
            translate_transcript(&provider, &spec.translation, &mut segments).await?;
            job.write_segments(&segments)?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            self.write_translated_outputs(&job, &segments)?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result)
    }

    pub fn apply_glossary(&self, spec: ApplyGlossarySpec) -> Result<Job> {
        let job = Job::open(spec.job_dir)?;
        let mut manifest = self.manifest_for_existing_job(&job)?;

        let result = (|| {
            let segments = job.read_segments()?;

            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;
            let (segments, report) = glossary::apply_file_to_segments(
                &spec.glossary,
                segments,
                !spec.keep_translations,
            )?;
            job.write_segments(&segments)?;
            eprintln!(
                "[job] glossary applied: {} segment(s) changed, {} stale translation(s) cleared",
                report.changed_segments, report.cleared_translations
            );

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.source_srt, &segments, subtitle::SubtitleTrack::Source)?;
            self.write_translated_outputs(&job, &segments)?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        })();

        self.finish(job, manifest, result)
    }

    pub fn export(&self, spec: ExportSpec) -> Result<Job> {
        let job = Job::open(spec.job_dir)?;
        let mut manifest = self.manifest_for_existing_job(&job)?;

        let result = (|| {
            let segments = job.read_segments()?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.source_srt, &segments, subtitle::SubtitleTrack::Source)?;
            self.write_translated_outputs(&job, &segments)?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        })();

        self.finish(job, manifest, result)
    }

    pub async fn render(&self, spec: RenderSpec) -> Result<()> {
        let job = Job::open(spec.job_dir)?;
        let mut manifest = self.manifest_for_existing_job(&job)?;
        manifest.input = Some(spec.input.clone());
        manifest.render_output = Some(spec.output.clone());

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::RenderingVideo)?;
            media::render_subtitles(
                &self.config.ffmpeg,
                &spec.input,
                &job.bilingual_ass,
                &job.bilingual_srt,
                &spec.output,
                &spec.render,
            )
            .await?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result).map(|_| ())
    }

    pub async fn rerender(&self, spec: RerenderSpec) -> Result<()> {
        let job = Job::open(spec.job_dir)?;
        let mut manifest = self.manifest_for_existing_job(&job)?;
        let input = spec
            .input
            .or_else(|| manifest.input.clone())
            .ok_or_else(|| {
                anyhow!("missing input. Pass --input or run from a job with status.json input")
            })?;
        let output = spec
            .output
            .or_else(|| manifest.render_output.clone())
            .ok_or_else(|| {
                anyhow!(
                    "missing output. Pass --output or run from a job with status.json render_output"
                )
            })?;

        manifest.input = Some(input.clone());
        manifest.render_output = Some(output.clone());

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::RenderingVideo)?;
            media::render_subtitles(
                &self.config.ffmpeg,
                &input,
                &job.bilingual_ass,
                &job.bilingual_srt,
                &output,
                &spec.render,
            )
            .await?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result).map(|_| ())
    }

    fn write_translated_outputs(
        &self,
        job: &Job,
        segments: &[crate::domain::TranscriptSegment],
    ) -> Result<()> {
        subtitle::write_srt(
            &job.translated_srt,
            segments,
            subtitle::SubtitleTrack::Translation,
        )?;
        subtitle::write_srt(
            &job.bilingual_srt,
            segments,
            subtitle::SubtitleTrack::Bilingual,
        )?;
        subtitle::write_ass(&job.bilingual_ass, segments)?;
        Ok(())
    }

    async fn run_initial_asr(
        &self,
        job: &Job,
        input_media: &Path,
        audio_path: &Path,
        options: &TranscriptionOptions,
    ) -> Result<Vec<crate::domain::TranscriptSegment>> {
        let provider = self.asr_provider.status();
        let config_snapshot = serde_json::to_value(options)
            .context("failed to snapshot recognition configuration")?;
        let parent = if let Some(db) = &self.database {
            db.list_asr_runs(&job.id())
                .await?
                .first()
                .map(|r| r.id.clone())
        } else {
            None
        };
        let mut run = AsrRun::new(
            job.id(),
            parent,
            provider.id,
            provider.name,
            options.whisper.model.display().to_string(),
            input_media.to_path_buf(),
            AsrInputScope::full(),
            config_snapshot,
        );
        let artifacts = AsrRunArtifacts::create(job, &run)?;
        if let Some(database) = &self.database {
            database.record_asr_run(&run, &artifacts.dir).await?;
        }

        run.start();
        artifacts.write_run(&run)?;
        if let Some(database) = &self.database {
            database.update_asr_run(&run).await?;
        }

        let response = if matches!(
            self.asr_provider.status().id.as_str(),
            "whisper.cpp" | "mlx-whisper"
        ) {
            crate::application::acoustic_plan::execute_chunks_with_progress(
                job,
                &self.config.ffmpeg,
                Arc::clone(&self.asr_provider),
                options,
                &run.id,
                self.database.as_ref(),
                self.asr_progress.as_ref(),
            )
            .await
        } else {
            self.asr_provider
                .transcribe(AsrRequest {
                    audio_path: audio_path.to_path_buf(),
                    output_prefix: artifacts.provider_output_prefix.clone(),
                    scope: AsrInputScope::full(),
                    cloud_audio_upload_authorized: false,
                    transcription: options.clone(),
                })
                .await
        };

        let completion: Result<Vec<crate::domain::TranscriptSegment>> = async {
            let response = response?;
            let cache = job.dir.join("asr-checkpoint/final-cues.json");
            let mut cue_set = if matches!(
                self.asr_provider.status().id.as_str(),
                "whisper.cpp" | "mlx-whisper"
            ) {
                let fresh = || -> Result<crate::application::CandidateCueSet> {
                    if self.asr_provider.status().id == "mlx-whisper" {
                        let mut cues =
                            segment_timed_units(&response.timed_units, options.source_language)?;
                        cues.segmentation_policy = format!(
                            "{}+{}",
                            cues.segmentation_policy,
                            crate::application::acoustic_stitch::STITCHING_STRATEGY
                        );
                        Ok(cues)
                    } else {
                        Ok(crate::application::acoustic_plan::candidate_cues(
                            &response.timed_units,
                        ))
                    }
                };
                let expected_policy = if self.asr_provider.status().id == "mlx-whisper" {
                    format!(
                        "{}+{}",
                        crate::application::subtitle_segmentation::LEGACY_SEGMENTATION_POLICY,
                        crate::application::acoustic_stitch::STITCHING_STRATEGY
                    )
                } else {
                    crate::application::acoustic_stitch::STITCHING_STRATEGY.into()
                };
                if cache.is_file()
                    && std::fs::read_to_string(cache.with_extension("sha256"))
                        .ok()
                        .as_deref()
                        == crate::application::acoustic_plan::file_digest(&cache)
                            .ok()
                            .as_deref()
                {
                    let cached: crate::application::CandidateCueSet =
                        serde_json::from_slice(&std::fs::read(&cache)?)?;
                    if cached.segmentation_policy != expected_policy {
                        fresh()?
                    } else {
                        cached
                    }
                } else {
                    fresh()?
                }
            } else {
                segment_timed_units(&response.timed_units, options.source_language)?
            };
            let candidates = glossary::apply_to_segments(options, cue_set.transcript_segments())?;
            cue_set.replace_transcript_segments(candidates.clone())?;
            artifacts.write_timed_units(&response.timed_units)?;
            artifacts.write_candidate_cues(&cue_set)?;
            if matches!(
                self.asr_provider.status().id.as_str(),
                "whisper.cpp" | "mlx-whisper"
            ) {
                crate::application::acoustic_plan::write_json(&cache, &cue_set)?;
                crate::application::acoustic_plan::atomic_write(
                    &cache.with_extension("sha256"),
                    crate::application::acoustic_plan::file_digest(&cache)?.as_bytes(),
                )?;
            }
            let mut signals =
                detect_quality_signals(job.id().as_str(), &run.id, &cue_set, run.created_at_unix);
            signals.extend(crate::application::acoustic_plan::boundary_signals(
                &job.id(),
                &run.id,
                &response.timed_units,
                run.created_at_unix,
            ));
            if matches!(
                self.asr_provider.status().id.as_str(),
                "whisper.cpp" | "mlx-whisper"
            ) {
                let mut seam_signals = crate::application::acoustic_plan::stitching_signals(
                    job,
                    &job.id(),
                    &run.id,
                    run.created_at_unix,
                    0,
                )?;
                for signal in &mut seam_signals {
                    signal.cue_ids = cue_set
                        .cues
                        .iter()
                        .filter(|cue| {
                            cue.timed_unit_ids
                                .iter()
                                .any(|id| signal.cue_ids.contains(id))
                        })
                        .map(|cue| cue.cue.id.clone())
                        .collect();
                }
                signals.extend(seam_signals);
            }
            artifacts.write_quality_signals(&signals)?;
            if let Some(database) = &self.database {
                database
                    .replace_asr_quality_signals(&run.id, &signals)
                    .await?;
            }
            run.succeed();
            artifacts.write_run(&run)?;
            if let Some(database) = &self.database {
                database.update_asr_run(&run).await?;
            }
            Ok(candidates)
        }
        .await;

        match completion {
            Ok(candidates) => Ok(candidates),
            Err(error) => {
                run.fail(format!("{error:#}"));
                let file_result = artifacts.write_run(&run);
                let database_result = if let Some(database) = &self.database {
                    database.update_asr_run(&run).await
                } else {
                    Ok(())
                };
                if let Err(persistence_error) = file_result.and(database_result) {
                    eprintln!(
                        "[job] failed to persist ASR run failure {}: {persistence_error:#}",
                        run.id
                    );
                }
                Err(error)
            }
        }
    }

    fn report(&self, status: JobStatus) {
        eprintln!("[job] {}", status.label());
    }

    fn mark(&self, job: &Job, manifest: &mut JobManifest, status: JobStatus) -> Result<()> {
        manifest.mark(status);
        job.write_manifest(manifest)?;
        self.report(status);
        Ok(())
    }

    fn manifest_for_existing_job(&self, job: &Job) -> Result<JobManifest> {
        Ok(job
            .read_manifest_if_exists()?
            .unwrap_or_else(|| JobManifest::new(job, None, None, LanguagePair::default())))
    }

    fn manifest_for_job(
        &self,
        job: &Job,
        input: Option<std::path::PathBuf>,
        render_output: Option<std::path::PathBuf>,
        languages: LanguagePair,
    ) -> Result<JobManifest> {
        let mut manifest = self.manifest_for_existing_job(job)?;
        manifest.input = input;
        manifest.render_output = render_output;
        manifest.source_language = languages.source;
        manifest.target_language = languages.target;
        Ok(manifest)
    }

    fn finish(&self, job: Job, mut manifest: JobManifest, result: Result<()>) -> Result<Job> {
        match result {
            Ok(()) => Ok(job),
            Err(error) => {
                manifest.fail(error.to_string());
                let _ = job.write_manifest(&manifest);
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, Mutex},
    };

    use anyhow::Result;

    use crate::{
        application::{
            AsrDataLocality, AsrFuture, AsrProvider, AsrProviderCapabilities, AsrProviderStatus,
            AsrRequest, AsrResponse, AsrTimingGranularity, TimedUnit, TimedUnitKind, TimingSource,
            TranscriptionOptions, TranslationFuture, TranslationOptions, TranslationProvider,
            TranslationProviderStatus, TranslationRequest, TranslationResponse, TranslationResult,
            TranslationUsage, translate_transcript,
        },
        domain::{LanguageCode, TranscriptSegment},
        infrastructure::{config::AppConfig, job_store::Job},
    };

    use super::JobRunner;

    #[derive(Debug)]
    struct FakeAsrProvider;

    #[derive(Debug, Default)]
    struct RecordingTranslationProvider {
        requests: Mutex<Vec<TranslationRequest>>,
    }

    impl TranslationProvider for RecordingTranslationProvider {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: "recording".into(),
                name: "Recording translator".into(),
                configured: true,
                model: Some("recording-v1".into()),
                endpoint_kind: "test".into(),
                configuration_hint: None,
            }
        }

        fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
            self.requests.lock().unwrap().push(request.clone());
            Box::pin(async move {
                Ok(TranslationResponse {
                    translations: request
                        .targets
                        .into_iter()
                        .map(|target| TranslationResult {
                            segment_id: target.segment_id,
                            translated_text: format!("译：{}", target.source_text),
                        })
                        .collect(),
                    model: Some("recording-v1".into()),
                    usage: TranslationUsage::default(),
                })
            })
        }
    }

    impl AsrProvider for FakeAsrProvider {
        fn status(&self) -> AsrProviderStatus {
            AsrProviderStatus {
                id: "fake-asr".into(),
                name: "Fake ASR".into(),
                capabilities: AsrProviderCapabilities {
                    timing_granularities: vec![AsrTimingGranularity::Word],
                    vocabulary_biasing: false,
                    diarization: false,
                    streaming: false,
                    data_locality: AsrDataLocality::Local,
                },
            }
        }

        fn transcribe<'a>(&'a self, request: AsrRequest) -> AsrFuture<'a> {
            Box::pin(async move {
                let raw_output_path = request.output_prefix.with_extension("json");
                fs::write(&raw_output_path, br#"{"provider":"fake"}"#)?;
                Ok(AsrResponse {
                    provider_id: "fake-asr".into(),
                    model_identity: "fake-model".into(),
                    raw_output_path,
                    timed_units: vec![TimedUnit {
                        id: "word-1".into(),
                        text: "テスト".into(),
                        kind: TimedUnitKind::Word,
                        start_ms: Some(0),
                        end_ms: Some(900),
                        timing_source: TimingSource::Model,
                        provider_confidence: Some(0.9),
                        speaker: None,
                    }],
                    legacy_segments: vec![TranscriptSegment::new(0, 900, "テスト".into())],
                })
            })
        }
    }

    #[tokio::test]
    async fn completed_candidate_run_does_not_mutate_the_workspace() -> Result<()> {
        let root =
            std::env::temp_dir().join(format!("atogaki-runner-test-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root)?;
        let existing = TranscriptSegment::new(0, 500, "人工编辑".into());
        job.write_segments(std::slice::from_ref(&existing))?;
        let input = job.dir.join("input.mp4");
        let audio = job.dir.join("audio.wav");
        fs::write(&input, [])?;
        fs::write(&audio, [])?;
        let runner = JobRunner::new(AppConfig {
            ffmpeg: "ffmpeg".into(),
            whisper_cli: "whisper-cli".into(),
            deepl_auth_key: None,
        })
        .with_asr_provider(Arc::new(FakeAsrProvider));
        let options = TranscriptionOptions::new("model.bin".into(), LanguageCode::Japanese);

        let candidates = runner
            .run_initial_asr(&job, &input, &audio, &options)
            .await?;

        assert_eq!(candidates.len(), 1);
        assert_eq!(job.read_segments()?[0].source_text, existing.source_text);
        let run_dirs = fs::read_dir(&job.asr_runs_dir)?.collect::<std::io::Result<Vec<_>>>()?;
        assert_eq!(run_dirs.len(), 1);
        let run_dir = run_dirs[0].path();
        assert!(run_dir.join("provider-output.json").is_file());
        assert!(run_dir.join("timed-units.json").is_file());
        assert!(run_dir.join("candidate-cues.json").is_file());

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[tokio::test]
    async fn cli_translation_uses_semantic_groups_and_stable_ids() -> Result<()> {
        let provider = RecordingTranslationProvider::default();
        let mut segments = vec![
            TranscriptSegment::new(0, 1_000, "第一句。".into()),
            TranscriptSegment::new(1_000, 2_000, "第二句".into()),
        ];

        translate_transcript(
            &provider,
            &TranslationOptions::new(LanguageCode::Japanese, LanguageCode::SimplifiedChinese),
            &mut segments,
        )
        .await?;

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].targets[0].segment_id, segments[0].id);
        assert_eq!(requests[1].targets[0].segment_id, segments[1].id);
        assert_eq!(requests[1].before_context[0].segment_id, segments[0].id);
        assert_eq!(segments[0].translated_text.as_deref(), Some("译：第一句。"));
        assert_eq!(segments[1].translated_text.as_deref(), Some("译：第二句"));
        Ok(())
    }
}
