use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use serde::Serialize;
use sha1::{Digest, Sha1};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};

use crate::desktop_settings::DesktopSettingsService;

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
    download_url: &'static str,
    #[serde(skip)]
    sha1: Option<&'static str>,
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
}

#[derive(Debug, Clone)]
pub struct ModelDownloadService {
    client: reqwest::Client,
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
            client: reqwest::Client::builder()
                .user_agent("Atogaki/0.1 model downloader")
                .connect_timeout(Duration::from_secs(20))
                .read_timeout(Duration::from_secs(60))
                .build()
                .context("failed to build model download client")?,
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
            self.settings
                .set_downloaded_model(model.kind, &output_path)
                .await?;
            self.update_state(model.id, |state| {
                state.status = "done".to_string();
                state.path = Some(output_path.display().to_string());
            })
            .await;
            return Ok(());
        }

        self.update_state(model.id, |state| {
            state.status = "downloading".to_string();
        })
        .await;
        let response = self
            .client
            .get(model.download_url)
            .send()
            .await
            .context("failed to contact the official model host")?
            .error_for_status()
            .context("official model host rejected the download")?;
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
        let mut hasher = Sha1::new();
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
            bail!("official model host returned an empty file");
        }
        if let Some(expected) = model.sha1 {
            let actual = format!("{:x}", hasher.finalize());
            if actual != expected {
                bail!("model checksum mismatch: expected {expected}, received {actual}");
            }
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
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            sha1: Some("55356645c2b361a969dfd0ef2c5a50d530afd8d5"),
        },
        ModelCatalogItem {
            id: "whisper-medium",
            kind: "whisper",
            name: "Whisper medium（推荐）",
            file_name: "ggml-medium.bin",
            size_label: "约 1.5 GiB",
            recommended_for: "16 GB 及以上内存；当前日语节目质量基线。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            sha1: Some("fd9727b6e1217c2f614f9b698455c4ffd82463b4"),
        },
        ModelCatalogItem {
            id: "whisper-large-v3-turbo-q5_0",
            kind: "whisper",
            name: "Whisper large-v3-turbo q5（实验）",
            file_name: "ggml-large-v3-turbo-q5_0.bin",
            size_label: "约 547 MiB",
            recommended_for: "现代 Apple Silicon / GPU；需与 medium 做真实节目质量对比。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md",
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
            sha1: Some("e050f7970618a659205450ad97eb95a18d69c9ee"),
        },
        ModelCatalogItem {
            id: "vad-silero-v6.2.0",
            kind: "vad",
            name: "Silero VAD v6.2.0（推荐）",
            file_name: "ggml-silero-v6.2.0.bin",
            size_label: "约 865 KiB",
            recommended_for: "过滤静音、音乐和环境声，建议与任一 Whisper 模型配套。",
            source_url: "https://github.com/ggml-org/whisper.cpp/blob/master/models/download-vad-model.sh",
            download_url: "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin",
            sha1: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{cleanup_partial_downloads, model_catalog};

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
                .all(|model| model.download_url.starts_with("https://"))
        );
        assert!(catalog.iter().any(|model| model.kind == "vad"));
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
