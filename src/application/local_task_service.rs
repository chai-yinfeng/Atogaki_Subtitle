use std::{
    collections::HashMap,
    fs,
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use tokio::sync::{Mutex, mpsc, watch};

use crate::{
    application::{
        job_manifest::JobManifest,
        job_runner::JobRunner,
        job_snapshot::JobSnapshot,
        job_spec::{ProcessSpec, TranscribeSpec},
        job_status::JobStatus,
        local_glossary_service::glossary_for_task,
    },
    domain::{LanguageCode, LanguagePair},
    infrastructure::{
        asr_run_store::AsrRunArtifacts, config::AppConfig, job_store::Job, local_db::LocalDatabase,
    },
};

const DEFAULT_QUEUE_CAPACITY: usize = 8;

/// A durable, local queue for the long-running media operations used by the
/// desktop UI. Queued state is written to disk before work is sent to a worker,
/// so the UI can immediately poll a task without holding an HTTP request open.
#[derive(Clone)]
pub struct LocalTaskService {
    sender: mpsc::Sender<QueuedTask>,
    scheduler: Arc<crate::application::stage_scheduler::StageScheduler>,
    controls: Arc<Mutex<HashMap<String, TaskControl>>>,
    automatic: Arc<Mutex<HashMap<String, crate::application::LocalWorkspaceService>>>,
    jobs_dir: PathBuf,
    database: Option<LocalDatabase>,
}

struct TaskControl {
    finished: watch::Sender<bool>,
    sender: watch::Sender<bool>,
    started: bool,
}
impl TaskControl {
    fn new() -> Self {
        Self {
            finished: watch::channel(false).0,
            sender: watch::channel(false).0,
            started: false,
        }
    }
}

enum QueuedTask {
    Transcribe {
        job_dir: PathBuf,
        spec: TranscribeSpec,
    },
    Process {
        job_dir: PathBuf,
        spec: ProcessSpec,
    },
}

impl LocalTaskService {
    pub fn stage_scheduler(&self) -> Arc<crate::application::stage_scheduler::StageScheduler> {
        self.scheduler.clone()
    }
    pub fn schedule_translation_provider(
        &self,
        provider: Arc<dyn crate::application::TranslationProvider>,
    ) -> Arc<dyn crate::application::TranslationProvider> {
        self.scheduler.translation_provider(provider)
    }

    pub async fn configure_task_translation(
        &self,
        job_id: &str,
        provider: Arc<dyn crate::application::TranslationProvider>,
    ) -> Result<()> {
        let db = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("translation requires SQLite"))?;
        self.automatic.lock().await.insert(
            job_id.into(),
            crate::application::LocalWorkspaceService::with_provider(
                db.clone(),
                self.scheduler.translation_provider(provider),
            ),
        );
        Ok(())
    }

    /// Starts the desktop stage scheduler with two concurrent ASR jobs.
    pub fn start(config: AppConfig, jobs_dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_workers(
            config,
            jobs_dir,
            crate::application::stage_scheduler::DEFAULT_ASR_CONCURRENCY,
            DEFAULT_QUEUE_CAPACITY,
        )
    }

    /// Opens the SQLite-backed variant used by the desktop application.
    pub async fn start_persistent(
        config: AppConfig,
        jobs_dir: impl Into<PathBuf>,
        database_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let database = LocalDatabase::open(database_path).await?;
        Self::start_with_database(config, jobs_dir, database)
    }

    pub fn start_with_database(
        config: AppConfig,
        jobs_dir: impl Into<PathBuf>,
        database: LocalDatabase,
    ) -> Result<Self> {
        Self::with_workers_and_database(
            config,
            jobs_dir,
            crate::application::stage_scheduler::DEFAULT_ASR_CONCURRENCY,
            DEFAULT_QUEUE_CAPACITY,
            Some(database),
        )
    }

    /// Starts a bounded queue with an explicit number of workers.
    pub fn with_workers(
        config: AppConfig,
        jobs_dir: impl Into<PathBuf>,
        workers: usize,
        queue_capacity: usize,
    ) -> Result<Self> {
        Self::with_workers_and_database(config, jobs_dir, workers, queue_capacity, None)
    }

    pub fn with_workers_and_database(
        config: AppConfig,
        jobs_dir: impl Into<PathBuf>,
        workers: usize,
        queue_capacity: usize,
        database: Option<LocalDatabase>,
    ) -> Result<Self> {
        if workers == 0 {
            bail!("local task service requires at least one worker");
        }
        if queue_capacity == 0 {
            bail!("local task service queue capacity must be at least one");
        }

        let handle = tokio::runtime::Handle::try_current()
            .context("local task service must be started inside a Tokio runtime")?;
        let (sender, receiver) = mpsc::channel(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let controls = Arc::new(Mutex::new(HashMap::new()));
        let automatic = Arc::new(Mutex::new(HashMap::new()));

        let scheduler = crate::application::stage_scheduler::StageScheduler::new(workers);
        handle.spawn(run_worker(
            config.clone(),
            receiver,
            database.clone(),
            controls.clone(),
            automatic.clone(),
            scheduler.clone(),
        ));

        Ok(Self {
            scheduler,
            sender,
            controls,
            automatic,
            jobs_dir: jobs_dir.into(),
            database,
        })
    }

    pub async fn submit_transcription(&self, spec: TranscribeSpec) -> Result<JobSnapshot> {
        self.submit_transcription_with_glossary(spec, None, &[])
            .await
    }

    pub async fn submit_transcription_with_glossary(
        &self,
        spec: TranscribeSpec,
        glossary_id: Option<&str>,
        selected_content_groups: &[String],
    ) -> Result<JobSnapshot> {
        self.submit_with_translation(spec, glossary_id, selected_content_groups, None)
            .await
    }

    pub async fn submit_with_translation(
        &self,
        mut spec: TranscribeSpec,
        glossary_id: Option<&str>,
        selected_content_groups: &[String],
        translation: Option<(
            Arc<dyn crate::application::TranslationProvider>,
            serde_json::Value,
        )>,
    ) -> Result<JobSnapshot> {
        self.require_service_owned_output_dir(spec.output_dir.as_deref())?;
        let languages = LanguagePair::new(
            spec.transcription.source_language,
            LanguageCode::SimplifiedChinese,
        )?;
        let glossary_selection = if let Some(glossary_id) = glossary_id {
            let database = self.database.as_ref().ok_or_else(|| {
                anyhow!("glossary selection requires SQLite-backed local task service")
            })?;
            let detail = database
                .get_glossary(glossary_id)
                .await?
                .ok_or_else(|| anyhow!("local glossary not found: {glossary_id}"))?;
            if detail.glossary.source_language != spec.transcription.source_language.as_str() {
                bail!(
                    "glossary language {} does not match transcription language {}",
                    detail.glossary.source_language,
                    spec.transcription.source_language
                );
            }
            let resolved = glossary_for_task(&detail, selected_content_groups)?;
            Some((detail, resolved))
        } else {
            None
        };
        let job = self
            .create_queued_job(Some(spec.input.clone()), None, languages)
            .await?;
        if let Some((detail, resolved)) = glossary_selection {
            let database = self
                .database
                .as_ref()
                .expect("glossary database was validated");
            let snapshot_path = job.dir.join("recognition-glossary.txt");
            tokio::fs::write(&snapshot_path, resolved.to_file_text())
                .await
                .with_context(|| {
                    format!(
                        "failed to write task glossary snapshot {}",
                        snapshot_path.display()
                    )
                })?;
            database
                .assign_job_glossary(
                    job.id().as_str(),
                    &detail.glossary.id,
                    &detail.glossary.name,
                    &snapshot_path,
                )
                .await?;
            spec.transcription.glossary = Some(snapshot_path);
        }
        if let Some((provider, profile)) = translation {
            crate::application::acoustic_plan::write_json(
                &job.dir.join("task-preferences.json"),
                &profile,
            )?;
            self.configure_task_translation(&job.id(), provider).await?;
        }
        spec.output_dir = Some(job.dir.clone());
        let prompt = crate::domain::glossary::build_whisper_prompt(&spec.transcription)?;
        job.write_whisper_prompt(prompt.as_deref())?;
        job.write_recognition_options(&spec.transcription)?;
        self.enqueue(
            job.clone(),
            QueuedTask::Transcribe {
                job_dir: job.dir.clone(),
                spec,
            },
        )
        .await
    }

    pub async fn submit_process(&self, mut spec: ProcessSpec) -> Result<JobSnapshot> {
        self.require_service_owned_output_dir(spec.output_dir.as_deref())?;
        let languages = LanguagePair::new(
            spec.translation.source_language,
            spec.translation.target_language,
        )?;
        if spec.transcription.source_language != languages.source {
            bail!(
                "transcription language {} does not match translation source {}",
                spec.transcription.source_language,
                languages.source
            );
        }
        let job = self
            .create_queued_job(
                Some(spec.input.clone()),
                spec.render_output.clone(),
                languages,
            )
            .await?;
        spec.output_dir = Some(job.dir.clone());
        let prompt = crate::domain::glossary::build_whisper_prompt(&spec.transcription)?;
        job.write_whisper_prompt(prompt.as_deref())?;
        job.write_recognition_options(&spec.transcription)?;
        self.enqueue(
            job.clone(),
            QueuedTask::Process {
                job_dir: job.dir.clone(),
                spec,
            },
        )
        .await
    }

    /// Reads a task's current durable state. A UI can poll this method now and
    /// switch to event-driven updates later without changing task persistence.
    pub fn snapshot(&self, job_dir: impl AsRef<Path>) -> Result<JobSnapshot> {
        JobSnapshot::load(job_dir)
    }

    pub async fn list_persisted_jobs(
        &self,
    ) -> Result<Vec<crate::infrastructure::local_db::LocalJobRecord>> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("local task service was started without SQLite persistence"))?;
        let jobs = database.list_jobs().await?;
        for job in jobs {
            if let Ok(snapshot) = JobSnapshot::load(&job.storage_dir)
                && snapshot.manifest.updated_at_unix > job.updated_at_unix as u64
            {
                database.sync_snapshot(&snapshot).await?;
            }
        }
        database.list_jobs().await
    }

    pub async fn list_persisted_job_translation_stats(
        &self,
    ) -> Result<Vec<crate::infrastructure::local_db::LocalJobTranslationStats>> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("local task service was started without SQLite persistence"))?;
        database.list_job_translation_stats().await
    }

    pub async fn asr_run_is_protected(&self, run_id: &str) -> Result<bool> {
        self.database
            .as_ref()
            .ok_or_else(|| anyhow!("ASR history requires SQLite"))?
            .asr_run_is_protected(run_id)
            .await
    }

    pub async fn list_persisted_asr_runs(
        &self,
        job_id: &str,
    ) -> Result<Vec<crate::infrastructure::local_db::LocalAsrRunRecord>> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("ASR run history requires SQLite persistence"))?;
        database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        database.list_asr_runs(job_id).await
    }

    pub async fn list_persisted_asr_quality_signals(
        &self,
        job_id: &str,
    ) -> Result<Vec<crate::infrastructure::local_db::LocalAsrQualitySignalRecord>> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("ASR quality review requires SQLite persistence"))?;
        database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        database.list_asr_quality_signals(job_id).await
    }

    pub async fn record_asr_review_decision(
        &self,
        decision: &crate::application::ReviewDecision,
    ) -> Result<()> {
        self.database
            .as_ref()
            .ok_or_else(|| anyhow!("ASR quality review requires SQLite persistence"))?
            .record_asr_review_decision(decision)
            .await
    }

    pub async fn record_asr_repair_attempt(
        &self,
        attempt: &crate::application::RepairAttempt,
    ) -> Result<()> {
        self.database
            .as_ref()
            .ok_or_else(|| anyhow!("ASR repair history requires SQLite persistence"))?
            .record_asr_repair_attempt(attempt)
            .await
    }

    pub async fn rename_persisted_job(
        &self,
        job_id: &str,
        display_name: Option<String>,
    ) -> Result<crate::infrastructure::local_db::LocalJobRecord> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("local task service was started without SQLite persistence"))?;
        database.rename_job(job_id, display_name).await
    }

    pub async fn reorder_persisted_jobs(
        &self,
        job_ids: &[String],
    ) -> Result<Vec<crate::infrastructure::local_db::LocalJobRecord>> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("task ordering requires SQLite persistence"))?;
        database.reorder_jobs(job_ids).await
    }

    /// Reconnects a durable task to source media that the user moved after
    /// transcription. Derived audio and subtitle data stay in the task folder.
    pub async fn relink_persisted_job_input(
        &self,
        job_id: &str,
        input_path: impl AsRef<Path>,
    ) -> Result<crate::infrastructure::local_db::LocalJobRecord> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("media relinking requires SQLite persistence"))?;
        let input_path = input_path.as_ref().canonicalize().with_context(|| {
            format!(
                "failed to resolve replacement source media {}",
                input_path.as_ref().display()
            )
        })?;
        if !input_path.is_file() {
            bail!(
                "replacement source media is not a file: {}",
                input_path.display()
            );
        }
        let record = database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let job = Job::open(PathBuf::from(&record.storage_dir))?;
        let mut manifest = job
            .read_manifest_if_exists()?
            .ok_or_else(|| anyhow!("task status is missing: {}", job.status_json.display()))?;
        manifest.replace_input(input_path.clone());
        job.write_manifest(&manifest)?;
        database
            .update_job_input_path(
                job_id,
                &input_path,
                i64::try_from(manifest.updated_at_unix)
                    .context("replacement source media timestamp exceeds SQLite range")?,
            )
            .await?;
        database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task disappeared after media relinking: {job_id}"))
    }

    /// Converts non-terminal task state left by an earlier application session
    /// into an explicit, retryable failure. Automatically resuming an external
    /// ffmpeg/Whisper process is unsafe because its process state cannot be
    /// reconstructed after the desktop application exits.
    pub async fn recover_interrupted_jobs(&self) -> Result<usize> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("task recovery requires SQLite persistence"))?;
        let records = database.list_jobs().await?;
        let asr_error = "Atogaki 上次退出时该识别 run 尚未完成；已保留已有产物，可重新运行。";
        for record in &records {
            for run_record in database.list_asr_runs(&record.job_id).await? {
                if !matches!(run_record.status.as_str(), "queued" | "running") {
                    continue;
                }
                let file_result = (|| -> Result<()> {
                    let job = Job::open(PathBuf::from(&record.storage_dir))?;
                    let artifacts = AsrRunArtifacts::open(&job, &run_record.id)?;
                    let mut run = artifacts.read_run()?;
                    run.fail(asr_error);
                    artifacts.write_run(&run)
                })();
                if let Err(error) = file_result {
                    eprintln!(
                        "[task-service] cannot recover ASR run file {}: {error:#}",
                        run_record.id
                    );
                }
            }
        }
        database.recover_interrupted_asr_runs(asr_error).await?;

        let mut recovered = 0;
        for record in records {
            if matches!(record.status.as_str(), "done" | "failed") {
                continue;
            }
            let job = match Job::open(PathBuf::from(&record.storage_dir)) {
                Ok(job) => job,
                Err(error) => {
                    eprintln!(
                        "[task-service] cannot recover missing task directory {}: {error:#}",
                        record.storage_dir
                    );
                    database
                        .mark_job_failed(
                            &record.job_id,
                            "Atogaki 上次退出时任务尚未完成，且任务目录已不存在，无法重试。",
                        )
                        .await?;
                    recovered += 1;
                    continue;
                }
            };
            let mut manifest = match job.read_manifest_if_exists() {
                Ok(Some(manifest)) => manifest,
                Ok(None) => {
                    database
                        .mark_job_failed(
                            &record.job_id,
                            "Atogaki 上次退出时任务尚未完成，但任务状态文件缺失，旧目录已保留。",
                        )
                        .await?;
                    recovered += 1;
                    continue;
                }
                Err(error) => {
                    eprintln!(
                        "[task-service] cannot read task state {}: {error:#}",
                        job.status_json.display()
                    );
                    database
                        .mark_job_failed(
                            &record.job_id,
                            "Atogaki 上次退出时任务尚未完成，但任务状态文件已损坏，旧目录已保留。",
                        )
                        .await?;
                    recovered += 1;
                    continue;
                }
            };
            if matches!(manifest.status, JobStatus::Done | JobStatus::Failed) {
                database
                    .sync_snapshot(&JobSnapshot {
                        manifest,
                        segments: job.read_segments().unwrap_or_default(),
                    })
                    .await?;
                continue;
            }
            let last_observed = fs::read(job.dir.join("execution-heartbeat.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<u64>(&bytes).ok())
                .unwrap_or(manifest.updated_at_unix);
            manifest.fail_interrupted(
                "Atogaki 上次退出时任务尚未完成。已保留检查点；支持断点的任务可点击“继续”，历史任务可重新运行。",
                last_observed,
            );
            if let Err(error) = job.write_manifest(&manifest) {
                eprintln!(
                    "[task-service] cannot persist recovered task state {}: {error:#}",
                    job.status_json.display()
                );
                database
                    .mark_job_failed(
                        &record.job_id,
                        "Atogaki 上次退出时任务尚未完成，且无法更新任务状态文件；旧目录已保留。",
                    )
                    .await?;
                recovered += 1;
                continue;
            }
            database
                .sync_snapshot(&JobSnapshot {
                    manifest,
                    segments: job.read_segments().unwrap_or_default(),
                })
                .await?;
            recovered += 1;
        }
        Ok(recovered)
    }

    /// Creates a new transcription task from a failed task's immutable input,
    /// recognition options, and glossary snapshot. The failed directory remains
    /// untouched for diagnosis and any partial artifacts are never overwritten.
    pub async fn retry_persisted_job(
        &self,
        job_id: &str,
        whisper_model_override: Option<PathBuf>,
        vad_model_override: Option<PathBuf>,
    ) -> Result<JobSnapshot> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("task retry requires SQLite persistence"))?;
        let previous = database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        if previous.status != "failed" {
            bail!(
                "only failed tasks can be retried; task {job_id} is {}",
                previous.status
            );
        }
        let previous_job = Job::open(PathBuf::from(&previous.storage_dir))?;
        let manifest = previous_job
            .read_manifest_if_exists()?
            .ok_or_else(|| anyhow!("failed task is missing status.json"))?;
        let languages = LanguagePair {
            source: manifest.source_language,
            target: manifest.target_language,
        };
        let input = manifest
            .input
            .ok_or_else(|| anyhow!("failed task does not record its source media"))?;
        if !input.is_file() {
            bail!("source media no longer exists: {}", input.display());
        }
        let mut transcription = previous_job.read_recognition_options()?;
        if !transcription.whisper.model.is_file() {
            transcription.whisper.model = whisper_model_override
                .filter(|path| path.is_file())
                .ok_or_else(|| {
                    anyhow!(
                        "Whisper model no longer exists: {}; configure an available model before retrying",
                        transcription.whisper.model.display()
                    )
                })?;
        }
        if let Some(missing_vad_model) = transcription
            .whisper
            .vad_model
            .as_ref()
            .filter(|path| !path.is_file())
        {
            transcription.whisper.vad_model = Some(
                vad_model_override
                    .filter(|path| path.is_file())
                    .ok_or_else(|| {
                        anyhow!(
                            "VAD model no longer exists: {}; configure an available VAD model before retrying",
                            missing_vad_model.display()
                        )
                    })?,
            );
        }

        let job = self
            .create_queued_job(Some(input.clone()), None, languages)
            .await?;
        if let Some(previous_glossary) = transcription
            .glossary
            .as_deref()
            .filter(|path| path.is_file())
        {
            let snapshot_path = job.dir.join("recognition-glossary.txt");
            fs::copy(previous_glossary, &snapshot_path).with_context(|| {
                format!(
                    "failed to copy recognition glossary snapshot {}",
                    previous_glossary.display()
                )
            })?;
            transcription.glossary = Some(snapshot_path.clone());
            if let (Some(glossary_id), Some(glossary_name)) = (
                previous.glossary_id.as_deref(),
                previous.glossary_name.as_deref(),
            ) && database.get_glossary(glossary_id).await?.is_some()
            {
                database
                    .assign_job_glossary(
                        job.id().as_str(),
                        glossary_id,
                        glossary_name,
                        &snapshot_path,
                    )
                    .await?;
            }
        }
        let prompt = crate::domain::glossary::build_whisper_prompt(&transcription)?;
        job.write_whisper_prompt(prompt.as_deref())?;
        job.write_recognition_options(&transcription)?;
        let job_dir = job.dir.clone();
        let snapshot = self
            .enqueue(
                job,
                QueuedTask::Transcribe {
                    job_dir: job_dir.clone(),
                    spec: TranscribeSpec {
                        input,
                        output_dir: Some(job_dir),
                        transcription,
                    },
                },
            )
            .await?;
        Ok(snapshot)
    }

    pub async fn delete_persisted_job(&self, job_id: &str) -> Result<()> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("local task service was started without SQLite persistence"))?;
        let record = database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        if !matches!(record.status.as_str(), "done" | "failed") {
            bail!(
                "cannot delete task {job_id} while its status is {}; wait for it to finish",
                record.status
            );
        }

        let jobs_root = fs::canonicalize(&self.jobs_dir).with_context(|| {
            format!(
                "failed to resolve local jobs directory {}",
                self.jobs_dir.display()
            )
        })?;
        let storage_dir = PathBuf::from(&record.storage_dir);
        if storage_dir.file_name().and_then(|name| name.to_str()) != Some(job_id) {
            bail!(
                "refusing to delete task directory with unexpected name: {}",
                storage_dir.display()
            );
        }
        let storage_parent = storage_dir
            .parent()
            .ok_or_else(|| anyhow!("task directory has no parent: {}", storage_dir.display()))?;
        let storage_parent = fs::canonicalize(storage_parent).with_context(|| {
            format!(
                "failed to resolve task directory parent {}",
                storage_parent.display()
            )
        })?;
        if storage_parent != jobs_root {
            bail!(
                "refusing to delete task directory outside managed jobs root: {}",
                storage_dir.display()
            );
        }

        if !storage_dir.exists() {
            return database.delete_job(job_id).await;
        }
        let resolved_storage = fs::canonicalize(&storage_dir).with_context(|| {
            format!("failed to resolve task directory {}", storage_dir.display())
        })?;
        if resolved_storage.parent() != Some(jobs_root.as_path()) {
            bail!(
                "refusing to delete resolved task directory outside managed jobs root: {}",
                resolved_storage.display()
            );
        }

        let tombstone = jobs_root.join(format!(".deleting-{job_id}-{}", uuid::Uuid::new_v4()));
        fs::rename(&resolved_storage, &tombstone).with_context(|| {
            format!(
                "failed to prepare task directory for deletion: {}",
                resolved_storage.display()
            )
        })?;
        if let Err(error) = database.delete_job(job_id).await {
            let _ = fs::rename(&tombstone, &resolved_storage);
            return Err(error.context("failed to delete task metadata; restored task directory"));
        }
        fs::remove_dir_all(&tombstone).with_context(|| {
            format!(
                "task metadata was deleted but directory cleanup failed: {}",
                tombstone.display()
            )
        })
    }

    fn require_service_owned_output_dir(&self, output_dir: Option<&Path>) -> Result<()> {
        if let Some(output_dir) = output_dir {
            bail!(
                "local task service owns task directories; received explicit output directory: {}",
                output_dir.display()
            );
        }
        Ok(())
    }

    async fn create_queued_job(
        &self,
        input: Option<PathBuf>,
        render_output: Option<PathBuf>,
        languages: LanguagePair,
    ) -> Result<Job> {
        let job = Job::create_in(&self.jobs_dir)?;
        let mut manifest = JobManifest::new(&job, input, render_output, languages);
        manifest.mark(JobStatus::Queued);
        job.write_manifest(&manifest)?;
        if let Some(database) = &self.database {
            database
                .sync_snapshot(&JobSnapshot {
                    manifest,
                    segments: Vec::new(),
                })
                .await?;
        }
        Ok(job)
    }

    pub async fn recovery(
        &self,
        job_id: &str,
    ) -> Result<crate::application::acoustic_plan::TaskRecoverySummary> {
        let db = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("recovery requires SQLite"))?;
        let record = db
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        let job = Job::open(PathBuf::from(record.storage_dir))?;
        let mut summary = crate::application::acoustic_plan::recovery_summary(&job)?;
        summary.can_resume &= record.status == "failed";
        if record.status != "failed" {
            summary.phase = record.status;
        }
        Ok(summary)
    }

    pub async fn pause(&self, job_id: &str) -> Result<()> {
        let controls = self.controls.lock().await;
        let control = controls
            .get(job_id)
            .ok_or_else(|| anyhow!("task is not running or queued"))?;
        control.sender.send_replace(true);
        let mut finished = control.finished.subscribe();
        let started = control.started;
        if !started {
            let db = self
                .database
                .as_ref()
                .ok_or_else(|| anyhow!("pause requires SQLite"))?;
            let record = db
                .get_job(job_id)
                .await?
                .ok_or_else(|| anyhow!("task not found"))?;
            let job = Job::open(record.storage_dir.into())?;
            let mut manifest = job
                .read_manifest_if_exists()?
                .ok_or_else(|| anyhow!("task manifest missing"))?;
            manifest.fail("已暂停（等待处理）；可继续原排队项");
            job.write_manifest(&manifest)?;
            db.sync_snapshot(&JobSnapshot::load(&job.dir)?).await?;
        }
        drop(controls);
        if started && !*finished.borrow() {
            let _ = finished.changed().await;
        }
        Ok(())
    }

    pub async fn resume(&self, job_id: &str) -> Result<JobSnapshot> {
        self.resume_with_models(job_id, None, None).await
    }
    pub async fn resume_with_models(
        &self,
        job_id: &str,
        whisper_override: Option<PathBuf>,
        vad_override: Option<PathBuf>,
    ) -> Result<JobSnapshot> {
        let db = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("resume requires SQLite"))?;
        let record = db
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("task not found"))?;
        if record.status != "failed" {
            bail!("only stopped tasks can resume")
        }
        {
            let mut controls = self.controls.lock().await;
            if let Some(control) = controls.get_mut(job_id) {
                let paused = *control.sender.borrow();
                if !control.started && paused {
                    let job = Job::open(PathBuf::from(&record.storage_dir))?;
                    let mut manifest = job
                        .read_manifest_if_exists()?
                        .ok_or_else(|| anyhow!("task manifest missing"))?;
                    manifest.mark(JobStatus::Queued);
                    job.write_manifest(&manifest)?;
                    db.sync_snapshot(&JobSnapshot::load(&job.dir)?).await?;
                    control.sender.send_replace(false);
                    return JobSnapshot::load(&job.dir);
                }
                bail!("task already queued or running")
            }
        }
        let job = Job::open(PathBuf::from(&record.storage_dir))?;
        if !crate::application::acoustic_plan::recovery_summary(&job)?.can_resume {
            bail!("该历史任务无法断点继续；可重新运行")
        }
        let mut manifest = job
            .read_manifest_if_exists()?
            .ok_or_else(|| anyhow!("task manifest missing"))?;
        let mut options = job.read_recognition_options()?;
        let asr_pending = !job.dir.join("asr-published.json").is_file();
        let mut relocated = false;
        if asr_pending && !options.whisper.model.exists() {
            relocated = true;
            options.whisper.model = whisper_override
                .filter(|p| p.is_file())
                .ok_or_else(|| anyhow!("原识别模型缺失；请在设置中重新定位相同模型"))?;
        }
        if asr_pending
            && options
                .whisper
                .vad_model
                .as_ref()
                .is_some_and(|p| !p.is_file())
        {
            relocated = true;
            options.whisper.vad_model = Some(
                vad_override
                    .filter(|p| p.is_file())
                    .ok_or_else(|| anyhow!("原 VAD 模型缺失；请在设置中重新定位相同模型"))?,
            );
        }
        let plan_path = job.dir.join("asr-checkpoint/plan.json");
        if relocated && plan_path.is_file() {
            let plan: crate::application::acoustic_plan::AcousticPlan =
                serde_json::from_slice(&fs::read(plan_path)?)?;
            if crate::application::acoustic_plan::configuration_digest(&options, "whisper.cpp")
                .await?
                != plan.configuration_sha256
            {
                bail!("重新定位的模型内容与检查点不一致；原配置未改变，请选择相同模型")
            }
        }
        job.write_recognition_options(&options)?;
        let spec = TranscribeSpec {
            input: manifest
                .input
                .clone()
                .unwrap_or_else(|| job.audio_wav.clone()),
            output_dir: Some(job.dir.clone()),
            transcription: options,
        };
        // Reserve this task before enqueueing: simultaneous clicks cannot dispatch twice.
        {
            let mut controls = self.controls.lock().await;
            if controls.contains_key(job_id) {
                bail!("task already queued or running")
            };
            controls.insert(job_id.into(), TaskControl::new());
        }
        let result = async {
            manifest.mark(JobStatus::Queued);
            job.write_manifest(&manifest)?;
            db.sync_snapshot(&JobSnapshot::load(&job.dir)?).await?;
            self.enqueue(
                job.clone(),
                QueuedTask::Transcribe {
                    job_dir: job.dir.clone(),
                    spec,
                },
            )
            .await
        }
        .await;
        if result.is_err() {
            self.controls.lock().await.remove(job_id);
        }
        result
    }

    async fn enqueue(&self, job: Job, task: QueuedTask) -> Result<JobSnapshot> {
        crate::application::acoustic_plan::write_json(&job.dir.join("workflow-version.json"), &1)?;
        self.controls
            .lock()
            .await
            .entry(job.id())
            .or_insert_with(TaskControl::new);
        if self.sender.send(task).await.is_err() {
            let mut manifest = job
                .read_manifest_if_exists()?
                .unwrap_or_else(|| JobManifest::new(&job, None, None, LanguagePair::default()));
            manifest.fail("local task service stopped before the task could start");
            let _ = job.write_manifest(&manifest);
            return Err(anyhow!("local task service is not running"));
        }

        JobSnapshot::load(&job.dir)
    }
}

