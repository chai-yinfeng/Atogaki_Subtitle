use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use tokio::{process::Child, sync::Mutex, time::sleep};
use uuid::Uuid;

use crate::{
    application::{
        TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
    },
    infrastructure::{
        child_process::sidecar_command,
        network::NetworkClientConfig,
        openai_compatible::{
            OpenAiCompatibleConfig, OpenAiCompatibleTranslationProvider, OpenAiGenerationConfig,
        },
    },
};

const STARTUP_ATTEMPTS: usize = 240;
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct LocalLlamaConfig {
    pub runtime_path: PathBuf,
    pub model_path: PathBuf,
    pub model_id: String,
    pub provider_name: String,
    pub style_instruction: String,
    pub context_size: u32,
    pub gpu_layers: u32,
}

impl LocalLlamaConfig {
    pub fn hy_mt2(runtime_path: PathBuf, model_path: PathBuf, style_instruction: String) -> Self {
        let model_id = model_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Hy-MT2")
            .to_string();
        Self {
            runtime_path,
            model_path,
            model_id,
            provider_name: "Hy-MT2（本地实验）".to_string(),
            style_instruction,
            context_size: 8_192,
            gpu_layers: 99,
        }
    }
}

struct LocalLlamaRuntime {
    config: LocalLlamaConfig,
    address: OnceLock<SocketAddr>,
    api_key: String,
    client: reqwest::Client,
    child: Mutex<Option<Child>>,
}

impl fmt::Debug for LocalLlamaRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalLlamaRuntime")
            .field("runtime_path", &self.config.runtime_path)
            .field("model_path", &self.config.model_path)
            .field("address", &self.address.get())
            .finish_non_exhaustive()
    }
}

impl LocalLlamaRuntime {
    fn new(config: LocalLlamaConfig) -> Result<Self> {
        Ok(Self {
            config,
            address: OnceLock::new(),
            api_key: Uuid::new_v4().to_string(),
            client: reqwest::Client::builder()
                .no_proxy()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(5))
                .build()
                .context("failed to build local llama health client")?,
            child: Mutex::new(None),
        })
    }

    fn base_url(&self) -> Result<String> {
        Ok(format!(
            "http://{}/v1",
            self.address
                .get()
                .ok_or_else(|| anyhow!("local llama runtime has not reserved an address"))?
        ))
    }

    async fn ensure_started(&self) -> Result<()> {
        validate_runtime_file(&self.config.runtime_path, "llama-server runtime")?;
        validate_runtime_file(&self.config.model_path, "Hy-MT2 model")?;

        let mut child_guard = self.child.lock().await;
        let address = if let Some(address) = self.address.get() {
            *address
        } else {
            let address = reserve_loopback_address()?;
            let _ = self.address.set(address);
            address
        };
        if let Some(child) = child_guard.as_mut() {
            match child.try_wait().context("failed to inspect llama-server")? {
                None if self.health_ready().await => return Ok(()),
                None => {
                    child
                        .kill()
                        .await
                        .context("failed to stop unresponsive llama-server")?;
                }
                Some(_) => {}
            }
            *child_guard = None;
        }

        let mut command = sidecar_command(&self.config.runtime_path);
        command
            .arg("--host")
            .arg(address.ip().to_string())
            .arg("--port")
            .arg(address.port().to_string())
            .arg("--model")
            .arg(&self.config.model_path)
            .arg("--ctx-size")
            .arg(self.config.context_size.to_string())
            .arg("--jinja")
            .arg("--n-gpu-layers")
            .arg(self.config.gpu_layers.to_string())
            .arg("--api-key")
            .arg(&self.api_key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let child = command.spawn().with_context(|| {
            format!(
                "failed to start managed llama-server at {}",
                self.config.runtime_path.display()
            )
        })?;
        *child_guard = Some(child);

        for _ in 0..STARTUP_ATTEMPTS {
            let child = child_guard
                .as_mut()
                .expect("managed llama-server child must exist during startup");
            if let Some(status) = child
                .try_wait()
                .context("failed to inspect llama-server startup")?
            {
                *child_guard = None;
                bail!("managed llama-server exited during startup with {status}");
            }
            if self.health_ready().await {
                return Ok(());
            }
            sleep(STARTUP_POLL_INTERVAL).await;
        }

        if let Some(child) = child_guard.as_mut() {
            let _ = child.kill().await;
        }
        *child_guard = None;
        bail!("managed llama-server did not become ready within 60 seconds")
    }

    async fn health_ready(&self) -> bool {
        self.client
            .get(format!(
                "http://{}/health",
                self.address
                    .get()
                    .expect("health check requires a reserved address")
            ))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
    }

    async fn shutdown(&self) -> Result<()> {
        if let Some(mut child) = self.child.lock().await.take() {
            child
                .kill()
                .await
                .context("failed to stop managed llama-server")?;
            let _ = child.wait().await;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct LocalLlamaTranslationProvider {
    runtime: Arc<LocalLlamaRuntime>,
}

impl LocalLlamaTranslationProvider {
    pub fn new(config: LocalLlamaConfig) -> Result<Self> {
        Ok(Self {
            runtime: Arc::new(LocalLlamaRuntime::new(config)?),
        })
    }

    #[cfg(test)]
    fn with_address(config: LocalLlamaConfig, address: SocketAddr) -> Result<Self> {
        let runtime = LocalLlamaRuntime::new(config)?;
        runtime
            .address
            .set(address)
            .map_err(|_| anyhow!("local llama address was already assigned"))?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    fn request_provider(&self) -> Result<OpenAiCompatibleTranslationProvider> {
        OpenAiCompatibleTranslationProvider::with_network_config(
            OpenAiCompatibleConfig {
                provider_id: "hy-mt2-local".to_string(),
                provider_name: self.runtime.config.provider_name.clone(),
                api_key: Some(self.runtime.api_key.clone()),
                base_url: self.runtime.base_url()?,
                model: self.runtime.config.model_id.clone(),
                style_instruction: self.runtime.config.style_instruction.clone(),
                disable_deepseek_thinking: false,
                generation: OpenAiGenerationConfig {
                    temperature: Some(0.7),
                    top_p: Some(0.6),
                    top_k: Some(20),
                    repetition_penalty: Some(1.05),
                    max_tokens: Some(2_048),
                    strict_json_schema: true,
                },
            },
            &NetworkClientConfig::new("direct", None)?,
        )
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.runtime.shutdown().await
    }
}

impl fmt::Debug for LocalLlamaTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalLlamaTranslationProvider")
            .field("runtime", &self.runtime)
            .finish()
    }
}

impl TranslationProvider for LocalLlamaTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        let runtime_ready = self.runtime.config.runtime_path.is_file();
        let model_ready = self.runtime.config.model_path.is_file();
        TranslationProviderStatus {
            id: "hy-mt2-local".to_string(),
            name: self.runtime.config.provider_name.clone(),
            configured: runtime_ready && model_ready,
            model: Some(self.runtime.config.model_id.clone()),
            endpoint_kind: "managed-loopback-openai-compatible".to_string(),
            configuration_hint: (!(runtime_ready && model_ready)).then(|| {
                if !runtime_ready {
                    "当前版本缺少内置 llama-server runtime。".to_string()
                } else {
                    "请先下载一个 Hy-MT2 模型。".to_string()
                }
            }),
        }
    }

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async move {
            self.runtime.ensure_started().await?;
            self.request_provider()?.translate(request).await
        })
    }
}

