use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use tokio::sync::{Mutex, mpsc};

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
    infrastructure::{config::AppConfig, job_store::Job, local_db::LocalDatabase},
};

const DEFAULT_QUEUE_CAPACITY: usize = 8;

/// A durable, local queue for the long-running media operations used by the
/// desktop UI. Queued state is written to disk before work is sent to a worker,
/// so the UI can immediately poll a task without holding an HTTP request open.
#[derive(Clone)]
pub struct LocalTaskService {
    sender: mpsc::Sender<QueuedTask>,
    jobs_dir: PathBuf,
    database: Option<LocalDatabase>,
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
    /// Starts a single worker. One is the safe default because local Whisper
    /// models usually compete for the same CPU, memory, or GPU resources.
    pub fn start(config: AppConfig, jobs_dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_workers(config, jobs_dir, 1, DEFAULT_QUEUE_CAPACITY)
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
        Self::with_workers_and_database(config, jobs_dir, 1, DEFAULT_QUEUE_CAPACITY, Some(database))
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

        for _ in 0..workers {
            handle.spawn(run_worker(
                config.clone(),
                Arc::clone(&receiver),
                database.clone(),
            ));
        }

        Ok(Self {
            sender,
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
        mut spec: TranscribeSpec,
        glossary_id: Option<&str>,
        selected_content_groups: &[String],
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

    /// Converts non-terminal task state left by an earlier application session
    /// into an explicit, retryable failure. Automatically resuming an external
    /// ffmpeg/Whisper process is unsafe because its process state cannot be
    /// reconstructed after the desktop application exits.
    pub async fn recover_interrupted_jobs(&self) -> Result<usize> {
        let database = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow!("task recovery requires SQLite persistence"))?;
        let mut recovered = 0;
        for record in database.list_jobs().await? {
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
            manifest.fail(
                "Atogaki 上次退出时任务尚未完成。旧任务数据已保留，可点击“重试”创建一个新任务。",
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
        if !transcription.model.is_file() {
            transcription.model = whisper_model_override
                .filter(|path| path.is_file())
                .ok_or_else(|| {
                    anyhow!(
                        "Whisper model no longer exists: {}; configure an available model before retrying",
                        transcription.model.display()
                    )
                })?;
        }
        if let Some(missing_vad_model) = transcription
            .vad_model
            .as_ref()
            .filter(|path| !path.is_file())
        {
            transcription.vad_model = Some(
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

    async fn enqueue(&self, job: Job, task: QueuedTask) -> Result<JobSnapshot> {
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
) {
    let runner = JobRunner::new(config);

    loop {
        let task = {
            let mut receiver = receiver.lock().await;
            receiver.recv().await
        };

        let Some(task) = task else {
            return;
        };

        let (job_dir, result) = match task {
            QueuedTask::Transcribe { job_dir, spec } => {
                let result =
                    run_with_status_sync(&job_dir, database.as_ref(), runner.transcribe(spec))
                        .await
                        .map(|_| ());
                (job_dir, result)
            }
            QueuedTask::Process { job_dir, spec } => {
                let result =
                    run_with_status_sync(&job_dir, database.as_ref(), runner.process(spec))
                        .await
                        .map(|_| ());
                (job_dir, result)
            }
        };

        if let Err(error) = result {
            eprintln!("[task-service] task failed: {error:#}");
        }

        if let Some(database) = &database {
            match JobSnapshot::load(&job_dir) {
                Ok(snapshot) => {
                    if let Err(error) = database.sync_snapshot(&snapshot).await {
                        eprintln!("[task-service] failed to sync local database: {error:#}");
                    }
                }
                Err(error) => eprintln!("[task-service] failed to load task state: {error:#}"),
            }
        }
    }
}

async fn run_with_status_sync<T>(
    job_dir: &Path,
    database: Option<&LocalDatabase>,
    task: impl Future<Output = Result<T>>,
) -> Result<T> {
    let Some(database) = database else {
        return task.await;
    };
    let mut task = Box::pin(task);
    let mut refresh = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            result = &mut task => return result,
            _ = refresh.tick() => {
                if let Ok(snapshot) = JobSnapshot::load(job_dir)
                    && let Err(error) = database.sync_snapshot(&snapshot).await
                {
                    eprintln!("[task-service] failed to publish running task status: {error:#}");
                }
            }
        }
    }
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
            sender,
            jobs_dir: root.join("jobs"),
            database: Some(database.clone()),
        };

        let vad_model = root.join("ggml-silero.bin");
        let mut transcription = TranscriptionOptions::japanese(root.join("model.bin"));
        transcription.vad_model = Some(vad_model.clone());
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
            persisted_options.vad_model.as_deref(),
            Some(vad_model.as_path())
        );
        assert_eq!(persisted_options.vad_max_speech_s, 8);
        let prompt = fs::read_to_string(
            root.join("jobs")
                .join(&snapshot.manifest.job_id)
                .join("whisper-prompt.txt"),
        )
        .unwrap();
        assert!(prompt.contains("ナブナ（表記: n-buna）"));

        drop(service);
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
        assert_eq!(retried_job.read_recognition_options().unwrap().model, model);
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
            retried_again_job.read_recognition_options().unwrap().model,
            replacement_model
        );

        drop(service);
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
                manifest,
                segments: Vec::new(),
            })
            .await
            .unwrap();
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
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
