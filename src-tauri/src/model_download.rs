use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

use crate::desktop_settings::DesktopSettingsService;
use atogaki_subtitle::infrastructure::network::{NetworkClientConfig, normalize_https_endpoint};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogItem {
    pub id: &'static str,
    pub kind: &'static str,
    pub name: &'static str,
    pub file_name: &'static str,
    pub size_label: &'static str,
    pub recommended_for: &'static str,
    pub source_url: &'static str,
    #[serde(skip)]
    download_path: &'static str,
    #[serde(skip)]
    sha256: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadState {
    pub model_id: String,
    pub status: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub path: Option<String>,
    pub error: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSourceCheck {
    pub label: String,
    pub requested_url: String,
    pub resolved_host: Option<String>,
    pub status: Option<u16>,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct DownloadSource {
    label: &'static str,
    url: String,
}

#[derive(Debug, Clone)]
pub struct ModelDownloadService {
    models_directory: PathBuf,
    settings: DesktopSettingsService,
    states: Arc<Mutex<HashMap<String, ModelDownloadState>>>,
    cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ModelDownloadService {
    pub fn new(models_directory: PathBuf, settings: DesktopSettingsService) -> Result<Self> {
        std::fs::create_dir_all(&models_directory).with_context(|| {
            format!(
                "failed to create managed model directory {}",
                models_directory.display()
            )
        })?;
        Ok(Self {
            models_directory,
            settings,
            states: Arc::new(Mutex::new(HashMap::new())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn catalog(&self) -> Vec<ModelCatalogItem> {
        model_catalog().to_vec()
    }

    pub async fn states(&self) -> Vec<ModelDownloadState> {
        let mut states = self.states.lock().await.clone();
        for model in model_catalog() {
            let path = self.models_directory.join(model.file_name);
            if path.is_file() {
                states
                    .entry(model.id.to_string())
                    .or_insert_with(|| ModelDownloadState {
                        model_id: model.id.to_string(),
                        status: "done".to_string(),
                        downloaded_bytes: std::fs::metadata(&path)
                            .map(|metadata| metadata.len())
                            .unwrap_or_default(),
                        total_bytes: None,
                        path: Some(path.display().to_string()),
                        error: None,
                        source: Some("本地已安装".to_string()),
                    });
            }
        }
        let mut states = states.into_values().collect::<Vec<_>>();
        states.sort_by(|left, right| left.model_id.cmp(&right.model_id));
        states
    }

    pub async fn start(&self, model_id: &str) -> Result<ModelDownloadState> {
        let model = model_catalog()
            .iter()
            .find(|model| model.id == model_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown model: {model_id}"))?;
        let mut states = self.states.lock().await;
        if states
            .values()
            .any(|state| {
                matches!(
                    state.status.as_str(),
                    "queued" | "downloading" | "cancelling"
                )
            })
        {
            bail!("another model download is already running");
        }
        let downloaded_bytes = fs::metadata(self.partial_path(&model))
            .await
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let state = ModelDownloadState {
            model_id: model.id.to_string(),
            status: "queued".to_string(),
            downloaded_bytes,
            total_bytes: None,
            path: None,
            error: None,
            source: None,
        };
        states.insert(model.id.to_string(), state.clone());
        drop(states);
        self.cancellations
            .lock()
            .await
            .insert(model.id.to_string(), Arc::new(AtomicBool::new(false)));

        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service.download(model.clone()).await {
                let cancelled = service.is_cancelled(model.id).await;
                service
                    .update_state(model.id, |state| {
                        state.status = if cancelled { "cancelled" } else { "failed" }.to_string();
                        state.error = (!cancelled).then(|| format!("{error:#}"));
                    })
                    .await;
            }
            service.cancellations.lock().await.remove(model.id);
        });
        Ok(state)
    }

    pub async fn cancel(&self, model_id: &str) -> Result<ModelDownloadState> {
        let cancellation = self
            .cancellations
            .lock()
            .await
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("model download is not active: {model_id}"))?;
        cancellation.store(true, Ordering::SeqCst);
        self.update_state(model_id, |state| {
            state.status = "cancelling".to_string();
            state.error = None;
        })
        .await;
        self.states
            .lock()
            .await
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("model download state disappeared: {model_id}"))
    }

    pub async fn uninstall(&self, model_id: &str) -> Result<()> {
        if self.cancellations.lock().await.contains_key(model_id) {
            bail!("cancel the active download before uninstalling {model_id}");
        }
        let model = model_catalog()
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| anyhow!("unknown model: {model_id}"))?;
        let output_path = self.models_directory.join(model.file_name);
        let partial_path = self.partial_path(model);
        if output_path.is_file() {
            fs::remove_file(&output_path)
                .await
                .with_context(|| format!("failed to uninstall {}", output_path.display()))?;
        }
        if partial_path.is_file() {
            fs::remove_file(&partial_path)
                .await
                .with_context(|| format!("failed to remove {}", partial_path.display()))?;
        }
        self.settings
            .clear_downloaded_model(model.kind, &output_path)
            .await?;
        self.states.lock().await.remove(model_id);
        Ok(())
    }

    async fn download(&self, model: ModelCatalogItem) -> Result<()> {
        let output_path = self.models_directory.join(model.file_name);
        if output_path.is_file() {
            if sha256_file(&output_path).await? == model.sha256 {
                self.settings
                    .set_downloaded_model(model.kind, &output_path)
                    .await?;
                self.update_state(model.id, |state| {
                    state.status = "done".to_string();
                    state.path = Some(output_path.display().to_string());
                    state.source = Some("本地已校验文件".to_string());
                })
                .await;
                return Ok(());
            }
            fs::remove_file(&output_path)
                .await
                .with_context(|| format!("failed to replace invalid {}", output_path.display()))?;
        }

        let network = self.settings.download_network_settings().await?;
        let client = build_download_client(&network.client)?;
        let sources = download_sources(&model, network.model_mirror_url.as_deref());
        self.update_state(model.id, |state| {
            state.status = "downloading".to_string();
        })
        .await;
        let mut failures = Vec::new();
        for source in sources {
            self.ensure_not_cancelled(model.id).await?;
            self.update_state(model.id, |state| {
                state.source = Some(source.label.to_string());
                state.total_bytes = None;
            })
            .await;
            match self
                .download_from_source(&client, &model, &source, &output_path)
                .await
            {
                Ok(()) => return Ok(()),
                Err(error) => {
                    failures.push(format!("{}：{error:#}", source.label));
                }
            }
        }
        bail!("所有模型下载来源均失败：{}", failures.join("；"))
    }

    async fn download_from_source(
        &self,
        client: &reqwest::Client,
        model: &ModelCatalogItem,
        source: &DownloadSource,
        output_path: &Path,
    ) -> Result<()> {
        let part_path = self.partial_path(model);
        let existing_bytes = fs::metadata(&part_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let mut request = client
            .get(&source.url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity");
        if existing_bytes > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={existing_bytes}-"));
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("无法连接 {}", source.label))?;
        let status = response.status();
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && existing_bytes > 0 {
            if sha256_file(&part_path).await? == model.sha256 {
                return self.install_verified_download(model, &part_path, output_path).await;
            }
            fs::remove_file(&part_path).await.with_context(|| {
                format!("failed to discard invalid partial model {}", part_path.display())
            })?;
            bail!("服务器拒绝续传，且现有临时文件未通过完整校验；下次将重新下载");
        }
        if !status.is_success() {
            bail!("服务器返回 {status}");
        }
        let resumed = existing_bytes > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if resumed {
            validate_content_range(response.headers(), existing_bytes)?;
        }
        let resolved_host = response
            .url()
            .host_str()
            .map(str::to_string)
            .unwrap_or_else(|| "未知主机".to_string());
        self.update_state(model.id, |state| {
            state.source = Some(format!("{} · {resolved_host}", source.label));
        })
        .await;
        let total_bytes = if resumed {
            content_range_total(response.headers()).or_else(|| {
                response
                    .content_length()
                    .map(|remaining| existing_bytes.saturating_add(remaining))
            })
        } else {
            response.content_length()
        };
        self.update_state(model.id, |state| state.total_bytes = total_bytes)
            .await;

        let mut options = fs::OpenOptions::new();
        options.create(true).write(true);
        if resumed {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut output = options.open(&part_path)
            .await
            .with_context(|| format!("failed to create {}", part_path.display()))?;
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = if resumed { existing_bytes } else { 0 };
        self.update_state(model.id, |state| {
            state.downloaded_bytes = downloaded_bytes;
        })
        .await;
        let mut hasher = Sha256::new();
        if resumed {
            update_sha256_from_file(&part_path, &mut hasher).await?;
        }
        while let Some(chunk) = stream.next().await {
            self.ensure_not_cancelled(model.id).await?;
            let chunk = chunk.context("model download was interrupted")?;
            output
                .write_all(&chunk)
                .await
                .context("failed to write model download")?;
            hasher.update(&chunk);
            downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
            self.update_state(model.id, |state| {
                state.downloaded_bytes = downloaded_bytes;
            })
            .await;
        }
        output
            .flush()
            .await
            .context("failed to flush model download")?;
        drop(output);

        if downloaded_bytes == 0 {
            bail!("下载来源返回了空文件");
        }
        let actual = format!("{:x}", hasher.finalize());
        if actual != model.sha256 {
            fs::remove_file(&part_path).await.with_context(|| {
                format!("failed to discard corrupt model download {}", part_path.display())
            })?;
            bail!("SHA-256 校验失败，期望 {}，实际 {actual}", model.sha256);
        }
        self.ensure_not_cancelled(model.id).await?;
        self.install_verified_download(model, &part_path, output_path).await
    }

    async fn install_verified_download(
        &self,
        model: &ModelCatalogItem,
        part_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        let downloaded_bytes = fs::metadata(part_path).await?.len();
        fs::rename(part_path, output_path)
            .await
            .with_context(|| format!("failed to install {}", output_path.display()))?;
        self.settings
            .set_downloaded_model(model.kind, output_path)
            .await?;
        self.update_state(model.id, |state| {
            state.status = "done".to_string();
            state.downloaded_bytes = downloaded_bytes;
            state.path = Some(output_path.display().to_string());
        })
        .await;
        Ok(())
    }

    async fn update_state(&self, model_id: &str, update: impl FnOnce(&mut ModelDownloadState)) {
        if let Some(state) = self.states.lock().await.get_mut(model_id) {
            update(state);
        }
    }

    fn partial_path(&self, model: &ModelCatalogItem) -> PathBuf {
        self.models_directory
            .join(format!("{}.part", model.file_name))
    }

    async fn is_cancelled(&self, model_id: &str) -> bool {
        self.cancellations
            .lock()
            .await
            .get(model_id)
            .is_some_and(|cancelled| cancelled.load(Ordering::SeqCst))
    }

    async fn ensure_not_cancelled(&self, model_id: &str) -> Result<()> {
        if self.is_cancelled(model_id).await {
            bail!("model download was cancelled");
        }
        Ok(())
    }
}

pub async fn test_download_network(
    proxy_mode: &str,
    proxy_url: Option<String>,
    model_mirror_url: Option<String>,
) -> Result<Vec<NetworkSourceCheck>> {
    let network = NetworkClientConfig::new(proxy_mode, proxy_url)?;
    let mirror = normalize_https_endpoint(model_mirror_url)?;
    let client = build_download_client(&network)?;
    let model = model_catalog()
        .iter()
        .find(|model| model.id == "vad-silero-v6.2.0")
        .expect("VAD model must remain in the catalog");
    let mut checks = Vec::new();
    for source in download_sources(model, mirror.as_deref()) {
        let result = client
            .get(&source.url)
            .header(reqwest::header::RANGE, "bytes=0-0")
            .send()
            .await;
        checks.push(match result {
            Ok(response) => NetworkSourceCheck {
                label: source.label.to_string(),
                requested_url: source.url,
                resolved_host: response.url().host_str().map(str::to_string),
                status: Some(response.status().as_u16()),
                ok: response.status().is_success(),
                error: (!response.status().is_success())
                    .then(|| format!("服务器返回 {}", response.status())),
            },
            Err(error) => NetworkSourceCheck {
                label: source.label.to_string(),
                requested_url: source.url,
                resolved_host: None,
                status: None,
                ok: false,
                error: Some(format!("{error}; {}", network.failure_guidance())),
            },
        });
    }
    Ok(checks)
}

fn build_download_client(network: &NetworkClientConfig) -> Result<reqwest::Client> {
    network
        .apply(
            reqwest::Client::builder()
                .user_agent("Atogaki/0.1 model downloader")
                .connect_timeout(Duration::from_secs(20))
                .read_timeout(Duration::from_secs(60)),
        )?
        .build()
        .context("failed to build model download client")
}

fn download_sources(
    model: &ModelCatalogItem,
    model_mirror_url: Option<&str>,
) -> Vec<DownloadSource> {
    let official_url = format!("https://huggingface.co/{}", model.download_path);
    let mut sources = Vec::new();
    if let Some(mirror) = model_mirror_url {
        let mirror_url = format!("{}/{}", mirror.trim_end_matches('/'), model.download_path);
        if mirror_url != official_url {
            sources.push(DownloadSource {
                label: "自定义镜像",
                url: mirror_url,
            });
        }
    }
    sources.push(DownloadSource {
        label: "Hugging Face 官方源",
        url: official_url,
    });
    sources
}

async fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {} for verification", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("failed to verify {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn update_sha256_from_file(path: &Path, hasher: &mut Sha256) -> Result<()> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("failed to open partial download {}", path.display()))?;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(())
}

fn validate_content_range(headers: &reqwest::header::HeaderMap, expected_start: u64) -> Result<()> {
    let value = headers
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| anyhow!("续传响应缺少 Content-Range"))?;
    let range = value
        .strip_prefix("bytes ")
        .and_then(|value| value.split('/').next())
        .ok_or_else(|| anyhow!("无法解析续传范围：{value}"))?;
    let start = range
        .split('-')
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| anyhow!("无法解析续传起点：{value}"))?;
    if start != expected_start {
        bail!("服务器从 {start} 字节续传，但本地需要从 {expected_start} 字节继续");
    }
    Ok(())
}

fn content_range_total(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .split('/')
        .nth(1)?
        .parse()
        .ok()
}

fn model_catalog() -> &'static [ModelCatalogItem] {
    &[
        ModelCatalogItem {
            id: "whisper-small",
            kind: "whisper",
            name: "Whisper small（轻量）",
            file_name: "ggml-small.bin",
            size_label: "约 466 MiB",
            recommended_for: "8 GB 内存或希望更快完成；适合短媒体和初步识别。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_path: "ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        },
        ModelCatalogItem {
            id: "whisper-medium",
            kind: "whisper",
            name: "Whisper medium（推荐）",
            file_name: "ggml-medium.bin",
            size_label: "约 1.5 GiB",
            recommended_for: "16 GB 及以上内存；适合作为日语节目的常用选择。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_path: "ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
        },
        ModelCatalogItem {
            id: "whisper-large-v3-q5_0",
            kind: "whisper",
            name: "Whisper large-v3 q5（质量实验）",
            file_name: "ggml-large-v3-q5_0.bin",
            size_label: "约 1.1 GiB",
            recommended_for: "现代 Apple Silicon；适合优先追求识别准确性。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_path: "ggerganov/whisper.cpp/resolve/main/ggml-large-v3-q5_0.bin",
            sha256: "d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1",
        },
        ModelCatalogItem {
            id: "whisper-large-v3-turbo-q5_0",
            kind: "whisper",
            name: "Whisper large-v3-turbo q5（实验）",
            file_name: "ggml-large-v3-turbo-q5_0.bin",
            size_label: "约 547 MiB",
            recommended_for: "现代 Apple Silicon / GPU；速度与磁盘占用较低。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_path: "ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
            sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        },
        ModelCatalogItem {
            id: "whisper-large-v3-turbo-q8_0",
            kind: "whisper",
            name: "Whisper large-v3-turbo q8（质量实验）",
            file_name: "ggml-large-v3-turbo-q8_0.bin",
            size_label: "约 834 MiB",
            recommended_for: "turbo 的较高精度量化档；适合优先追求 turbo 的识别质量。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_path: "ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin",
            sha256: "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
        },
        ModelCatalogItem {
            id: "vad-silero-v6.2.0",
            kind: "vad",
            name: "Silero VAD v6.2.0（推荐）",
            file_name: "ggml-silero-v6.2.0.bin",
            size_label: "约 865 KiB",
            recommended_for: "过滤静音、音乐和环境声，建议与任一 Whisper 模型配套。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/download-vad-model.sh",
            download_path: "ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin",
            sha256: "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987",
        },
        ModelCatalogItem {
            id: "hy-mt2-1.8b-q4-k-m",
            kind: "hy-mt2",
            name: "Hy-MT2 1.8B Q4_K_M（本地翻译候选）",
            file_name: "Hy-MT2-1.8B-Q4_K_M.gguf",
            size_label: "约 1.06 GiB",
            recommended_for: "速度与内存占用较低；真实盲评完成前不标记为推荐。",
            source_url: "https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF",
            download_path: "tencent/Hy-MT2-1.8B-GGUF/resolve/a0c709d9fac510f2c807aa3af52872340dc37a4a/Hy-MT2-1.8B-Q4_K_M.gguf",
            sha256: "dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699",
        },
        ModelCatalogItem {
            id: "hy-mt2-7b-q4-k-m",
            kind: "hy-mt2",
            name: "Hy-MT2 7B Q4_K_M（本地翻译候选）",
            file_name: "Hy-MT2-7B-Q4_K_M.gguf",
            size_label: "约 4.31 GiB",
            recommended_for: "质量候选，当前 Apple Silicon 实测约比 1.8B 慢 5.3 倍。",
            source_url: "https://huggingface.co/tencent/Hy-MT2-7B-GGUF",
            download_path: "tencent/Hy-MT2-7B-GGUF/resolve/ab8472660ac61fac25f1af43fac2599d52a8a775/Hy-MT2-7B-Q4_K_M.gguf",
            sha256: "9f96256500f3fc1ab4d64336b58f52a949a95ad7516b0c229476eef782f9f77b",
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use atogaki_subtitle::{
        application::{MutableTranslationProvider, UnconfiguredTranslationProvider},
        infrastructure::local_db::LocalDatabase,
    };
    use reqwest::header::{CONTENT_RANGE, HeaderMap, HeaderValue};

    use super::{
        ModelDownloadService, content_range_total, download_sources, model_catalog,
        validate_content_range,
    };
    use crate::desktop_settings::DesktopSettingsService;

    #[test]
    fn catalog_uses_unique_ids_and_https_downloads() {
        let catalog = model_catalog();
        let mut ids = catalog.iter().map(|model| model.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), catalog.len());
        assert!(
            catalog
                .iter()
                .all(|model| !model.download_path.starts_with('/') && model.sha256.len() == 64)
        );
        assert!(
            catalog
                .iter()
                .all(|model| { model.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) })
        );
        assert!(catalog.iter().any(|model| model.kind == "vad"));
    }

    #[test]
    fn curated_whisper_catalog_has_the_supported_quality_tiers() {
        let whisper_ids = model_catalog()
            .iter()
            .filter(|model| model.kind == "whisper")
            .map(|model| model.id)
            .collect::<Vec<_>>();

        assert_eq!(
            whisper_ids,
            vec![
                "whisper-small",
                "whisper-medium",
                "whisper-large-v3-q5_0",
                "whisper-large-v3-turbo-q5_0",
                "whisper-large-v3-turbo-q8_0",
            ]
        );
    }

    #[test]
    fn hy_mt2_catalog_uses_the_validated_revisions_and_hashes() {
        let models = model_catalog()
            .iter()
            .filter(|model| model.kind == "hy-mt2")
            .collect::<Vec<_>>();

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "hy-mt2-1.8b-q4-k-m");
        assert!(models[0].download_path.contains("a0c709d9fac510f2c807aa3af52872340dc37a4a"));
        assert_eq!(
            models[0].sha256,
            "dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699"
        );
        assert_eq!(models[1].id, "hy-mt2-7b-q4-k-m");
        assert!(models[1].download_path.contains("ab8472660ac61fac25f1af43fac2599d52a8a775"));
        assert_eq!(
            models[1].sha256,
            "9f96256500f3fc1ab4d64336b58f52a949a95ad7516b0c229476eef782f9f77b"
        );
    }

    #[test]
    fn custom_mirror_precedes_the_canonical_hugging_face_source() {
        let model = &model_catalog()[0];
        let sources = download_sources(model, Some("https://mirror.example/hf"));
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].label, "自定义镜像");
        assert!(sources[0].url.starts_with("https://mirror.example/hf/"));
        assert_eq!(sources[1].label, "Hugging Face 官方源");
        assert!(sources[1].url.starts_with("https://huggingface.co/"));
    }

    #[test]
    fn content_range_must_continue_at_the_local_file_boundary() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 1024-2047/4096"));
        validate_content_range(&headers, 1024).unwrap();
        assert_eq!(content_range_total(&headers), Some(4096));
        assert!(validate_content_range(&headers, 512).is_err());
    }

    #[tokio::test]
    async fn uninstall_removes_a_managed_model_and_its_active_setting() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-model-uninstall-test-{nonce}"));
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let settings = DesktopSettingsService::new(
            database.clone(),
            MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider)),
            models.clone(),
            None,
            None,
            None,
            None,
        );
        let model_path = models.join("Hy-MT2-1.8B-Q4_K_M.gguf");
        fs::write(&model_path, b"verified model placeholder").unwrap();
        settings
            .set_downloaded_model("hy-mt2", &model_path)
            .await
            .unwrap();
        let service = ModelDownloadService::new(models, settings).unwrap();

        assert!(
            service
                .states()
                .await
                .iter()
                .any(|state| state.model_id == "hy-mt2-1.8b-q4-k-m")
        );
        service
            .uninstall("hy-mt2-1.8b-q4-k-m")
            .await
            .unwrap();

        assert!(!model_path.exists());
        assert!(
            database
                .get_setting("translation.hy_mt2_model_path")
                .await
                .unwrap()
                .is_none()
        );
        assert!(service.states().await.is_empty());
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