fn validate_runtime_file(path: &Path, label: &str) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(anyhow!("{label} does not exist: {}", path.display()))
    }
}

fn reserve_loopback_address() -> Result<SocketAddr> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .context("failed to reserve a loopback port for llama-server")?;
    let port = listener.local_addr()?.port();
    Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::{LocalLlamaConfig, LocalLlamaTranslationProvider};
    use crate::{
        application::{
            TranslationOptions, TranslationProvider, TranslationRequest, TranslationTargetSegment,
        },
        domain::LanguageCode,
    };

    #[test]
    fn missing_managed_files_leave_local_provider_unconfigured() {
        let provider = LocalLlamaTranslationProvider::with_address(
            LocalLlamaConfig::hy_mt2(
                "missing-llama-server".into(),
                "missing-hy-mt2.gguf".into(),
                "自然字幕".to_string(),
            ),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 18_080),
        )
        .unwrap();

        let status = provider.status();
        assert_eq!(status.id, "hy-mt2-local");
        assert!(!status.configured);
        assert_eq!(status.endpoint_kind, "managed-loopback-openai-compatible");
        assert!(status.configuration_hint.unwrap().contains("runtime"));
    }

    #[tokio::test]
    #[ignore = "requires pinned llama-server and Hy-MT2 model paths"]
    async fn managed_runtime_translates_and_shuts_down() {
        let provider = LocalLlamaTranslationProvider::new(LocalLlamaConfig::hy_mt2(
            std::env::var("ATOGAKI_TEST_LLAMA_SERVER").unwrap().into(),
            std::env::var("ATOGAKI_TEST_HY_MT2_MODEL").unwrap().into(),
            "准确、自然、简洁的中文口语字幕".to_string(),
        ))
        .unwrap();
        let response = provider
            .translate(TranslationRequest {
                options: TranslationOptions::new(
                    LanguageCode::Japanese,
                    LanguageCode::SimplifiedChinese,
                ),
                before_context: Vec::new(),
                targets: vec![TranslationTargetSegment {
                    segment_id: "cue-1".to_string(),
                    source_text: "こんばんは。".to_string(),
                }],
                after_context: Vec::new(),
                style_instruction: None,
            })
            .await
            .unwrap();

        assert_eq!(response.translations.len(), 1);
        assert_eq!(response.translations[0].segment_id, "cue-1");
        assert!(!response.translations[0].translated_text.trim().is_empty());
        provider.shutdown().await.unwrap();
    }
}