async fn run_worker(
    config: AppConfig,
    receiver: Arc<Mutex<mpsc::Receiver<QueuedTask>>>,
    database: Option<LocalDatabase>,
    controls: Arc<Mutex<HashMap<String, TaskControl>>>,
    automatic: Arc<Mutex<HashMap<String, crate::application::LocalWorkspaceService>>>,
    scheduler: Arc<crate::application::stage_scheduler::StageScheduler>,
) {
    loop {
        let task = {
            let mut receiver = receiver.lock().await;
            receiver.recv().await
        };

        let Some(task) = task else {
            return;
        };

        let config = config.clone();
        let database = database.clone();
        let controls = controls.clone();
        let automatic = automatic.clone();
        let scheduler = scheduler.clone();
        tokio::spawn(async move {
            run_queued_task(config, task, database, controls, automatic, scheduler).await;
        });
    }
}

async fn run_queued_task(
    config: AppConfig,
    task: QueuedTask,
    database: Option<LocalDatabase>,
    controls: Arc<Mutex<HashMap<String, TaskControl>>>,
    automatic: Arc<Mutex<HashMap<String, crate::application::LocalWorkspaceService>>>,
    scheduler: Arc<crate::application::stage_scheduler::StageScheduler>,
) {
    let mut runner = JobRunner::new(config);
    if let Some(database) = &database {
        runner = runner.with_database(database.clone());
    }
    let job_dir = match &task {
        QueuedTask::Transcribe { job_dir, .. } | QueuedTask::Process { job_dir, .. } => {
            job_dir.clone()
        }
    };
    let id = job_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut pause = {
        let mut entries = controls.lock().await;
        let Some(control) = entries.get_mut(&id) else {
            return;
        };
        control.sender.subscribe()
    };
    if !job_dir.is_dir() {
        controls.lock().await.remove(&id);
        automatic.lock().await.remove(&id);
        return;
    }
    let asr_pending = !job_dir.join("asr-published.json").is_file();
    let mut asr_permit = loop {
        while *pause.borrow() {
            if pause.changed().await.is_err() {
                return;
            }
        }
        let permit = if asr_pending {
            tokio::select! {
                permit = scheduler.acquire_asr() => Some(permit),
                _ = pause.changed() => continue,
            }
        } else {
            None
        };
        let mut entries = controls.lock().await;
        let Some(control) = entries.get_mut(&id) else {
            return;
        };
        if *control.sender.borrow() {
            drop(permit);
            continue;
        }
        control.started = true;
        break permit;
    };
    let (progress_tx, progress_rx) = watch::channel(0);
    runner = runner.with_asr_progress(progress_tx);
    let automatic_workspace = automatic.lock().await.get(&id).cloned();
    let work = async {
        let job = Job::open(job_dir.clone())?;
        let early = automatic_workspace
            .as_ref()
            .map(|workspace| {
                crate::application::early_translation::EarlyTranslationSession::new(
                    job.clone(),
                    workspace.frozen_translation_provider(),
                )
            })
            .transpose()?
            .flatten();
        let mut early_error = None;
        if !job.dir.join("asr-published.json").is_file() {
            let (done_tx, done_rx) = watch::channel(false);
            let asr = async {
                let mut result = match task {
                    QueuedTask::Transcribe { spec, .. } => {
                        runner.transcribe(spec).await.map(|_| ())
                    }
                    QueuedTask::Process { spec, .. } => runner.process(spec).await.map(|_| ()),
                };
                drop(asr_permit.take());
                if result.is_ok() && automatic_workspace.is_some() {
                    result = async {
                        let mut manifest = job
                            .read_manifest_if_exists()?
                            .ok_or_else(|| anyhow!("task manifest missing"))?;
                        manifest.mark(JobStatus::Translating);
                        job.write_manifest(&manifest)?;
                        Ok(())
                    }
                    .await;
                }
                done_tx.send_replace(true);
                result
            };
            let prefix = async {
                if let Some(session) = &early {
                    session.run(progress_rx, done_rx).await
                } else {
                    Ok(())
                }
            };
            let (asr_result, prefix_result) = tokio::join!(asr, prefix);
            asr_result?;
            early_error = prefix_result.err();
            if let Some(db) = &database {
                db.sync_snapshot(&JobSnapshot::load(&job_dir)?).await?;
                sync_initial_asr_provenance(db, &job_dir).await?;
            }
            crate::application::acoustic_plan::write_json(
                &job.dir.join("asr-published.json"),
                &true,
            )?;
        }
        if let Some(workspace) = automatic_workspace {
            let mut manifest = job
                .read_manifest_if_exists()?
                .ok_or_else(|| anyhow!("task manifest missing"))?;
            manifest.mark(JobStatus::Translating);
            job.write_manifest(&manifest)?;
            if let Some(error) = early_error {
                return Err(error);
            }
            let workspace = if let (Some(early), Some(db)) = (&early, &database) {
                workspace.using_provider(early.final_provider(db.clone()))
            } else {
                workspace
            };
            workspace.translate_pending(&id).await?;
            manifest.mark(JobStatus::Done);
            job.write_manifest(&manifest)?;
        }
        Ok::<(), anyhow::Error>(())
    };
    let result=run_with_status_sync(&job_dir,database.as_ref(),async {
            if *pause.borrow() {return Err(anyhow!("已暂停；可从检查点继续"))}
            tokio::select! { result=work => result, _=pause.changed()=>Err(anyhow!("已暂停；可从检查点继续")) }
        }).await;
    if let Err(error) = &result {
        if let (Some(db), Ok(job)) = (&database, Job::open(job_dir.clone())) {
            if let Ok(runs) = db.list_asr_runs(&id).await {
                for record in runs
                    .into_iter()
                    .filter(|r| r.status == "running" || r.status == "queued")
                {
                    if let Ok(artifacts) = AsrRunArtifacts::open(&job, &record.id) {
                        if let Ok(mut run) = artifacts.read_run() {
                            run.fail(format!("{error:#}"));
                            let _ = artifacts.write_run(&run);
                            let _ = db.update_asr_run(&run).await;
                        }
                    }
                }
            }
        }
        if let Ok(job) = Job::open(job_dir.clone()) {
            if let Ok(Some(mut manifest)) = job.read_manifest_if_exists() {
                manifest.fail(format!("{error:#}"));
                let _ = job.write_manifest(&manifest);
            }
        }
    }

    if let Err(error) = result {
        eprintln!("[task-service] task failed: {error:#}");
    }

    if let Some(database) = &database {
        match JobSnapshot::load(&job_dir) {
            Ok(snapshot) => {
                if let Err(error) = database.sync_snapshot(&snapshot).await {
                    eprintln!("[task-service] failed to sync local database: {error:#}");
                } else if let Err(error) = sync_initial_asr_provenance(database, &job_dir).await {
                    eprintln!("[task-service] failed to sync ASR cue provenance: {error:#}");
                }
            }
            Err(error) => eprintln!("[task-service] failed to load task state: {error:#}"),
        }
    }
    automatic.lock().await.remove(&id);
    if let Some(control) = controls.lock().await.remove(&id) {
        control.finished.send_replace(true);
    }
}

