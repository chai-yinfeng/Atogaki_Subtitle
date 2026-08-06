use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
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
}

impl ModelDownloadService {
    pub fn new(models_directory: PathBuf, settings: DesktopSettingsService) -> Result<Self> {
        std::fs::create_dir_all(&models_directory).with_context(|| {
            format!(
                "failed to create managed model directory {}",
                models_directory.display()
            )
        })?;
        cleanup_partial_downloads(&models_directory)?;
        Ok(Self {
            models_directory,
            settings,
            states: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn catalog(&self) -> Vec<ModelCatalogItem> {
        model_catalog().to_vec()
    }

    pub async fn states(&self) -> Vec<ModelDownloadState> {
        self.states.lock().await.values().cloned().collect()
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
            .any(|state| matches!(state.status.as_str(), "queued" | "downloading"))
        {
            bail!("another model download is already running");
        }
        let state = ModelDownloadState {
            model_id: model.id.to_string(),
            status: "queued".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            path: None,
            error: None,
            source: None,
        };
        states.insert(model.id.to_string(), state.clone());
        drop(states);

        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service.download(model.clone()).await {
                let part_path = service.partial_path(&model);
                let _ = fs::remove_file(part_path).await;
                service
                    .update_state(model.id, |state| {
                        state.status = "failed".to_string();
                        state.error = Some(format!("{error:#}"));
                    })
                    .await;
            }
        });
        Ok(state)
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
            self.update_state(model.id, |state| {
                state.source = Some(source.label.to_string());
                state.downloaded_bytes = 0;
                state.total_bytes = None;
            })
            .await;
            match self
                .download_from_source(&client, &model, &source, &output_path)
                .await
            {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let _ = fs::remove_file(self.partial_path(&model)).await;
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
        let response = client
            .get(&source.url)
            .send()
            .await
            .with_context(|| format!("无法连接 {}", source.label))?;
        let status = response.status();
        if !status.is_success() {
            bail!("服务器返回 {status}");
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
        let total_bytes = response.content_length();
        self.update_state(model.id, |state| state.total_bytes = total_bytes)
            .await;

        let part_path = self.partial_path(&model);
        let _ = fs::remove_file(&part_path).await;
        let mut output = fs::File::create(&part_path)
            .await
            .with_context(|| format!("failed to create {}", part_path.display()))?;
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = 0_u64;
        let mut hasher = Sha256::new();
        while let Some(chunk) = stream.next().await {
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
            bail!("SHA-256 校验失败，期望 {}，实际 {actual}", model.sha256);
        }
        fs::rename(&part_path, &output_path)
            .await
            .with_context(|| format!("failed to install {}", output_path.display()))?;
        self.settings
            .set_downloaded_model(model.kind, &output_path)
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
                error: Some(error.to_string()),
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

fn cleanup_partial_downloads(models_directory: &Path) -> Result<()> {
    for entry in std::fs::read_dir(models_directory)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("part") {
            std::fs::remove_file(&path).with_context(|| {
                format!(
                    "failed to clean interrupted model download {}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn model_catalog() -> &'static [ModelCatalogItem] {
    &[
        ModelCatalogItem {
            id: "whisper-small",
            kind: "whisper",
            name: "Whisper small（轻量）",
            file_name: "ggml-small.bin",
            size_label: "约 466 MiB",
            recommended_for: "8 GB 内存或希望更快完成；日语准确率低于 medium。",
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
            recommended_for: "16 GB 及以上内存；当前日语节目质量基线。",
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
            recommended_for: "现代 Apple Silicon；更高质量上限，建议与 medium 做同节目对比。",
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
            recommended_for: "现代 Apple Silicon / GPU；需与 medium 做真实节目质量对比。",
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
            recommended_for: "turbo 的较高精度量化档；适合与 turbo q5 比较质量差异。",
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
    ]
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{cleanup_partial_downloads, download_sources, model_catalog};

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
    fn startup_cleanup_only_removes_partial_downloads() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-model-cleanup-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let partial = root.join("ggml-medium.bin.part");
        let installed = root.join("ggml-small.bin");
        fs::write(&partial, b"incomplete").unwrap();
        fs::write(&installed, b"installed").unwrap();

        cleanup_partial_downloads(&root).unwrap();

        assert!(!partial.exists());
        assert!(installed.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
