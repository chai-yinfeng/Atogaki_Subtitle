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
    },
    infrastructure::{config::AppConfig, job_store::Job},
};

const DEFAULT_QUEUE_CAPACITY: usize = 8;

/// A durable, local queue for the long-running media operations used by the
/// desktop UI. Queued state is written to disk before work is sent to a worker,
/// so the UI can immediately poll a task without holding an HTTP request open.
#[derive(Clone)]
pub struct LocalTaskService {
    sender: mpsc::Sender<QueuedTask>,
    jobs_dir: PathBuf,
}

enum QueuedTask {
    Transcribe(TranscribeSpec),
    Process(ProcessSpec),
}

impl LocalTaskService {
    /// Starts a single worker. One is the safe default because local Whisper
    /// models usually compete for the same CPU, memory, or GPU resources.
    pub fn start(config: AppConfig, jobs_dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_workers(config, jobs_dir, 1, DEFAULT_QUEUE_CAPACITY)
    }

    /// Starts a bounded queue with an explicit number of workers.
    pub fn with_workers(
        config: AppConfig,
        jobs_dir: impl Into<PathBuf>,
        workers: usize,
        queue_capacity: usize,
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
            handle.spawn(run_worker(config.clone(), Arc::clone(&receiver)));
        }

        Ok(Self {
            sender,
            jobs_dir: jobs_dir.into(),
        })
    }

    pub async fn submit_transcription(&self, mut spec: TranscribeSpec) -> Result<JobSnapshot> {
        self.require_service_owned_output_dir(spec.output_dir.as_deref())?;
        let job = self.create_queued_job(Some(spec.input.clone()), None)?;
        spec.output_dir = Some(job.dir.clone());
        self.enqueue(job, QueuedTask::Transcribe(spec)).await
    }

    pub async fn submit_process(&self, mut spec: ProcessSpec) -> Result<JobSnapshot> {
        self.require_service_owned_output_dir(spec.output_dir.as_deref())?;
        let job = self.create_queued_job(Some(spec.input.clone()), spec.render_output.clone())?;
        spec.output_dir = Some(job.dir.clone());
        self.enqueue(job, QueuedTask::Process(spec)).await
    }

    /// Reads a task's current durable state. A UI can poll this method now and
    /// switch to event-driven updates later without changing task persistence.
    pub fn snapshot(&self, job_dir: impl AsRef<Path>) -> Result<JobSnapshot> {
        JobSnapshot::load(job_dir)
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

    fn create_queued_job(
        &self,
        input: Option<PathBuf>,
        render_output: Option<PathBuf>,
    ) -> Result<Job> {
        let job = Job::create_in(&self.jobs_dir)?;
        let mut manifest = JobManifest::new(&job, input, render_output);
        manifest.mark(JobStatus::Queued);
        job.write_manifest(&manifest)?;
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

async fn run_worker(config: AppConfig, receiver: Arc<Mutex<mpsc::Receiver<QueuedTask>>>) {
    let runner = JobRunner::new(config);

    loop {
        let task = {
            let mut receiver = receiver.lock().await;
            receiver.recv().await
        };

        let Some(task) = task else {
            return;
        };

        let result = match task {
            QueuedTask::Transcribe(spec) => runner.transcribe(spec).await.map(|_| ()),
            QueuedTask::Process(spec) => runner.process(spec).await.map(|_| ()),
        };

        if let Err(error) = result {
            eprintln!("[task-service] task failed: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LocalTaskService;
    use crate::infrastructure::config::AppConfig;

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
}
