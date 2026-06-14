use anyhow::{Result, anyhow};

use crate::{
    application::{
        job_manifest::JobManifest,
        job_spec::{ExportSpec, ProcessSpec, RenderSpec, TranscribeSpec, TranslateSpec},
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

    pub async fn transcribe(&self, spec: TranscribeSpec) -> Result<Job> {
        let job = Job::create(spec.output_dir.as_deref())?;
        let mut manifest = JobManifest::new(&job, Some(spec.input.clone()), None);
        self.mark(&job, &mut manifest, JobStatus::Created)?;

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::ExtractingAudio)?;
            let wav = media::extract_wav(&self.config.ffmpeg, &spec.input, &job.audio_wav).await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let raw =
                whisper::transcribe(&self.config.whisper_cli, &spec.whisper, &wav, &job.prefix)
                    .await?;

            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;
            let refined = segment::refine(glossary::apply_to_segments(&spec.whisper, raw)?);
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
            JobManifest::new(&job, Some(spec.input.clone()), spec.render_output.clone());
        self.mark(&job, &mut manifest, JobStatus::Created)?;

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::ExtractingAudio)?;
            let wav = media::extract_wav(&self.config.ffmpeg, &spec.input, &job.audio_wav).await?;

            self.mark(&job, &mut manifest, JobStatus::Transcribing)?;
            let raw =
                whisper::transcribe(&self.config.whisper_cli, &spec.whisper, &wav, &job.prefix)
                    .await?;

            self.mark(&job, &mut manifest, JobStatus::RefiningSegments)?;
            let mut segments = segment::refine(glossary::apply_to_segments(&spec.whisper, raw)?);

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;

            if let Some(key) = spec.deepl_auth_key.or(self.config.deepl_auth_key.clone()) {
                self.mark(&job, &mut manifest, JobStatus::Translating)?;
                deepl::translate_segments(&key, &mut segments).await?;

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
        let mut manifest = JobManifest::new(&job, None, None);
        let result = async {
            let mut segments = job.read_segments()?;
            let key = spec
                .deepl_auth_key
                .or(self.config.deepl_auth_key.clone())
                .ok_or_else(|| {
                    anyhow!("DeepL key missing. Set DEEPL_AUTH_KEY or pass --deepl-auth-key")
                })?;

            self.mark(&job, &mut manifest, JobStatus::Translating)?;
            deepl::translate_segments(&key, &mut segments).await?;
            job.write_segments(&segments)?;

            self.mark(&job, &mut manifest, JobStatus::ExportingSubtitles)?;
            self.write_translated_outputs(&job, &segments)?;

            self.mark(&job, &mut manifest, JobStatus::Done)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.finish(job, manifest, result)
    }

    pub fn export(&self, spec: ExportSpec) -> Result<Job> {
        let job = Job::open(spec.job_dir)?;
        let mut manifest = JobManifest::new(&job, None, None);

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
        let mut manifest =
            JobManifest::new(&job, Some(spec.input.clone()), Some(spec.output.clone()));

        let result = async {
            self.mark(&job, &mut manifest, JobStatus::RenderingVideo)?;
            media::render_subtitles(
                &self.config.ffmpeg,
                &spec.input,
                &job.bilingual_ass,
                &job.bilingual_srt,
                &spec.output,
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
