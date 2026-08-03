use anyhow::{Result, anyhow};

use crate::{
    application::{
        job_manifest::JobManifest,
        job_snapshot::JobSnapshot,
        job_spec::{
            ApplyGlossarySpec, ExportSpec, ProcessSpec, RenderSpec, RerenderSpec, TranscribeSpec,
            TranslateSpec,
        },
        job_status::JobStatus,
    },
    domain::{glossary, segment, subtitle},
    infrastructure::{config::AppConfig, deepl, job_store::Job, media, whisper},
};

pub struct JobRunner {
    config: AppConfig,
}

impl JobRunner {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    /// Reads the durable state of a task without invoking any media tooling.
    /// This is the query entry point for UI task lists and subtitle editors.
    pub fn snapshot(&self, job_dir: impl AsRef<std::path::Path>) -> Result<JobSnapshot> {
        JobSnapshot::load(job_dir)
    }

    pub async fn transcribe(&self, spec: TranscribeSpec) -> Result<Job> {
        let job = Job::create(spec.output_dir.as_deref())?;
        let mut manifest = self.manifest_for_job(&job, Some(spec.input.clone()), None)?;
        self.mark(&job, &mut manifest, JobStatus::Created)?;

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::ExtractingAudio)?;
            let wav = media::extract_wav(&self.config.ffmpeg, &spec.input, &job.audio_wav).await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let raw = whisper::transcribe(
                &self.config.whisper_cli,
                &spec.transcription,
                &wav,
                &job.prefix,
            )
            .await?;

            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;
            let refined = segment::refine(glossary::apply_to_segments(&spec.transcription, raw)?);
            job.write_segments(&refined)?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.ja_srt, &refined, subtitle::SubtitleTrack::Japanese)?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result)
    }

    pub async fn process(&self, spec: ProcessSpec) -> Result<Job> {
        let job = Job::create(spec.output_dir.as_deref())?;
        let mut manifest =
            self.manifest_for_job(&job, Some(spec.input.clone()), spec.render_output.clone())?;
        self.mark(&job, &mut manifest, JobStatus::Created)?;

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::ExtractingAudio)?;
            let wav = media::extract_wav(&self.config.ffmpeg, &spec.input, &job.audio_wav).await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let raw = whisper::transcribe(
                &self.config.whisper_cli,
                &spec.transcription,
                &wav,
                &job.prefix,
            )
            .await?;

            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;
            let mut segments =
                segment::refine(glossary::apply_to_segments(&spec.transcription, raw)?);

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;

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
                eprintln!("DeepL key missing; wrote Japanese transcript only.");
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
            subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;
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
            subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;
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
        subtitle::write_srt(&job.zh_srt, segments, subtitle::SubtitleTrack::Chinese)?;
        subtitle::write_srt(
            &job.bilingual_srt,
            segments,
            subtitle::SubtitleTrack::Bilingual,
        )?;
        subtitle::write_ass(&job.bilingual_ass, segments)?;
        Ok(())
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
            .unwrap_or_else(|| JobManifest::new(job, None, None)))
    }

    fn manifest_for_job(
        &self,
        job: &Job,
        input: Option<std::path::PathBuf>,
        render_output: Option<std::path::PathBuf>,
    ) -> Result<JobManifest> {
        let mut manifest = self.manifest_for_existing_job(job)?;
        manifest.input = input;
        manifest.render_output = render_output;
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
