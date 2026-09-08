use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

use crate::{
    application::LocalWorkspaceService,
    domain::{render::RenderOptions, subtitle::SubtitleTrack},
    infrastructure::{
        local_db::{LocalDatabase, LocalRenderJobRecord, NewLocalRenderJob},
        media::{self, RenderCancelled, RenderEncoder},
    },
};

const RENDER_QUEUE_CAPACITY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRenderQuality {
    Compact,
    Balanced,
    High,
}

impl LocalRenderQuality {
    fn key(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Balanced => "balanced",
            Self::High => "high",
        }
    }

    fn apply_to(self, balanced_bitrate: u64) -> u64 {
        match self {
            Self::Compact => balanced_bitrate.saturating_mul(5) / 8,
            Self::Balanced => balanced_bitrate,
            Self::High => balanced_bitrate.saturating_mul(3) / 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalRenderEstimate {
    pub quality: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: u64,
    pub target_video_bitrate_bps: u64,
    pub estimated_audio_bitrate_bps: u64,
    pub estimated_size_bytes: u64,
    pub bitrate_source: String,
}

#[derive(Debug, Clone)]
pub struct LocalRenderRequest {
    pub source_job_id: String,
    pub output_path: PathBuf,
    pub subtitle_track: SubtitleTrack,
    pub quality: LocalRenderQuality,
    pub overwrite_existing: bool,
}

#[derive(Clone)]
pub struct LocalRenderService {
    ffmpeg: PathBuf,
    database: LocalDatabase,
    workspace: LocalWorkspaceService,
    sender: mpsc::Sender<QueuedRender>,
    cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

struct QueuedRender {
    record: LocalRenderJobRecord,
    duration_ms: u64,
    target_video_bitrate_bps: u64,
    overwrite_existing: bool,
    cancelled: Arc<AtomicBool>,
}

impl LocalRenderService {
    pub async fn start(
        ffmpeg: PathBuf,
        database: LocalDatabase,
        workspace: LocalWorkspaceService,
    ) -> Result<Self> {
        database.interrupt_unfinished_renders().await?;
        let (sender, receiver) = mpsc::channel(RENDER_QUEUE_CAPACITY);
        let cancellations = Arc::new(Mutex::new(HashMap::new()));
        tokio::spawn(run_render_worker(
            ffmpeg.clone(),
            database.clone(),
            receiver,
            Arc::clone(&cancellations),
        ));
        Ok(Self {
            ffmpeg,
            database,
            workspace,
            sender,
            cancellations,
        })
    }

    pub async fn capabilities(&self) -> Result<media::MediaCapabilities> {
        media::inspect_capabilities(&self.ffmpeg).await
    }

    pub async fn estimates(&self, source_job_id: &str) -> Result<Vec<LocalRenderEstimate>> {
        let job = self
            .database
            .get_job(source_job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {source_job_id}"))?;
        let input = job
            .input_path
            .as_deref()
            .map(Path::new)
            .ok_or_else(|| anyhow!("source task does not have an input media path"))?;
        let probe = media::probe_media(&self.ffmpeg, input).await?;
        if !probe.has_video {
            bail!("the selected task contains audio only; video subtitle rendering is unavailable");
        }
        Ok([
            LocalRenderQuality::Compact,
            LocalRenderQuality::Balanced,
            LocalRenderQuality::High,
        ]
        .into_iter()
        .map(|quality| render_estimate(probe, quality))
        .collect())
    }

    pub async fn submit(&self, request: LocalRenderRequest) -> Result<LocalRenderJobRecord> {
        validate_output_path(&request.output_path, request.overwrite_existing)?;
        let workspace = self.workspace.get_job(&request.source_job_id).await?;
        if workspace.job.status != "done" {
            bail!(
                "source task must be done before rendering video; current status is {}",
                workspace.job.status
            );
        }
        let input_path = workspace
            .job
            .input_path
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("source task does not have an input media path"))?;
        let input_path = input_path
            .canonicalize()
            .with_context(|| format!("failed to resolve source media {}", input_path.display()))?;
        if request.output_path.exists()
            && request.output_path.canonicalize().ok().as_deref() == Some(input_path.as_path())
        {
            bail!("refusing to overwrite the source media file");
        }

        let capabilities = self.capabilities().await?;
        if !capabilities.ready_for_hard_subtitles {
            bail!(
                "configured ffmpeg cannot burn subtitles: libass={}, hardware H.264={}, MPEG-4={}",
                capabilities.ass_filter,
                capabilities.videotoolbox_encoder,
                capabilities.mpeg4_encoder
            );
        }
        let probe = media::probe_media(&self.ffmpeg, &input_path).await?;
        if !probe.has_video {
            bail!("the selected task contains audio only; video subtitle rendering is unavailable");
        }
        let estimate = render_estimate(probe, request.quality);

        let render_id = format!("render-{}", Uuid::new_v4());
        let render_directory = PathBuf::from(&workspace.job.storage_dir).join("renders");
        fs::create_dir_all(&render_directory).with_context(|| {
            format!(
                "failed to create video render directory {}",
                render_directory.display()
            )
        })?;
        let subtitle_path = render_directory.join(format!(
            "{render_id}.{}.ass",
            subtitle_track_key(request.subtitle_track)
        ));
        if let Err(error) = self
            .workspace
            .export_ass_snapshot(
                &request.source_job_id,
                &subtitle_path,
                request.subtitle_track,
            )
            .await
        {
            let _ = fs::remove_file(&subtitle_path);
            return Err(error);
        }

        let record = match self
            .database
            .create_render_job(NewLocalRenderJob {
                id: render_id.clone(),
                source_job_id: request.source_job_id,
                input_path: input_path.display().to_string(),
                subtitle_path: subtitle_path.display().to_string(),
                output_path: request.output_path.display().to_string(),
                subtitle_track: subtitle_track_key(request.subtitle_track).to_string(),
            })
            .await
        {
            Ok(record) => record,
            Err(error) => {
                let _ = fs::remove_file(&subtitle_path);
                return Err(error);
            }
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .await
            .insert(render_id.clone(), Arc::clone(&cancelled));
        if let Err(error) = self
            .sender
            .send(QueuedRender {
                record: record.clone(),
                duration_ms: probe.duration_ms,
                target_video_bitrate_bps: estimate.target_video_bitrate_bps,
                overwrite_existing: request.overwrite_existing,
                cancelled,
            })
            .await
        {
            self.cancellations.lock().await.remove(&render_id);
            let message = format!("local video render worker is unavailable: {error}");
            self.database.fail_render(&render_id, &message).await?;
            bail!(message);
        }
        Ok(record)
    }

    pub async fn list(&self, source_job_id: &str) -> Result<Vec<LocalRenderJobRecord>> {
        self.database.list_render_jobs(source_job_id).await
    }

    pub async fn cancel(&self, render_id: &str) -> Result<LocalRenderJobRecord> {
        let record = self
            .database
            .get_render_job(render_id)
            .await?
            .ok_or_else(|| anyhow!("local video render not found: {render_id}"))?;
        if matches!(record.status.as_str(), "done" | "failed" | "cancelled") {
            return Ok(record);
        }
        let cancellations = self.cancellations.lock().await;
        let flag = cancellations
            .get(render_id)
            .ok_or_else(|| anyhow!("video render is no longer attached to this app session"))?;
        flag.store(true, Ordering::Relaxed);
        drop(cancellations);
        if record.status == "queued" {
            self.database.cancel_render(render_id).await?;
        }
        self.database
            .get_render_job(render_id)
            .await?
            .ok_or_else(|| anyhow!("cancelled local video render disappeared: {render_id}"))
    }
}

async fn run_render_worker(
    ffmpeg: PathBuf,
    database: LocalDatabase,
    mut receiver: mpsc::Receiver<QueuedRender>,
    cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
) {
    while let Some(render) = receiver.recv().await {
        let render_id = render.record.id.clone();
        if render.cancelled.load(Ordering::Relaxed) {
            let _ = database.cancel_render(&render_id).await;
            cancellations.lock().await.remove(&render_id);
            continue;
        }
        if let Err(error) = database.mark_render_running(&render_id).await {
            eprintln!("failed to start local video render {render_id}: {error:#}");
            cancellations.lock().await.remove(&render_id);
            continue;
        }

        let output_path = PathBuf::from(&render.record.output_path);
        let temporary_path = temporary_output_path(&output_path, &render_id);
        let _ = fs::remove_file(&temporary_path);
        let (progress_sender, mut progress_receiver) =
            mpsc::unbounded_channel::<media::RenderProgress>();
        let progress_database = database.clone();
        let progress_render_id = render_id.clone();
        let progress_task = tokio::spawn(async move {
            let mut last_persisted = 0.0;
            while let Some(progress) = progress_receiver.recv().await {
                if progress.progress >= 0.99 || progress.progress - last_persisted >= 0.005 {
                    let _ = progress_database
                        .update_render_progress(&progress_render_id, progress.progress)
                        .await;
                    last_persisted = progress.progress;
                }
            }
        });
        let options = RenderOptions {
            video_crf: 20,
            video_preset: "medium".to_string(),
            target_video_bitrate_bps: Some(render.target_video_bitrate_bps),
            soft_subtitles: false,
        };
        let result = media::render_subtitles_with_progress(
            &ffmpeg,
            Path::new(&render.record.input_path),
            Path::new(&render.record.subtitle_path),
            Path::new(&render.record.subtitle_path),
            &temporary_path,
            &options,
            render.duration_ms,
            Some(progress_sender),
            Arc::clone(&render.cancelled),
        )
        .await;
        let _ = progress_task.await;

        match result {
            Ok(outcome) => {
                if let Err(error) = install_rendered_output(
                    &temporary_path,
                    &output_path,
                    &render_id,
                    render.overwrite_existing,
                ) {
                    let _ = fs::remove_file(&temporary_path);
                    let _ = database.fail_render(&render_id, &error.to_string()).await;
                } else {
                    let encoder = match outcome.encoder {
                        RenderEncoder::VideoToolbox => "videotoolbox",
                        RenderEncoder::Mpeg4 => "mpeg4",
                        RenderEncoder::SubtitleMux => "subtitle_mux",
                    };
                    let _ = database
                        .finish_render(
                            &render_id,
                            encoder,
                            &outcome.audio_encoder,
                            outcome.fallback_reason.as_deref(),
                        )
                        .await;
                }
            }
            Err(error) if error.downcast_ref::<RenderCancelled>().is_some() => {
                let _ = fs::remove_file(&temporary_path);
                let _ = database.cancel_render(&render_id).await;
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                let _ = database
                    .fail_render(&render_id, &format!("{error:#}"))
                    .await;
            }
        }
        cancellations.lock().await.remove(&render_id);
    }
}

fn render_estimate(probe: media::MediaProbe, quality: LocalRenderQuality) -> LocalRenderEstimate {
    const AUDIO_FALLBACK_BITRATE_BPS: u64 = 192_000;
    let (balanced_bitrate, bitrate_source) = match probe.video_bitrate_bps {
        Some(source) => (source.saturating_mul(6) / 5, "source_bitrate"),
        None => {
            let height = probe.height.unwrap_or(1080);
            let baseline = if height <= 480 {
                1_500_000
            } else if height <= 720 {
                3_000_000
            } else if height <= 1080 {
                6_000_000
            } else {
                12_000_000
            };
            (baseline, "resolution_fallback")
        }
    };
    let target_video_bitrate_bps = quality.apply_to(balanced_bitrate).max(250_000);
    let estimated_audio_bitrate_bps = probe
        .audio_bitrate_bps
        .unwrap_or(AUDIO_FALLBACK_BITRATE_BPS);
    let estimated_size_bytes = target_video_bitrate_bps
        .saturating_add(estimated_audio_bitrate_bps)
        .saturating_mul(probe.duration_ms)
        .saturating_div(8_000)
        .saturating_mul(102)
        .saturating_div(100);
    LocalRenderEstimate {
        quality: quality.key().to_string(),
        width: probe.width,
        height: probe.height,
        duration_ms: probe.duration_ms,
        target_video_bitrate_bps,
        estimated_audio_bitrate_bps,
        estimated_size_bytes,
        bitrate_source: bitrate_source.to_string(),
    }
}

fn subtitle_track_key(track: SubtitleTrack) -> &'static str {
    match track {
        SubtitleTrack::Source => "source",
        SubtitleTrack::Translation => "translation",
        SubtitleTrack::Bilingual => "bilingual",
    }
}

fn validate_output_path(output: &Path, overwrite_existing: bool) -> Result<()> {
    if output.extension().and_then(|extension| extension.to_str()) != Some("mp4") {
        bail!("video render output must use the .mp4 extension");
    }
    let parent = output
        .parent()
        .ok_or_else(|| anyhow!("video render output must have a parent directory"))?;
    if !parent.is_dir() {
        bail!(
            "video render output directory does not exist: {}",
            parent.display()
        );
    }
    if output.exists() && !overwrite_existing {
        bail!("video render output already exists: {}", output.display());
    }
    if output
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        bail!(
            "refusing to overwrite a symbolic link: {}",
            output.display()
        );
    }
    Ok(())
}

fn temporary_output_path(output: &Path, render_id: &str) -> PathBuf {
    let file_name = output
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Atogaki");
    output.with_file_name(format!(".{file_name}.{render_id}.partial.mp4"))
}

fn install_rendered_output(
    temporary: &Path,
    output: &Path,
    render_id: &str,
    overwrite_existing: bool,
) -> Result<()> {
    if !temporary.is_file() {
        bail!("render completed without producing {}", temporary.display());
    }
    if !output.exists() {
        return fs::rename(temporary, output)
            .with_context(|| format!("failed to move completed render to {}", output.display()));
    }
    if !overwrite_existing {
        bail!(
            "video render output appeared while rendering: {}",
            output.display()
        );
    }

    let backup = output.with_file_name(format!(".atogaki-{render_id}.previous.mp4"));
    let _ = fs::remove_file(&backup);
    fs::rename(output, &backup)
        .with_context(|| format!("failed to protect existing output {}", output.display()))?;
    if let Err(error) = fs::rename(temporary, output) {
        let _ = fs::rename(&backup, output);
        return Err(error).with_context(|| {
            format!("failed to install completed render at {}", output.display())
        });
    }
    if let Err(error) = fs::remove_file(&backup) {
        eprintln!(
            "completed render was installed, but failed to remove old output backup {}: {error}",
            backup.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command, time::Duration};

    use super::{
        LocalRenderQuality, LocalRenderRequest, LocalRenderService, install_rendered_output,
        render_estimate, temporary_output_path, validate_output_path,
    };
    use crate::{
        application::{
            LocalWorkspaceService, job_manifest::JobManifest, job_snapshot::JobSnapshot,
            job_status::JobStatus,
        },
        domain::{TranscriptSegment, subtitle::SubtitleTrack},
        infrastructure::{
            config::desktop_ffmpeg_path, job_store::Job, local_db::LocalDatabase, media::MediaProbe,
        },
    };

    #[test]
    fn render_quality_presets_have_explicit_bitrates_and_size_estimates() {
        let probe = MediaProbe {
            duration_ms: 60_000,
            has_video: true,
            video_bitrate_bps: Some(4_000_000),
            width: Some(1920),
            height: Some(1080),
            audio_bitrate_bps: Some(128_000),
        };

        let compact = render_estimate(probe, LocalRenderQuality::Compact);
        let balanced = render_estimate(probe, LocalRenderQuality::Balanced);
        let high = render_estimate(probe, LocalRenderQuality::High);

        assert_eq!(compact.target_video_bitrate_bps, 3_000_000);
        assert_eq!(balanced.target_video_bitrate_bps, 4_800_000);
        assert_eq!(high.target_video_bitrate_bps, 7_200_000);
        assert_eq!(balanced.estimated_size_bytes, 37_699_200);
        assert_eq!(balanced.bitrate_source, "source_bitrate");
        assert_eq!((balanced.width, balanced.height), (Some(1920), Some(1080)));
    }

    #[test]
    fn completed_render_replaces_an_existing_output_without_exposing_partial_data() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-render-install-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("episode.mp4");
        let temporary = temporary_output_path(&output, "render-test");
        fs::write(&output, b"old").unwrap();
        fs::write(&temporary, b"new").unwrap();

        validate_output_path(&output, true).unwrap();
        install_rendered_output(&temporary, &output, "render-test", true).unwrap();

        assert_eq!(fs::read(&output).unwrap(), b"new");
        assert!(!temporary.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a built LGPL FFmpeg sidecar"]
    async fn lgpl_sidecar_renders_a_persisted_sqlite_workspace() {
        let ffmpeg = desktop_ffmpeg_path();
        assert!(
            ffmpeg.is_file(),
            "missing FFmpeg sidecar at {}",
            ffmpeg.display()
        );
        let root =
            std::env::temp_dir().join(format!("atogaki-real-render-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("input.mp4");
        let generated = Command::new(&ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=navy:s=640x360:d=1",
                "-c:v",
                "mpeg4",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .output()
            .unwrap();
        assert!(generated.status.success());

        let job = Job::create_in(&root.join("jobs")).unwrap();
        let mut manifest = JobManifest::new(
            &job,
            Some(input.clone()),
            None,
            crate::domain::LanguagePair::default(),
        );
        manifest.mark(JobStatus::Done);
        let mut segment = TranscriptSegment::new(0, 900, "テスト字幕".to_string());
        segment.set_translation(Some("测试字幕".to_string()));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![segment],
            })
            .await
            .unwrap();
        let workspace = LocalWorkspaceService::new(database.clone());
        let service = LocalRenderService::start(ffmpeg, database.clone(), workspace)
            .await
            .unwrap();
        let output = root.join("rendered.mp4");
        service
            .submit(LocalRenderRequest {
                source_job_id: manifest.job_id.clone(),
                output_path: output.clone(),
                subtitle_track: SubtitleTrack::Bilingual,
                quality: LocalRenderQuality::Balanced,
                overwrite_existing: false,
            })
            .await
            .unwrap();

        let mut finished = None;
        for _ in 0..200 {
            let current = service.list(&manifest.job_id).await.unwrap();
            if current
                .first()
                .is_some_and(|render| matches!(render.status.as_str(), "done" | "failed"))
            {
                finished = current.into_iter().next();
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let finished = finished.expect("video render did not finish within ten seconds");
        assert_eq!(finished.status, "done", "{:?}", finished.error_message);
        assert_eq!(finished.progress, 1.0);
        if finished.encoder.as_deref() == Some("mpeg4") {
            assert!(
                finished
                    .fallback_reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("VideoToolbox"))
            );
        }
        assert!(output.is_file());

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
