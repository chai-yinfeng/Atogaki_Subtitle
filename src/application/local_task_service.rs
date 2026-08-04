use std::{
    path::{Path, PathBuf},
    sync::Arc,
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
        local_glossary_service::glossary_from_detail,
    },
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
        self.submit_transcription_with_glossary(spec, None).await
    }

    pub async fn submit_transcription_with_glossary(
        &self,
        mut spec: TranscribeSpec,
        glossary_id: Option<&str>,
    ) -> Result<JobSnapshot> {
        self.require_service_owned_output_dir(spec.output_dir.as_deref())?;
        let job = self
            .create_queued_job(Some(spec.input.clone()), None)
            .await?;
        if let Some(glossary_id) = glossary_id {
            let database = self.database.as_ref().ok_or_else(|| {
                anyhow!("glossary selection requires SQLite-backed local task service")
            })?;
            let detail = database
                .get_glossary(glossary_id)
                .await?
                .ok_or_else(|| anyhow!("local glossary not found: {glossary_id}"))?;
            let snapshot_path = job.dir.join("recognition-glossary.txt");
            tokio::fs::write(
                &snapshot_path,
                glossary_from_detail(&detail)?.to_file_text(),
            )
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
        let job = self
            .create_queued_job(Some(spec.input.clone()), spec.render_output.clone())
            .await?;
        spec.output_dir = Some(job.dir.clone());
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
    ) -> Result<Job> {
        let job = Job::create_in(&self.jobs_dir)?;
        let mut manifest = JobManifest::new(&job, input, render_output);
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
                .unwrap_or_else(|| JobManifest::new(&job, None, None));
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
                let result = runner.transcribe(spec).await.map(|_| ());
                (job_dir, result)
            }
            QueuedTask::Process { job_dir, spec } => {
                let result = runner.process(spec).await.map(|_| ());
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::LocalTaskService;
    use crate::{
        application::{TranscriptionOptions, job_spec::TranscribeSpec},
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

        let job = service.create_queued_job(None, None).await.unwrap();
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
                vec![LocalGlossaryTermInput {
                    source_text: "ナブナ".to_string(),
                    target_text: Some("n-buna".to_string()),
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

        let snapshot = service
            .submit_transcription_with_glossary(
                TranscribeSpec {
                    input: root.join("input.mp3"),
                    output_dir: None,
                    transcription: TranscriptionOptions::japanese(root.join("model.bin")),
                },
                Some(&glossary.glossary.id),
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

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