async fn sync_initial_asr_provenance(database: &LocalDatabase, job_dir: &Path) -> Result<()> {
    let job = Job::open(job_dir.to_path_buf())?;
    let Some(run) = database
        .list_asr_runs(&job.id())
        .await?
        .into_iter()
        .find(|run| run.run_kind == "full" && run.status == "succeeded")
    else {
        return Ok(());
    };
    let artifacts = AsrRunArtifacts::open(&job, &run.id)?;
    let candidates = artifacts.read_candidate_cues()?;
    database
        .assign_asr_cue_provenance(&job.id(), &run.id, &candidates)
        .await
}

async fn run_with_status_sync<T>(
    job_dir: &Path,
    database: Option<&LocalDatabase>,
    task: impl Future<Output = Result<T>>,
) -> Result<T> {
    let Some(database) = database.cloned() else {
        return task.await;
    };
    let path = job_dir.to_path_buf();
    // A progress query must never suspend the work future while it owns a
    // SQLite transaction. Run polling independently, then join its cancellation
    // before publishing the terminal snapshot.
    let progress = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut ticks = 0_u64;
        loop {
            interval.tick().await;
            if ticks % 10 == 0 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // Separate evidence avoids overwriting a concurrent phase transition.
                let _ = crate::application::acoustic_plan::write_json(
                    &path.join("execution-heartbeat.json"),
                    &now,
                );
            }
            ticks += 1;
            if let Ok(snapshot) = JobSnapshot::load(&path) {
                if let Err(error) = database.sync_snapshot(&snapshot).await {
                    eprintln!("[task-service] failed to publish running task status: {error:#}");
                }
            }
        }
    });
    let result = task.await;
    progress.abort();
    let _ = progress.await;
    result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::LocalTaskService;
    use crate::{
        application::{TranscriptionOptions, job_spec::TranscribeSpec},
        domain::LanguageCode,
        infrastructure::{
            config::AppConfig,
            local_db::{LocalDatabase, LocalGlossaryTermInput},
        },
    };
    use tokio::sync::mpsc;

    #[test]
    fn rejects_an_invalid_worker_or_queue_count_before_starting() {
        let config = AppConfig {
            ffmpeg: "ffmpeg".into(),
            whisper_cli: "whisper-cli".into(),
            deepl_auth_key: None,
        };

        assert!(LocalTaskService::with_workers(config.clone(), "jobs", 0, 1).is_err());
        assert!(LocalTaskService::with_workers(config, "jobs", 1, 0).is_err());
    }

    #[tokio::test]
    async fn translation_resume_does_not_require_removed_asr_models_or_rerun_asr() {
        use crate::application::{
            JobSnapshot, TranslationFuture, TranslationProvider, TranslationProviderStatus,
            TranslationRequest, TranslationResponse, TranslationResult, TranslationUsage,
            job_manifest::JobManifest, job_status::JobStatus,
        };
        use std::sync::{Arc, Mutex};
        #[derive(Debug)]
        struct Provider(Arc<Mutex<Vec<String>>>);
        impl TranslationProvider for Provider {
            fn status(&self) -> TranslationProviderStatus {
                TranslationProviderStatus {
                    id: "fixture".into(),
                    name: "Fixture".into(),
                    configured: true,
                    model: None,
                    endpoint_kind: "test".into(),
                    configuration_hint: None,
                }
            }
            fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
                Box::pin(async move {
                    self.0.lock().unwrap().extend(
                        request
                            .targets
                            .iter()
                            .map(|target| target.segment_id.clone()),
                    );
                    Ok(TranslationResponse {
                        translations: request
                            .targets
                            .into_iter()
                            .map(|target| TranslationResult {
                                segment_id: target.segment_id,
                                translated_text: "恢复后的译文".into(),
                            })
                            .collect(),
                        model: None,
                        usage: TranslationUsage::default(),
                    })
                })
            }
        }
        let root = std::env::temp_dir().join(format!(
            "atogaki-translation-resume-{}",
            uuid::Uuid::new_v4()
        ));
        let job = crate::infrastructure::job_store::Job::create_in(&root.join("jobs")).unwrap();
        let db = LocalDatabase::open(root.join("state.sqlite"))
            .await
            .unwrap();
        let mut manifest = JobManifest::new(
            &job,
            Some(root.join("removed-media")),
            None,
            crate::domain::LanguagePair::default(),
        );
        manifest.mark(JobStatus::Translating);
        manifest.fail("injected translation interruption");
        job.write_manifest(&manifest).unwrap();
        let mut options = TranscriptionOptions::japanese(root.join("removed-model"));
        options.whisper.vad_model = Some(root.join("removed-vad"));
        job.write_recognition_options(&options).unwrap();
        crate::application::acoustic_plan::write_json(&job.dir.join("workflow-version.json"), &1)
            .unwrap();
        crate::application::acoustic_plan::write_json(&job.dir.join("asr-published.json"), &true)
            .unwrap();
        assert_eq!(
            crate::application::acoustic_plan::recovery_summary(&job)
                .unwrap()
                .phase,
            "translating"
        );
        let mut completed = crate::domain::TranscriptSegment::new(0, 1000, "最初の文。".into());
        completed.set_translation(Some("保留的译文".into()));
        let pending = crate::domain::TranscriptSegment::new(20000, 21000, "次の文。".into());
        job.write_segments(&[completed.clone(), pending.clone()])
            .unwrap();
        db.sync_snapshot(&JobSnapshot::load(&job.dir).unwrap())
            .await
            .unwrap();
        crate::application::LocalWorkspaceService::new(db.clone())
            .update_subtitle(
                &job.id(),
                &completed.id,
                completed.source_text.clone(),
                Some("人工保留译文".into()),
                0,
                1000,
            )
            .await
            .unwrap();
        let service = LocalTaskService::start_with_database(
            AppConfig {
                ffmpeg: "must-not-run-ffmpeg".into(),
                whisper_cli: "must-not-run-asr".into(),
                deepl_auth_key: None,
            },
            root.join("jobs"),
            db.clone(),
        )
        .unwrap();
        let targets = Arc::new(Mutex::new(Vec::new()));
        service
            .configure_task_translation(&job.id(), Arc::new(Provider(targets.clone())))
            .await
            .unwrap();
        service.resume(&job.id()).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let status = db.get_job(&job.id()).await.unwrap().unwrap().status;
                if status == "done" {
                    break;
                }
                assert_ne!(status, "failed");
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(*targets.lock().unwrap(), vec![pending.id.clone()]);
        let segments = db.list_segments(&job.id()).await.unwrap();
        assert_eq!(segments[0].id, completed.id);
        assert_eq!(segments[0].translated_text.as_deref(), Some("人工保留译文"));
        assert_eq!(segments[1].translated_text.as_deref(), Some("恢复后的译文"));
        assert!(db.list_asr_runs(&job.id()).await.unwrap().is_empty());
        drop(service);
        db.close().await;
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn persistent_queue_writes_a_queued_snapshot_before_dispatch() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-task-service-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let (sender, _receiver) = mpsc::channel(1);
        let service = LocalTaskService {
            scheduler: crate::application::stage_scheduler::StageScheduler::new(2),
            automatic: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            controls: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            sender,
            jobs_dir: root.join("jobs"),
            database: Some(database.clone()),
        };

        let job = service
            .create_queued_job(None, None, crate::domain::LanguagePair::default())
            .await
            .unwrap();
        let jobs = database.list_jobs().await.unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, job.id());
        assert_eq!(jobs[0].status, "queued");

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn relinks_moved_source_media_without_replacing_task_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-task-media-relink-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let (sender, _receiver) = mpsc::channel(1);
        let service = LocalTaskService {
            scheduler: crate::application::stage_scheduler::StageScheduler::new(2),
            automatic: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            controls: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            sender,
            jobs_dir: root.join("jobs"),
            database: Some(database.clone()),
        };
        let original = root.join("original.mp4");
        let relocated = root.join("relocated.mp4");
        fs::write(&original, b"original media placeholder").unwrap();
        fs::write(&relocated, b"relocated media placeholder").unwrap();
        let job = service
            .create_queued_job(Some(original), None, crate::domain::LanguagePair::default())
            .await
            .unwrap();
        fs::write(&job.audio_wav, b"derived audio").unwrap();

        let updated = service
            .relink_persisted_job_input(&job.id(), &relocated)
            .await
            .unwrap();
        let manifest = job.read_manifest_if_exists().unwrap().unwrap();
        let relocated = relocated.canonicalize().unwrap();

        assert_eq!(updated.input_path.as_deref(), relocated.to_str());
        assert_eq!(manifest.input.as_deref(), Some(relocated.as_path()));
        assert_eq!(fs::read(&job.audio_wav).unwrap(), b"derived audio");

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn selected_glossary_is_snapshotted_and_associated_before_dispatch() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-task-glossary-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let glossary = database
            .save_glossary(
                None,
                "测试词表".to_string(),
                "ja",
                vec![LocalGlossaryTermInput {
                    source_text: "ナブナ".to_string(),
                    target_text: Some("n-buna".to_string()),
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        let (sender, _receiver) = mpsc::channel(1);
        let service = LocalTaskService {
            scheduler: crate::application::stage_scheduler::StageScheduler::new(2),
            automatic: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            controls: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            sender,
            jobs_dir: root.join("jobs"),
            database: Some(database.clone()),
        };

        let vad_model = root.join("ggml-silero.bin");
        let mut transcription = TranscriptionOptions::japanese(root.join("model.bin"));
        transcription.whisper.vad_model = Some(vad_model.clone());
        let snapshot = service
            .submit_transcription_with_glossary(
                TranscribeSpec {
                    input: root.join("input.mp3"),
                    output_dir: None,
                    transcription,
                },
                Some(&glossary.glossary.id),
                &[],
            )
            .await
            .unwrap();
        let record = database
            .get_job(&snapshot.manifest.job_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.glossary_id, Some(glossary.glossary.id));
        assert_eq!(record.glossary_name.as_deref(), Some("测试词表"));
        let glossary_path = record.glossary_snapshot_path.unwrap();
        assert!(
            fs::read_to_string(glossary_path)
                .unwrap()
                .contains("ナブナ => n-buna")
        );
        let persisted_options: TranscriptionOptions = serde_json::from_slice(
            &fs::read(
                root.join("jobs")
                    .join(&snapshot.manifest.job_id)
                    .join("recognition-options.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            persisted_options.whisper.vad_model.as_deref(),
            Some(vad_model.as_path())
        );
        assert_eq!(persisted_options.whisper.vad_max_speech_s, 8);
        let prompt = fs::read_to_string(
            root.join("jobs")
                .join(&snapshot.manifest.job_id)
                .join("whisper-prompt.txt"),
        )
        .unwrap();
        assert!(prompt.contains("n-buna"));
        assert!(!prompt.contains("ナブナ"));
        assert!(!prompt.contains("表記:"));

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn mismatched_glossary_language_is_rejected_without_creating_a_task() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-task-glossary-language-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let glossary = database
            .save_glossary(None, "日语词表".to_string(), "ja", vec![])
            .await
            .unwrap();
        let (sender, _receiver) = mpsc::channel(1);
        let jobs_dir = root.join("jobs");
        let service = LocalTaskService {
            scheduler: crate::application::stage_scheduler::StageScheduler::new(2),
            automatic: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            controls: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            sender,
            jobs_dir: jobs_dir.clone(),
            database: Some(database.clone()),
        };

        let error = service
            .submit_transcription_with_glossary(
                TranscribeSpec {
                    input: root.join("english.mp3"),
                    output_dir: None,
                    transcription: TranscriptionOptions::new(
                        root.join("model.bin"),
                        LanguageCode::English,
                    ),
                },
                Some(&glossary.glossary.id),
                &[],
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("does not match"));
        assert!(database.list_jobs().await.unwrap().is_empty());
        assert!(!jobs_dir.exists());

        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn interrupted_task_is_failed_and_retry_creates_a_new_preserved_job() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-task-recovery-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let (sender, _receiver) = mpsc::channel(2);
        let service = LocalTaskService {
            scheduler: crate::application::stage_scheduler::StageScheduler::new(2),
            automatic: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            controls: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            sender,
            jobs_dir: root.join("jobs"),
            database: Some(database.clone()),
        };
        let input = root.join("input.mp4");
        let model = root.join("ggml-small.bin");
        fs::write(&input, b"media placeholder").unwrap();
        fs::write(&model, b"model placeholder").unwrap();
        let job = service
            .create_queued_job(
                Some(input.clone()),
                None,
                crate::domain::LanguagePair::default(),
            )
            .await
            .unwrap();
        job.write_recognition_options(&TranscriptionOptions::japanese(model.clone()))
            .unwrap();
        let corrupt_job = service
            .create_queued_job(None, None, crate::domain::LanguagePair::default())
            .await
            .unwrap();
        fs::write(&corrupt_job.status_json, b"not valid json").unwrap();

        assert_eq!(service.recover_interrupted_jobs().await.unwrap(), 2);
        let interrupted = database.get_job(&job.id()).await.unwrap().unwrap();
        assert_eq!(interrupted.status, "failed");
        assert!(interrupted.error_message.unwrap().contains("上次退出"));
        let corrupt = database.get_job(&corrupt_job.id()).await.unwrap().unwrap();
        assert_eq!(corrupt.status, "failed");
        assert!(corrupt.error_message.unwrap().contains("状态文件已损坏"));

        let retried = service
            .retry_persisted_job(&job.id(), None, None)
            .await
            .unwrap();
        assert_ne!(retried.manifest.job_id, job.id());
        assert_eq!(retried.manifest.status.as_str(), "queued");
        assert_eq!(retried.manifest.input.as_deref(), Some(input.as_path()));
        let retried_job = crate::infrastructure::job_store::Job::open(
            root.join("jobs").join(&retried.manifest.job_id),
        )
        .unwrap();
        assert_eq!(
            retried_job
                .read_recognition_options()
                .unwrap()
                .whisper
                .model,
            model
        );
        assert!(job.dir.is_dir());

        let replacement_model = root.join("ggml-medium.bin");
        fs::write(&replacement_model, b"replacement model placeholder").unwrap();
        fs::remove_file(&model).unwrap();
        let retried_again = service
            .retry_persisted_job(&job.id(), Some(replacement_model.clone()), None)
            .await
            .unwrap();
        let retried_again_job = crate::infrastructure::job_store::Job::open(
            root.join("jobs").join(&retried_again.manifest.job_id),
        )
        .unwrap();
        assert_eq!(
            retried_again_job
                .read_recognition_options()
                .unwrap()
                .whisper
                .model,
            replacement_model
        );

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn renames_finished_tasks_and_deletes_only_managed_task_data() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-task-management-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let (sender, _receiver) = mpsc::channel(1);
        let service = LocalTaskService {
            scheduler: crate::application::stage_scheduler::StageScheduler::new(2),
            automatic: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            controls: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            sender,
            jobs_dir: root.join("jobs"),
            database: Some(database.clone()),
        };
        let source_media = root.join("source-media.mp3");
        fs::write(&source_media, b"source media must survive task deletion").unwrap();
        let job = service
            .create_queued_job(
                Some(source_media.clone()),
                None,
                crate::domain::LanguagePair::default(),
            )
            .await
            .unwrap();
        let job_id = job.id();

        let renamed = service
            .rename_persisted_job(&job_id, Some("深夜电台 第 12 回".to_string()))
            .await
            .unwrap();
        assert_eq!(renamed.display_name.as_deref(), Some("深夜电台 第 12 回"));
        assert!(service.delete_persisted_job(&job_id).await.is_err());
        assert!(job.dir.is_dir());

        let mut manifest = job.read_manifest_if_exists().unwrap().unwrap();
        manifest.mark(crate::application::job_status::JobStatus::Done);
        job.write_manifest(&manifest).unwrap();
        database
            .sync_snapshot(&crate::application::job_snapshot::JobSnapshot {
                manifest: manifest.clone(),
                segments: Vec::new(),
            })
            .await
            .unwrap();
        let completed_at = manifest.completed_at_unix.map(|value| value as i64);
        let renamed_after_completion = service
            .rename_persisted_job(&job_id, Some("已完成的任务".to_string()))
            .await
            .unwrap();
        assert_eq!(renamed_after_completion.completed_at_unix, completed_at);
        service.delete_persisted_job(&job_id).await.unwrap();
        assert!(!job.dir.exists());
        assert!(source_media.is_file());
        assert!(database.get_job(&job_id).await.unwrap().is_none());

        let unmanaged =
            crate::infrastructure::job_store::Job::create_in(&root.join("foreign")).unwrap();
        let mut unmanaged_manifest = crate::application::job_manifest::JobManifest::new(
            &unmanaged,
            None,
            None,
            crate::domain::LanguagePair::default(),
        );
        unmanaged_manifest.mark(crate::application::job_status::JobStatus::Done);
        unmanaged.write_manifest(&unmanaged_manifest).unwrap();
        database
            .sync_snapshot(&crate::application::job_snapshot::JobSnapshot {
                manifest: unmanaged_manifest,
                segments: Vec::new(),
            })
            .await
            .unwrap();
        assert!(service.delete_persisted_job(&unmanaged.id()).await.is_err());
        assert!(unmanaged.dir.is_dir());

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn pause_kills_decoder_and_resume_keeps_task_without_original_media() {
        use std::{os::unix::fs::PermissionsExt, time::Duration};
        let root = std::env::temp_dir().join(format!("atogaki-pause-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let ffmpeg = root.join("ffmpeg");
        fs::write(&ffmpeg,"#!/bin/sh\nneed_input=0\nfor arg do\n if [ \"$need_input\" = 1 ]; then input=$arg; need_input=0; fi\n if [ \"$arg\" = -i ]; then need_input=1; fi\n destination=$arg\ndone\ncp \"$input\" \"$destination\"\n").unwrap();
        let whisper = root.join("whisper");
        fs::write(&whisper,"#!/bin/sh\nneed_prefix=0\nfor arg do\n if [ \"$need_prefix\" = 1 ]; then prefix=$arg; need_prefix=0; fi\n if [ \"$arg\" = -of ]; then need_prefix=1; fi\ndone\necho $$ > \"$prefix.pid\"\nexec sleep 60\n").unwrap();
        for path in [&ffmpeg, &whisper] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let input = root.join("input.wav");
        let mut wav = vec![];
        wav.extend(b"RIFF");
        wav.extend(32036u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(32000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(32000u32.to_le_bytes());
        wav.resize(32044, 0);
        fs::write(&input, wav).unwrap();
        let model = root.join("model.bin");
        fs::write(&model, b"fixture").unwrap();
        let db = LocalDatabase::open(root.join("db.sqlite")).await.unwrap();
        let service = LocalTaskService::with_workers_and_database(
            AppConfig {
                ffmpeg,
                whisper_cli: whisper,
                deepl_auth_key: None,
            },
            root.join("jobs"),
            1,
            8,
            Some(db.clone()),
        )
        .unwrap();
        let snapshot = service
            .submit_transcription(TranscribeSpec {
                input: input.clone(),
                output_dir: None,
                transcription: TranscriptionOptions::japanese(model.clone()),
            })
            .await
            .unwrap();
        let id = snapshot.manifest.job_id;
        let job = crate::infrastructure::job_store::Job::open(root.join("jobs").join(&id)).unwrap();
        let pid = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut found = None;
                if let Ok(entries) = fs::read_dir(job.dir.join("asr-checkpoint")) {
                    for entry in entries.flatten() {
                        if let Ok(pid) =
                            fs::read_to_string(entry.path().join("provider-output.pid"))
                        {
                            found = Some(pid);
                            break;
                        }
                    }
                }
                if let Some(pid) = found {
                    break pid.trim().to_string();
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let queued = service
            .submit_transcription(TranscribeSpec {
                input: input.clone(),
                output_dir: None,
                transcription: TranscriptionOptions::japanese(model.clone()),
            })
            .await
            .unwrap();
        let queued_id = queued.manifest.job_id;
        service.pause(&queued_id).await.unwrap();
        assert_eq!(
            db.get_job(&queued_id).await.unwrap().unwrap().status,
            "failed"
        );
        assert_eq!(
            service.resume(&queued_id).await.unwrap().manifest.job_id,
            queued_id
        );
        assert!(service.resume(&queued_id).await.is_err());
        service.pause(&queued_id).await.unwrap();
        service.pause(&id).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while service.controls.lock().await.contains_key(&id) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(db.get_job(&id).await.unwrap().unwrap().status, "failed");
        let status = std::process::Command::new("/bin/kill")
            .args(["-0", &pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "decoder must be terminated");
        fs::remove_file(input).unwrap();
        let moved_model = root.join("moved-model.bin");
        fs::rename(&model, &moved_model).unwrap();
        let wrong_model = root.join("wrong-model.bin");
        fs::write(&wrong_model, b"different weights").unwrap();
        assert!(
            service
                .resume_with_models(&id, Some(wrong_model), None)
                .await
                .is_err()
        );
        assert_eq!(job.read_recognition_options().unwrap().whisper.model, model);
        let resumed = service
            .resume_with_models(&id, Some(moved_model), None)
            .await
            .unwrap();
        assert_eq!(resumed.manifest.job_id, id);
        assert!(service.resume(&id).await.is_err());
        service.pause(&id).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while service.controls.lock().await.contains_key(&id) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        drop(service);
        db.close().await;
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn stage_pipeline_starts_two_asr_jobs_and_next_job_while_early_cloud_is_blocked() {
        use super::{Arc, Duration, Job};
        use crate::application::{
            TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
            TranslationResponse, TranslationResult, TranslationUsage,
        };
        use std::{
            os::unix::fs::PermissionsExt,
            sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        #[derive(Debug)]
        struct Cloud {
            started: tokio::sync::mpsc::UnboundedSender<()>,
            release: Arc<tokio::sync::Semaphore>,
            calls: AtomicUsize,
            fail_once: AtomicBool,
        }
        impl TranslationProvider for Cloud {
            fn status(&self) -> TranslationProviderStatus {
                TranslationProviderStatus {
                    id: "deepseek".into(),
                    name: "Fixture".into(),
                    configured: true,
                    model: Some("v1".into()),
                    endpoint_kind: "test".into(),
                    configuration_hint: None,
                }
            }
            fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
                Box::pin(async move {
                    self.calls.fetch_add(1, Ordering::SeqCst);
                    self.started.send(()).unwrap();
                    self.release.acquire().await.unwrap().forget();
                    if self.fail_once.swap(false, Ordering::SeqCst) {
                        anyhow::bail!("injected cloud interruption");
                    }
                    Ok(TranslationResponse {
                        translations: request
                            .targets
                            .into_iter()
                            .map(|t| TranslationResult {
                                segment_id: t.segment_id,
                                translated_text: format!("译：{}", t.source_text),
                            })
                            .collect(),
                        model: Some("v1".into()),
                        usage: TranslationUsage::default(),
                    })
                })
            }
        }
        let root = std::env::temp_dir().join(format!("atogaki-stage-e2e-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let ffmpeg = root.join("ffmpeg");
        fs::write(&ffmpeg,"#!/bin/sh\nneed=0\nfor arg do\n if [ \"$need\" = 1 ]; then input=$arg; need=0; fi\n if [ \"$arg\" = -i ]; then need=1; fi\n destination=$arg\ndone\ncp \"$input\" \"$destination\"\n").unwrap();
        let whisper = root.join("whisper");
        fs::write(&whisper,r##"#!/bin/sh
need=0
for arg do
 if [ "$need" = 1 ]; then prefix=$arg; need=0; fi
 if [ "$arg" = -of ]; then need=1; fi
done
attempt=$(dirname "$prefix")
checkpoint=$(dirname "$attempt")
job=$(dirname "$checkpoint")
index=$(basename "$attempt" | cut -d- -f2)
echo started > "$prefix.started"
if [ "$index" = 2 ]; then
 while [ ! -f "$job/release-final" ]; do sleep 0.02; done
fi
printf '{"transcription":[{"offsets":{"from":6000,"to":7000},"text":"Sentence %s."}]}' "$index" > "$prefix.json"
"##).unwrap();
        for p in [&ffmpeg, &whisper] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let input = root.join("input.wav");
        let data_len = 360 * 32000u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((data_len + 36).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(32000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(data_len.to_le_bytes());
        wav.resize(data_len as usize + 44, 0);
        fs::write(&input, wav).unwrap();
        let model = root.join("model");
        fs::write(&model, b"fixture").unwrap();
        let db = LocalDatabase::open(root.join("index.sqlite"))
            .await
            .unwrap();
        let service = LocalTaskService::start_with_database(
            AppConfig {
                ffmpeg,
                whisper_cli: whisper,
                deepl_auth_key: None,
            },
            root.join("jobs"),
            db.clone(),
        )
        .unwrap();
        let (started, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let cloud = Arc::new(Cloud {
            started,
            release: release.clone(),
            calls: AtomicUsize::new(0),
            fail_once: AtomicBool::new(false),
        });
        let mut jobs = vec![];
        for _ in 0..3 {
            let snapshot = service
                .submit_with_translation(
                    TranscribeSpec {
                        input: input.clone(),
                        output_dir: None,
                        transcription: TranscriptionOptions::new(
                            model.clone(),
                            LanguageCode::English,
                        ),
                    },
                    None,
                    &[],
                    Some((cloud.clone(), serde_json::json!({"provider":"fixture"}))),
                )
                .await
                .unwrap();
            jobs.push(Job::open(root.join("jobs").join(snapshot.manifest.job_id)).unwrap());
        }
        for _ in 0..2 {
            tokio::time::timeout(Duration::from_secs(10), receiver.recv())
                .await
                .unwrap()
                .unwrap();
        }
        let active = jobs
            .iter()
            .filter(|j| j.dir.join("asr-checkpoint").is_dir())
            .collect::<Vec<_>>();
        assert_eq!(
            active.len(),
            2,
            "third ASR waits while first two are active"
        );
        for job in active {
            fs::write(job.dir.join("release-final"), b"continue").unwrap();
        }
        tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            jobs.iter().all(|j| j.dir.join("asr-checkpoint").is_dir()),
            "next ASR starts while earlier cloud requests are blocked"
        );
        assert_eq!(cloud.calls.load(Ordering::SeqCst), 3);
        for job in &jobs {
            fs::write(job.dir.join("release-final"), b"continue").unwrap();
        }
        release.add_permits(32);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let mut done = true;
                for job in &jobs {
                    let record = db.get_job(&job.id()).await.unwrap().unwrap();
                    assert_ne!(record.status, "failed", "{:?}", record.error_message);
                    done &= record.status == "done"
                        && !service.controls.lock().await.contains_key(&job.id());
                }
                if done {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            cloud.calls.load(Ordering::SeqCst),
            9,
            "each prefix result reused by final translation"
        );
        for job in &jobs {
            let cues = db.list_segments(&job.id()).await.unwrap();
            assert_eq!(cues.len(), 3);
            assert!(cues.iter().all(|c| c.translated_text.is_some()));
        }
        // A later task's early API failure must not stop ASR. Its resume must
        // use only the translation stage, even after original resources vanish.
        while receiver.try_recv().is_ok() {}
        cloud.fail_once.store(true, Ordering::SeqCst);
        let failed = service
            .submit_with_translation(
                TranscribeSpec {
                    input: input.clone(),
                    output_dir: None,
                    transcription: TranscriptionOptions::new(model.clone(), LanguageCode::English),
                },
                None,
                &[],
                Some((cloud.clone(), serde_json::json!({"provider":"fixture"}))),
            )
            .await
            .unwrap();
        let failed_job = Job::open(root.join("jobs").join(&failed.manifest.job_id)).unwrap();
        tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        fs::write(failed_job.dir.join("release-final"), b"continue").unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if db.get_job(&failed_job.id()).await.unwrap().unwrap().status == "failed"
                    && !service.controls.lock().await.contains_key(&failed_job.id())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let stopped = failed_job.read_manifest_if_exists().unwrap().unwrap();
        assert_eq!(
            stopped.failed_stage,
            Some(crate::application::job_status::JobStatus::Translating)
        );
        assert!(failed_job.dir.join("asr-published.json").is_file());
        let evidence_hashes = (0..3)
            .map(|i| {
                crate::application::acoustic_plan::file_digest(
                    &failed_job
                        .dir
                        .join(format!("asr-checkpoint/chunk-{i:06}/evidence.json")),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        fs::remove_file(&model).unwrap();
        fs::remove_file(&input).unwrap();
        service
            .configure_task_translation(&failed_job.id(), cloud.clone())
            .await
            .unwrap();
        service.resume(&failed_job.id()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let record = db.get_job(&failed_job.id()).await.unwrap().unwrap();
                assert_ne!(record.status, "failed", "{:?}", record.error_message);
                if record.status == "done"
                    && !service.controls.lock().await.contains_key(&failed_job.id())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        for (i, expected) in evidence_hashes.iter().enumerate() {
            assert_eq!(
                &crate::application::acoustic_plan::file_digest(
                    &failed_job
                        .dir
                        .join(format!("asr-checkpoint/chunk-{i:06}/evidence.json"))
                )
                .unwrap(),
                expected
            );
        }
        assert_eq!(cloud.calls.load(Ordering::SeqCst), 13);
        drop(service);
        db.close().await;
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    #[ignore = "requires ATOGAKI_STAGE_SMOKE_INPUT, ATOGAKI_STAGE_SMOKE_MODEL and real Metal sidecars"]
    async fn real_two_job_metal_pipeline_with_local_translation_fixture() {
        use super::{Arc, Job};
        use crate::application::{
            TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
            TranslationResponse, TranslationResult, TranslationUsage,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        #[derive(Debug)]
        struct Echo {
            jobs: std::path::PathBuf,
            early: AtomicBool,
        }
        impl TranslationProvider for Echo {
            fn status(&self) -> TranslationProviderStatus {
                TranslationProviderStatus {
                    id: "deepseek".into(),
                    name: "Local fixture (no network)".into(),
                    configured: true,
                    model: Some("fixture".into()),
                    endpoint_kind: "test".into(),
                    configuration_hint: None,
                }
            }
            fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
                Box::pin(async move {
                    for entry in fs::read_dir(&self.jobs)?.flatten() {
                        let Ok(job) = Job::open(entry.path()) else {
                            continue;
                        };
                        let Ok(summary) = crate::application::acoustic_plan::recovery_summary(&job)
                        else {
                            continue;
                        };
                        if summary.completed_ms > 0 && summary.completed_ms < summary.duration_ms {
                            let units = crate::application::acoustic_plan::partial_units(&job)?;
                            if request
                                .targets
                                .iter()
                                .any(|target| units.iter().any(|unit| unit.id == target.segment_id))
                            {
                                self.early.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    Ok(TranslationResponse {
                        translations: request
                            .targets
                            .into_iter()
                            .map(|t| TranslationResult {
                                segment_id: t.segment_id,
                                translated_text: format!("[local fixture] {}", t.source_text),
                            })
                            .collect(),
                        model: Some("fixture".into()),
                        usage: TranslationUsage::default(),
                    })
                })
            }
        }
        let input = std::path::PathBuf::from(std::env::var("ATOGAKI_STAGE_SMOKE_INPUT").unwrap());
        let model = std::path::PathBuf::from(std::env::var("ATOGAKI_STAGE_SMOKE_MODEL").unwrap());
        let sidecars =
            std::path::PathBuf::from(std::env::var("ATOGAKI_STAGE_SMOKE_SIDECARS").unwrap());
        let root = std::path::PathBuf::from(std::env::var("ATOGAKI_STAGE_SMOKE_OUTPUT").unwrap());
        fs::create_dir_all(&root).unwrap();
        let db = LocalDatabase::open(root.join("index.sqlite"))
            .await
            .unwrap();
        let service = LocalTaskService::start_with_database(
            AppConfig {
                ffmpeg: sidecars.join("ffmpeg"),
                whisper_cli: sidecars.join("whisper-cli"),
                deepl_auth_key: None,
            },
            root.join("jobs"),
            db.clone(),
        )
        .unwrap();
        let provider = Arc::new(Echo {
            jobs: root.join("jobs"),
            early: AtomicBool::new(false),
        });
        let mut jobs = vec![];
        for _ in 0..2 {
            let mut options = TranscriptionOptions::new(model.clone(), LanguageCode::English);
            options.whisper.vad_model = Some(std::path::PathBuf::from(
                std::env::var("ATOGAKI_STAGE_SMOKE_VAD").unwrap(),
            ));
            let snapshot = service
                .submit_with_translation(
                    TranscribeSpec {
                        input: input.clone(),
                        output_dir: None,
                        transcription: options,
                    },
                    None,
                    &[],
                    Some((provider.clone(), serde_json::json!({"test":"local-only"}))),
                )
                .await
                .unwrap();
            jobs.push(Job::open(root.join("jobs").join(snapshot.manifest.job_id)).unwrap());
        }
        tokio::time::timeout(std::time::Duration::from_secs(600), async {
            loop {
                let mut done = true;
                for job in &jobs {
                    let record = db.get_job(&job.id()).await.unwrap().unwrap();
                    assert_ne!(record.status, "failed", "{:?}", record.error_message);
                    done &= record.status == "done"
                        && !service.controls.lock().await.contains_key(&job.id());
                }
                if done {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            provider.early.load(Ordering::SeqCst),
            "real translation request must precede ASR completion for its own job"
        );
        let mut intervals = vec![];
        let mut report_jobs = vec![];
        for job in &jobs {
            let segments = db.list_segments(&job.id()).await.unwrap();
            assert!(!segments.is_empty());
            assert!(segments.iter().all(|s| s.translated_text.is_some()));
            let evidence_path = job.dir.join("asr-checkpoint/chunk-000000/evidence.json");
            let evidence: serde_json::Value =
                serde_json::from_slice(&fs::read(&evidence_path).unwrap()).unwrap();
            assert_eq!(evidence["device"], "gpu");
            let end = fs::metadata(evidence_path)
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let elapsed = evidence["recognition_elapsed_ms"].as_u64().unwrap() as u128;
            intervals.push((end - elapsed, end));
            report_jobs.push(serde_json::json!({"job_id":job.id(),"cues":segments.len(),"first_chunk_device":evidence["device"],"first_chunk_elapsed_ms":elapsed,"early_progress":serde_json::from_slice::<serde_json::Value>(&fs::read(job.dir.join("early-translation/progress.json")).unwrap()).unwrap()}));
        }
        assert!(
            intervals[0].0.max(intervals[1].0) < intervals[0].1.min(intervals[1].1),
            "first GPU recognitions must overlap"
        );
        crate::application::acoustic_plan::write_json(&root.join("report.json"),&serde_json::json!({"early_translation_before_own_asr_complete":true,"gpu_intervals_overlap":true,"first_chunk_intervals_ms":intervals,"cloud_network_used":false,"jobs":report_jobs})).unwrap();
        drop(service);
        db.close().await;
    }
}
