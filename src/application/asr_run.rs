use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::application::AsrInputScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl AsrRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

/// Durable metadata for one provider invocation inside a subtitle task.
///
/// A successful run remains a candidate until a separate workspace operation
/// adopts it. This keeps recognition history independent from the editable
/// subtitle view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrRun {
    pub schema_version: u32,
    pub id: String,
    pub job_id: String,
    pub parent_run_id: Option<String>,
    pub provider_id: String,
    pub provider_name: String,
    pub model_identity: String,
    pub input_media: PathBuf,
    pub scope: AsrInputScope,
    pub config_snapshot: Value,
    pub status: AsrRunStatus,
    pub created_at_unix: u64,
    pub started_at_unix: Option<u64>,
    pub completed_at_unix: Option<u64>,
    pub error: Option<String>,
}

impl AsrRun {
    pub fn new(
        job_id: impl Into<String>,
        parent_run_id: Option<String>,
        provider_id: impl Into<String>,
        provider_name: impl Into<String>,
        model_identity: impl Into<String>,
        input_media: PathBuf,
        scope: AsrInputScope,
        config_snapshot: Value,
    ) -> Self {
        Self {
            schema_version: 1,
            id: Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            parent_run_id,
            provider_id: provider_id.into(),
            provider_name: provider_name.into(),
            model_identity: model_identity.into(),
            input_media,
            scope,
            config_snapshot,
            status: AsrRunStatus::Queued,
            created_at_unix: unix_now(),
            started_at_unix: None,
            completed_at_unix: None,
            error: None,
        }
    }

    pub fn start(&mut self) {
        self.status = AsrRunStatus::Running;
        self.started_at_unix = Some(unix_now());
        self.completed_at_unix = None;
        self.error = None;
    }

    pub fn succeed(&mut self) {
        self.status = AsrRunStatus::Succeeded;
        self.completed_at_unix = Some(unix_now());
        self.error = None;
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = AsrRunStatus::Failed;
        self.completed_at_unix = Some(unix_now());
        self.error = Some(error.into());
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::application::{AsrInputScope, AsrRunStatus};

    use super::AsrRun;

    #[test]
    fn run_lifecycle_records_terminal_failure_without_adopting_a_workspace() {
        let mut run = AsrRun::new(
            "job-1",
            None,
            "whisper.cpp",
            "Whisper.cpp",
            "model.bin",
            "audio.wav".into(),
            AsrInputScope::full(),
            json!({"max_context": 0}),
        );
        assert_eq!(run.status, AsrRunStatus::Queued);
        run.start();
        assert_eq!(run.status, AsrRunStatus::Running);
        run.fail("decoder stopped");
        assert_eq!(run.status, AsrRunStatus::Failed);
        assert_eq!(run.error.as_deref(), Some("decoder stopped"));
        assert!(run.completed_at_unix.is_some());
    }
}
