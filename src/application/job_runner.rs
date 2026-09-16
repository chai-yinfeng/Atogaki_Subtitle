use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, anyhow};

use crate::{
    application::{
        AsrInputScope, AsrRequest, AsrRun, OfflineAsrProvider, TranscriptionOptions,
        job_manifest::JobManifest,
        job_snapshot::JobSnapshot,
        job_spec::{
            ApplyGlossarySpec, ExportSpec, ProcessSpec, RenderSpec, RerenderSpec, TranscribeSpec,
            TranslateSpec,
        },
        job_status::JobStatus,
        segment_timed_units,
    },
    domain::{LanguageCode, LanguagePair, glossary, subtitle},
    infrastructure::{
        asr_run_store::AsrRunArtifacts, config::AppConfig, deepl, job_store::Job,
        local_db::LocalDatabase, media, whisper_asr::WhisperAsrProvider,
    },
};

pub struct JobRunner {
    config: AppConfig,
    asr_provider: Arc<dyn OfflineAsrProvider>,
    database: Option<LocalDatabase>,
}

impl JobRunner {
    pub fn new(config: AppConfig) -> Self {
        let asr_provider = Arc::new(WhisperAsrProvider::new(config.whisper_cli.clone()));
        Self {
            config,
            asr_provider,
            database: None,
        }
    }

    pub fn with_database(mut self, database: LocalDatabase) -> Self {
        self.database = Some(database);
        self
    }

    pub fn with_asr_provider(mut self, provider: Arc<dyn OfflineAsrProvider>) -> Self {
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
            let wav = media::extract_wav(&self.config.ffmpeg, &spec.input, &job.audio_wav).await?;

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
            let wav = media::extract_wav(&self.config.ffmpeg, &spec.input, &job.audio_wav).await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let mut segments = self
                .run_initial_asr(&job, &spec.input, &wav, &spec.transcription)
                .await?;
            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.source_srt, &segments, subtitle::SubtitleTrack::Source)?;

            if let Some(key) = spec.deepl_auth_key.or(self.config.deepl_auth_key.clone()) {
                self.mark(&job, &mut manifest, JobStatus::Translating)?;
                deepl::translate_segments(&key, &spec.translation, &mut segments).await?;

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
            deepl::translate_segments(&key, &spec.translation, &mut segments).await?;
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
        let mut run = AsrRun::new(
            job.id(),
            None,
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

        let response = self
            .asr_provider
            .transcribe(AsrRequest {
                audio_path: audio_path.to_path_buf(),
                output_prefix: artifacts.provider_output_prefix.clone(),
                scope: AsrInputScope::full(),
                transcription: options.clone(),
            })
            .await;

        let completion: Result<Vec<crate::domain::TranscriptSegment>> = async {
            let response = response?;
            let mut cue_set = segment_timed_units(&response.timed_units, options.source_language)?;
            let candidates = glossary::apply_to_segments(options, cue_set.transcript_segments())?;
            cue_set.replace_transcript_segments(candidates.clone())?;
            artifacts.write_timed_units(&response.timed_units)?;
            artifacts.write_candidate_cues(&cue_set)?;
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
    use std::{fs, sync::Arc};

    use anyhow::Result;

    use crate::{
        application::{
            AsrFuture, AsrProviderCapabilities, AsrProviderStatus, AsrRequest, AsrResponse,
            AsrTimingGranularity, OfflineAsrProvider, TimedUnit, TimedUnitKind, TimingSource,
            TranscriptionOptions,
        },
        domain::{LanguageCode, TranscriptSegment},
        infrastructure::{config::AppConfig, job_store::Job},
    };

    use super::JobRunner;

    #[derive(Debug)]
    struct FakeAsrProvider;

    impl OfflineAsrProvider for FakeAsrProvider {
        fn status(&self) -> AsrProviderStatus {
            AsrProviderStatus {
                id: "fake-asr".into(),
                name: "Fake ASR".into(),
                capabilities: AsrProviderCapabilities {
                    timing_granularities: vec![AsrTimingGranularity::Word],
                    vocabulary_biasing: false,
                    diarization: false,
                    streaming: false,
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
}
