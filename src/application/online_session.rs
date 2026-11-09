//! Own the live capture/worker lifetime; no session audio or events are persisted.
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, watch},
    task::JoinHandle,
    time::{Instant, sleep_until, timeout},
};
use uuid::Uuid;

use super::stage_scheduler::StageScheduler;

#[derive(Clone)]
pub struct OnlineRuntime {
    pub python: PathBuf,
    /// Directory containing the online_worker package, not its __main__.py.
    pub worker_root: PathBuf,
    pub capture: PathBuf,
    pub translator: Option<crate::infrastructure::local_llama::LocalLlamaTranslationProvider>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineModels {
    pub language: String,
    pub draft_model: PathBuf,
    pub final_model: PathBuf,
    pub vad_model: PathBuf,
    /// The current worker only accepts loopback text translation. No cloud fallback.
    pub translation_url: Option<String>,
    #[serde(default = "enabled")]
    pub translation_drafts: bool,
    #[serde(default)]
    pub translation_context: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSessionStatus {
    pub session_id: Option<String>,
    pub phase: String,
    pub error: Option<String>,
    pub translation_enabled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSessionEvent {
    pub session_id: String,
    pub event: Value,
}

fn enabled() -> bool {
    true
}

type EventSink = Arc<dyn Fn(OnlineSessionEvent) + Send + Sync>;
struct ActiveSession {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct OnlineSessionService {
    active: Arc<Mutex<Option<ActiveSession>>>,
    status: watch::Sender<OnlineSessionStatus>,
    scheduler: Arc<StageScheduler>,
}

impl OnlineSessionService {
    pub fn new(scheduler: Arc<StageScheduler>) -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            status: watch::channel(OnlineSessionStatus {
                session_id: None,
                phase: "idle".into(),
                error: None,
                translation_enabled: false,
            })
            .0,
            scheduler,
        }
    }

    pub fn status(&self) -> OnlineSessionStatus {
        self.status.borrow().clone()
    }

    pub async fn start(
        &self,
        runtime: OnlineRuntime,
        models: OnlineModels,
        emit: EventSink,
    ) -> Result<OnlineSessionStatus> {
        let mut active = self.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|session| !session.task.is_finished())
        {
            bail!("an online session is already active");
        }
        for path in [&runtime.python, &runtime.capture, &models.vad_model] {
            if !path.is_file() {
                bail!("online runtime/model is not ready: {}", path.display());
            }
        }
        for path in [
            &models.draft_model,
            &models.final_model,
            &runtime.worker_root.join("online_worker"),
        ] {
            if !path.is_dir() {
                bail!("online model/package is not ready: {}", path.display());
            }
        }
        if !["ja", "en", "zh", "ko"].contains(&models.language.as_str()) {
            bail!("unsupported online source language");
        }
        let permit = self.scheduler.try_acquire_device().context(
            "local device is busy; finish the existing offline work before starting online capture",
        )?;
        let translation_enabled = models.translation_url.is_some() || runtime.translator.is_some();
        let id = Uuid::new_v4().to_string();
        let (stop, receiver) = watch::channel(false);
        self.status.send_replace(OnlineSessionStatus {
            session_id: Some(id.clone()),
            phase: "loading".into(),
            error: None,
            translation_enabled,
        });
        let status = self.status.clone();
        let task = tokio::spawn(async move {
            let _permit = permit;
            let translator = runtime.translator.clone();
            let result = run_session(&id, runtime, models, receiver, &status, &emit).await;
            if let Some(translator) = translator {
                let _ = translator.shutdown().await;
            }
            let (phase, error) = match result {
                Ok(()) => ("stopped", None),
                Err(error) => ("failed", Some(format!("{error:#}"))),
            };
            status.send_replace(OnlineSessionStatus {
                session_id: Some(id.clone()),
                phase: phase.into(),
                error: error.clone(),
                translation_enabled,
            });
            emit(OnlineSessionEvent {
                session_id: id,
                event: json!({"kind":"session_state", "phase":phase, "error":error}),
            });
        });
        *active = Some(ActiveSession { stop, task });
        Ok(self.status())
    }

    pub async fn stop(&self) -> Result<OnlineSessionStatus> {
        let mut active = self.active.lock().await;
        if let Some(session) = active.take() {
            let _ = session.stop.send(true);
            session
                .task
                .await
                .context("online session task stopped unexpectedly")?;
        }
        Ok(self.status())
    }
}

fn phase(status: &watch::Sender<OnlineSessionStatus>, value: &str) {
    status.send_modify(|s| s.phase = value.into());
}
fn relay(id: &str, line: &str, emit: &EventSink) -> Result<Value> {
    let event: Value = serde_json::from_str(line).context("invalid online worker event")?;
    emit(OnlineSessionEvent {
        session_id: id.into(),
        event: event.clone(),
    });
    if matches!(
        event["kind"].as_str(),
        Some("worker_error" | "capture_error")
    ) {
        bail!(
            "online worker failed: {}",
            event.get("error").unwrap_or(&Value::Null)
        );
    }
    Ok(event)
}

async fn run_session(
    id: &str,
    runtime: OnlineRuntime,
    mut models: OnlineModels,
    mut stop: watch::Receiver<bool>,
    status: &watch::Sender<OnlineSessionStatus>,
    emit: &EventSink,
) -> Result<()> {
    let mut translation_key = None;
    if let Some(translator) = &runtime.translator {
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            connection = translator.online_connection() => {
                let (url, key) = connection?;
                models.translation_url = Some(url);
                translation_key = Some(key);
            }
        }
    }
    let mut command = Command::new(runtime.python);
    command
        .args([
            "-s",
            "-B",
            "-u",
            "-m",
            "online_worker",
            "--pcm-stdin",
            "--language",
            &models.language,
            "--draft",
            if models.language == "ja" {
                "sensevoice"
            } else {
                "zipformer"
            },
            "--final",
            "qwen",
        ])
        .arg("--draft-model")
        .arg(models.draft_model)
        .arg("--final-model")
        .arg(models.final_model)
        .arg("--vad-model")
        .arg(models.vad_model)
        .env("PYTHONPATH", runtime.worker_root)
        .env("PYTHONNOUSERSITE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command.env(
        "ATOGAKI_ONLINE_MT_CONTEXT",
        models.translation_context.to_string(),
    );
    if !models.translation_drafts {
        command.arg("--no-translation-drafts");
    }
    if let Some(key) = translation_key {
        command.env("ATOGAKI_ONLINE_LLAMA_KEY", key);
    }
    if let Some(url) = models.translation_url {
        command.arg("--translation-url").arg(url);
    }
    let mut worker = command
        .spawn()
        .context("start local online inference worker")?;
    let mut events =
        BufReader::new(worker.stdout.take().context("missing worker event pipe")?).lines();
    let ready = timeout(Duration::from_secs(120), async {
        loop {
            tokio::select! {
                _ = stop.changed() => return Ok(false),
                line = events.next_line() => {
                    let line = line?.context("worker exited before becoming ready")?;
                    if relay(id, &line, emit)?["kind"] == "session_ready" { return Ok::<_, anyhow::Error>(true); }
                }
            }
        }
    }).await;
    let ready = match ready {
        Ok(result) => result,
        Err(error) => Err(error).context("online model load timed out"),
    };
    match ready {
        Ok(true) => {}
        result => {
            let _ = worker.kill().await;
            return result.map(|_| ());
        }
    }
    // Start capture only once models are ready; loading never buffers system audio.
    let mut capture = match Command::new(runtime.capture)
        .arg("--exclude-pid")
        .arg(std::process::id().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = worker.kill().await;
            return Err(error).context("start system audio capture");
        }
    };
    // Child::wait closes its owned stdin before waiting. Keep the stop pipe
    // outside Child so polling wait in select cannot accidentally stop capture.
    let mut capture_control = capture.stdin.take();
    let mut capture_events = BufReader::new(
        capture
            .stderr
            .take()
            .context("missing capture status pipe")?,
    )
    .lines();
    let mut input = capture
        .stdout
        .take()
        .context("missing capture audio pipe")?;
    let mut output = worker.stdin.take().context("missing worker audio pipe")?;
    let pipe = tokio::spawn(async move { tokio::io::copy(&mut input, &mut output).await });
    phase(status, "requesting_audio");
    let result = async {
        let mut stopping = false;
        let mut captured = false;
        let mut capture_events_open = true;
        let mut drained = false;
        let mut drain_end = Instant::now() + Duration::from_secs(86_400);
        loop {
            tokio::select! {
                _ = stop.changed(), if !stopping => {
                    stopping = true;
                    phase(status, "stopping");
                    drain_end = Instant::now()+Duration::from_secs(30);
                    // Helper destroys its tap, closes stdout; the pipe closes
                    // worker stdin, which flushes VAD, final ASR and MT.
                    if let Some(mut control) = capture_control.take() { let _ = control.write_all(&[0]).await; }
                }
                exit = capture.wait(), if !captured => {
                    captured = true;
                    if !exit?.success() { bail!("system audio capture exited unsuccessfully"); }
                    phase(status, "draining");
                    drain_end = Instant::now()+Duration::from_secs(30);
                }
                line = capture_events.next_line(), if capture_events_open => {
                    if let Some(line) = line? {
                        let event = relay(id, &line, emit)?;
                        if event["kind"] == "capture_ready" && !stopping && !captured { phase(status, "running"); }
                    } else { capture_events_open = false; }
                }
                line = events.next_line() => {
                    match line? {
                        Some(line) => { if relay(id, &line, emit)?["kind"] == "session_stopped" { drained = true; } },
                        None => {
                            if !drained { bail!("online worker exited without draining"); }
                            if !worker.wait().await?.success() { bail!("online worker exited unsuccessfully"); }
                            return Ok(());
                        }
                    }
                }
                _ = sleep_until(drain_end), if stopping || captured => bail!("online session drain timed out"),
            }
        }
    }.await;
    // Errors also stop capture; do not leave a HAL tap or inference child alive.
    if let Some(mut control) = capture_control.take() {
        let _ = control.write_all(&[0]).await;
    }
    if timeout(Duration::from_secs(2), capture.wait())
        .await
        .is_err()
    {
        let _ = capture.kill().await;
    }
    if worker.try_wait()?.is_none() {
        let _ = worker.kill().await;
    }
    pipe.abort();
    let _ = pipe.await;
    result
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn translation_failures_are_relayed_without_aborting_the_session() {
        let recorded = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = recorded.clone();
        let sink: EventSink = Arc::new(move |event| captured.lock().unwrap().push(event));
        for kind in ["translation_error", "translation_failed"] {
            relay(
                "session",
                &json!({"kind": kind, "error": "MT rejected"}).to_string(),
                &sink,
            )
            .unwrap();
        }
        assert!(
            relay(
                "session",
                r#"{"kind":"worker_error","error":"ASR failed"}"#,
                &sink
            )
            .is_err()
        );
        assert_eq!(recorded.lock().unwrap().len(), 3);
    }

    fn executable(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[tokio::test]
    async fn session_waits_for_models_then_stops_capture_and_drains_without_recording() {
        let root = std::env::temp_dir().join(format!("atogaki-session-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("online_worker")).unwrap();
        std::fs::write(root.join("vad.onnx"), b"fixture").unwrap();
        executable(
            &root.join("python"),
            "#!/bin/sh\nprintf '%s\\n' '{\"kind\":\"session_ready\"}'\ncat >/dev/null\nprintf '%s\\n' '{\"kind\":\"session_stopped\"}'\n",
        );
        executable(
            &root.join("capture"),
            "#!/bin/sh\nprintf '%s\\n' '{\"kind\":\"capture_ready\"}' >&2\nprintf '\\000\\000'\ncat >/dev/null\n",
        );
        let scheduler = StageScheduler::new(2);
        let service = OnlineSessionService::new(scheduler.clone());
        let runtime = OnlineRuntime {
            python: root.join("python"),
            worker_root: root.clone(),
            capture: root.join("capture"),
            translator: None,
        };
        let models = OnlineModels {
            language: "ja".into(),
            draft_model: root.clone(),
            final_model: root.clone(),
            vad_model: root.join("vad.onnx"),
            translation_url: None,
            translation_drafts: true,
            translation_context: Value::Null,
        };
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = events.clone();
        let sink: EventSink = Arc::new(move |event| recorded.lock().unwrap().push(event));
        let busy = scheduler.acquire_asr().await;
        assert!(
            service
                .start(runtime.clone(), models.clone(), sink.clone())
                .await
                .is_err()
        );
        assert_eq!(service.status().phase, "idle");
        drop(busy);
        service
            .start(runtime.clone(), models.clone(), sink.clone())
            .await
            .unwrap();
        timeout(Duration::from_secs(5), async {
            while service.status().phase != "running" {
                assert_ne!(service.status().phase, "failed", "{:?}", service.status());
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        // The capture control pipe must remain open while the owner polls
        // child exit; a ready event alone does not prove an active session.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(service.status().phase, "running", "{:?}", service.status());
        assert!(scheduler.try_acquire_device().is_none());
        assert!(service.start(runtime, models, sink).await.is_err());
        let result = service.stop().await.unwrap();
        assert_eq!(result.phase, "stopped", "{:?}", result.error);
        assert!(scheduler.try_acquire_device().is_some());
        let events = events.lock().unwrap();
        let kinds: Vec<_> = events
            .iter()
            .filter_map(|e| e.event["kind"].as_str())
            .collect();
        assert_eq!(
            kinds,
            [
                "session_ready",
                "capture_ready",
                "session_stopped",
                "session_state"
            ]
        );
        assert!(
            events
                .iter()
                .all(|e| Some(&e.session_id) == result.session_id.as_ref())
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 4);
        std::fs::remove_dir_all(root).unwrap();
    }
}
