use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

use crate::desktop_settings::DesktopSettingsService;
use atogaki_subtitle::infrastructure::network::{NetworkClientConfig, normalize_https_endpoint};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogItem {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub file_name: String,
    pub size_label: String,
    pub recommended_for: String,
    pub source_url: String,
    pub category: String,
    pub family: String,
    pub optional: bool,
    pub selectable: bool,
    pub downloadable: bool,
    pub retired: bool,
    #[serde(skip_serializing)]
    bundle: bool,
    #[serde(skip_serializing)]
    files: Vec<ModelFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelFile {
    name: String,
    download_path: String,
    sha256: String,
    bytes: u64,
}

impl ModelCatalogItem {
    fn total_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }

    fn file_path(&self, root: &Path, file: &ModelFile) -> PathBuf {
        if self.bundle {
            root.join(&file.name)
        } else {
            root.to_path_buf()
        }
    }

    pub fn installed(&self, models_directory: &Path) -> bool {
        self.installed_at(&models_directory.join(&self.file_name))
    }

    pub fn installed_at(&self, root: &Path) -> bool {
        self.files.iter().all(|file| {
            std::fs::metadata(self.file_path(root, file))
                .is_ok_and(|m| m.is_file() && (file.bytes == 0 || m.len() == file.bytes))
        })
    }
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
    pub active: bool,
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

    pub async fn states(&self) -> Result<Vec<ModelDownloadState>> {
        let mut states = self.states.lock().await.clone();
        for model in model_catalog() {
            let path = self.models_directory.join(&model.file_name);
            let active = model.selectable
                && self
                    .settings
                    .downloaded_model_path(&model.kind)
                    .await?
                    .as_deref()
                    == Some(path.as_path());
            if model.installed(&self.models_directory) {
                let state = states
                    .entry(model.id.clone())
                    .or_insert_with(|| ModelDownloadState {
                        model_id: model.id.clone(),
                        status: "done".into(),
                        downloaded_bytes: model
                            .total_bytes()
                            .max(std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)),
                        total_bytes: (model.total_bytes() > 0).then(|| model.total_bytes()),
                        path: Some(path.display().to_string()),
                        error: None,
                        source: Some("本地已安装".into()),
                        active,
                    });
                state.active = active;
            } else if states
                .get(&model.id)
                .is_some_and(|state| state.status == "done")
            {
                states.remove(&model.id);
            }
        }
        let mut states = states.into_values().collect::<Vec<_>>();
        states.sort_by(|left, right| left.model_id.cmp(&right.model_id));
        Ok(states)
    }

    pub async fn start(&self, model_id: &str) -> Result<ModelDownloadState> {
        let model = model_catalog()
            .iter()
            .find(|model| model.id == model_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown model: {model_id}"))?;
        if !model.downloadable {
            bail!("该模型当前随在线环境准备，尚不支持独立下载");
        }
        if model.retired && !model.installed(&self.models_directory) {
            bail!("该模型已从常用目录退出，不再提供新下载");
        }
        let mut states = self.states.lock().await;
        if states.values().any(|state| {
            matches!(
                state.status.as_str(),
                "queued" | "downloading" | "cancelling"
            )
        }) {
            bail!("another model download is already running");
        }
        let state = ModelDownloadState {
            model_id: model.id.clone(),
            status: "queued".into(),
            downloaded_bytes: 0,
            total_bytes: (model.total_bytes() > 0).then(|| model.total_bytes()),
            path: None,
            error: None,
            source: None,
            active: false,
        };
        let mut cancellations = self.cancellations.lock().await;
        if !cancellations.is_empty() {
            bail!("another model download is still finishing");
        }
        cancellations.insert(model.id.clone(), Arc::new(AtomicBool::new(false)));
        drop(cancellations);
        states.insert(model.id.clone(), state.clone());
        drop(states);
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service.download(model.clone()).await {
                let cancelled = service.is_cancelled(&model.id).await;
                service
                    .update_state(&model.id, |state| {
                        state.status = if cancelled { "cancelled" } else { "failed" }.into();
                        state.error = (!cancelled).then(|| format!("{error:#}"));
                    })
                    .await;
            }
            service.cancellations.lock().await.remove(&model.id);
        });
        Ok(state)
    }

    pub async fn cancel(&self, model_id: &str) -> Result<ModelDownloadState> {
        let mut states = self.states.lock().await;
        let state = states
            .get_mut(model_id)
            .ok_or_else(|| anyhow!("model download state disappeared: {model_id}"))?;
        // Completion can win a race with the user's cancel click. Keep the terminal state.
        if !matches!(
            state.status.as_str(),
            "queued" | "downloading" | "cancelling"
        ) {
            return Ok(state.clone());
        }
        let cancellations = self.cancellations.lock().await;
        let cancellation = cancellations
            .get(model_id)
            .ok_or_else(|| anyhow!("model download is not active: {model_id}"))?;
        cancellation.store(true, Ordering::SeqCst);
        state.status = "cancelling".into();
        state.error = None;
        Ok(state.clone())
    }

    pub async fn uninstall(&self, model_id: &str) -> Result<()> {
        if self.cancellations.lock().await.contains_key(model_id) {
            bail!("cancel the active download before uninstalling {model_id}");
        }
        let model = model_catalog()
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| anyhow!("unknown model: {model_id}"))?;
        if !model.downloadable {
            bail!("该模型由在线环境管理");
        }
        let output_path = self.models_directory.join(&model.file_name);
        let staging_path = self.partial_path(model);
        for path in [&output_path, &staging_path] {
            if path.is_dir() && model.bundle {
                fs::remove_dir_all(path).await?;
            } else if path.is_file() {
                fs::remove_file(path).await?;
            }
        }
        if model.selectable {
            self.settings
                .clear_downloaded_model(&model.kind, &output_path)
                .await?;
        }
        self.states.lock().await.remove(model_id);
        Ok(())
    }

    pub async fn select(&self, model_id: &str) -> Result<ModelDownloadState> {
        let model = model_catalog()
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| anyhow!("unknown model: {model_id}"))?;
        if !model.selectable {
            bail!("请在在线控制面板选择识别模型；离线文件转录尚未接入此模型");
        }
        let output_path = self.models_directory.join(&model.file_name);
        self.verify_package(model, &output_path).await?;
        self.settings
            .set_downloaded_model(&model.kind, &output_path)
            .await?;
        self.states()
            .await?
            .into_iter()
            .find(|state| state.model_id == model_id)
            .ok_or_else(|| anyhow!("selected model state disappeared: {model_id}"))
    }

    async fn verify_package(&self, model: &ModelCatalogItem, root: &Path) -> Result<()> {
        for file in &model.files {
            let path = model.file_path(root, file);
            let actual = sha256_file(&path).await?;
            if actual != file.sha256 {
                bail!("{} SHA-256 校验失败", file.name);
            }
        }
        Ok(())
    }

    async fn complete(&self, model: &ModelCatalogItem, output_path: &Path) -> Result<()> {
        self.ensure_not_cancelled(&model.id).await?;
        if model.selectable {
            self.settings
                .set_downloaded_model(&model.kind, output_path)
                .await?;
        }
        let bytes = if model.bundle {
            model.total_bytes()
        } else {
            fs::metadata(output_path).await?.len()
        };
        self.update_state(&model.id, |state| {
            state.downloaded_bytes = bytes;
            if state.source.is_none() {
                state.source = Some("本地已校验文件".into());
            }
            state.status = "done".into();
            state.path = Some(output_path.display().to_string());
            state.active = model.selectable;
            state.error = None;
        })
        .await;
        Ok(())
    }

    async fn download(&self, model: ModelCatalogItem) -> Result<()> {
        let network = self.settings.download_network_settings().await?;
        let client = build_download_client(&network.client)?;
        self.download_package(model, &client, |file| {
            download_sources(&file.download_path, network.model_mirror_url.as_deref())
        })
        .await
    }

    async fn download_package(
        &self,
        model: ModelCatalogItem,
        client: &reqwest::Client,
        sources: impl Fn(&ModelFile) -> Vec<DownloadSource>,
    ) -> Result<()> {
        let output_path = self.models_directory.join(&model.file_name);
        if model.bundle && output_path.exists() {
            // Imported packages remain in place. Never replace or merge an unverified directory.
            self.verify_package(&model, &output_path)
                .await
                .context("现有模型包未通过校验，请先检查或卸载该模型包后重试")?;
            return self.complete(&model, &output_path).await;
        }
        let root = if model.bundle {
            self.partial_path(&model)
        } else {
            output_path.clone()
        };
        if model.bundle {
            fs::create_dir_all(&root).await?;
        } else if let Some(parent) = root.parent() {
            fs::create_dir_all(parent).await?;
        }
        self.update_state(&model.id, |state| {
            state.status = "downloading".into();
        })
        .await;
        let mut completed_bytes = 0;
        for file in &model.files {
            self.ensure_not_cancelled(&model.id).await?;
            let path = model.file_path(&root, file);
            if path.is_file() && sha256_file(&path).await? == file.sha256 {
                completed_bytes += fs::metadata(&path).await?.len();
                self.update_state(&model.id, |state| state.downloaded_bytes = completed_bytes)
                    .await;
                continue;
            }
            let mut failures = Vec::new();
            let mut installed = false;
            for source in sources(file) {
                self.ensure_not_cancelled(&model.id).await?;
                match self
                    .download_from_source(client, &model, file, &source, &path, completed_bytes)
                    .await
                {
                    Ok(()) => {
                        installed = true;
                        break;
                    }
                    Err(error) => failures.push(format!("{}：{error:#}", source.label)),
                }
            }
            if !installed {
                bail!("{} 所有下载来源均失败：{}", file.name, failures.join("；"));
            }
            completed_bytes += fs::metadata(&path).await?.len();
        }
        self.ensure_not_cancelled(&model.id).await?;
        if model.bundle {
            fs::rename(&root, &output_path)
                .await
                .with_context(|| format!("failed to install {}", output_path.display()))?;
        }
        self.complete(&model, &output_path).await
    }

    async fn download_from_source(
        &self,
        client: &reqwest::Client,
        model: &ModelCatalogItem,
        file: &ModelFile,
        source: &DownloadSource,
        output_path: &Path,
        completed_bytes: u64,
    ) -> Result<()> {
        let part_path = output_path.with_file_name(format!(
            "{}.part",
            output_path.file_name().unwrap().to_string_lossy()
        ));
        let existing_bytes = fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0);
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
            if sha256_file(&part_path).await? == file.sha256 {
                self.ensure_not_cancelled(&model.id).await?;
                fs::rename(&part_path, output_path).await?;
                return Ok(());
            }
            fs::remove_file(&part_path).await?;
            bail!("服务器拒绝续传，且临时文件未通过完整校验；下次将重新下载");
        }
        if !status.is_success() {
            bail!("服务器返回 {status}");
        }
        let resumed = existing_bytes > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if resumed {
            validate_content_range(response.headers(), existing_bytes)?;
        }
        let file_total = if resumed {
            content_range_total(response.headers())
                .or_else(|| response.content_length().map(|n| n + existing_bytes))
        } else {
            response.content_length()
        };
        let resolved_host = response.url().host_str().unwrap_or("未知主机").to_string();
        self.update_state(&model.id, |state| {
            state.source = Some(format!(
                "{} · {resolved_host} · {}",
                source.label, file.name
            ));
            state.total_bytes = if model.bundle {
                Some(model.total_bytes())
            } else {
                file_total
            };
        })
        .await;
        let mut options = fs::OpenOptions::new();
        options.create(true).write(true);
        if resumed {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut output = options.open(&part_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = if resumed { existing_bytes } else { 0 };
        self.update_state(&model.id, |state| {
            state.downloaded_bytes = completed_bytes + downloaded_bytes
        })
        .await;
        let mut hasher = Sha256::new();
        if resumed {
            update_sha256_from_file(&part_path, &mut hasher).await?;
        }
        while let Some(chunk) = stream.next().await {
            self.ensure_not_cancelled(&model.id).await?;
            let chunk = chunk.context("model download was interrupted")?;
            output.write_all(&chunk).await?;
            hasher.update(&chunk);
            downloaded_bytes += chunk.len() as u64;
            self.update_state(&model.id, |state| {
                state.downloaded_bytes = completed_bytes + downloaded_bytes
            })
            .await;
        }
        output.flush().await?;
        drop(output);
        let actual = format!("{:x}", hasher.finalize());
        if downloaded_bytes == 0
            || actual != file.sha256
            || (file.bytes > 0 && downloaded_bytes != file.bytes)
        {
            fs::remove_file(&part_path).await?;
            bail!(
                "{} 大小或 SHA-256 校验失败，期望 {}，实际 {actual}",
                file.name,
                file.sha256
            );
        }
        self.ensure_not_cancelled(&model.id).await?;
        fs::rename(&part_path, output_path).await?;
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
            .is_some_and(|c| c.load(Ordering::SeqCst))
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
    for source in download_sources(&model.files[0].download_path, mirror.as_deref()) {
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

fn download_sources(download_path: &str, model_mirror_url: Option<&str>) -> Vec<DownloadSource> {
    let official_url = format!("https://huggingface.co/{}", download_path);
    let mut sources = Vec::new();
    if let Some(mirror) = model_mirror_url {
        let mirror_url = format!("{}/{}", mirror.trim_end_matches('/'), download_path);
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

pub fn model_catalog() -> &'static [ModelCatalogItem] {
    static CATALOG: OnceLock<Vec<ModelCatalogItem>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("model-catalog.json"))
            .expect("bundled model catalog must be valid")
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
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
        let mut ids = catalog
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), catalog.len());
        assert!(
            catalog
                .iter()
                .flat_map(|model| &model.files)
                .all(|file| !file.download_path.starts_with('/') && file.sha256.len() == 64)
        );
        assert!(
            catalog
                .iter()
                .flat_map(|model| &model.files)
                .all(|file| file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()))
        );
        assert!(catalog.iter().any(|model| model.kind == "vad"));
    }

    #[test]
    fn curated_whisper_catalog_has_the_supported_quality_tiers() {
        let whisper_ids = model_catalog()
            .iter()
            .filter(|model| model.kind == "whisper")
            .map(|model| model.id.as_str())
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
        assert!(
            models[0].files[0]
                .download_path
                .contains("a0c709d9fac510f2c807aa3af52872340dc37a4a")
        );
        assert_eq!(
            models[0].files[0].sha256,
            "dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699"
        );
        assert_eq!(models[1].id, "hy-mt2-7b-q4-k-m");
        assert!(
            models[1].files[0]
                .download_path
                .contains("ab8472660ac61fac25f1af43fac2599d52a8a775")
        );
        assert_eq!(
            models[1].files[0].sha256,
            "9f96256500f3fc1ab4d64336b58f52a949a95ad7516b0c229476eef782f9f77b"
        );
    }

    #[test]
    fn custom_mirror_precedes_the_canonical_hugging_face_source() {
        let model = &model_catalog()[0];
        let sources = download_sources(
            &model.files[0].download_path,
            Some("https://mirror.example/hf"),
        );
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].label, "自定义镜像");
        assert!(sources[0].url.starts_with("https://mirror.example/hf/"));
        assert_eq!(sources[1].label, "Hugging Face 官方源");
        assert!(sources[1].url.starts_with("https://huggingface.co/"));
    }

    #[test]
    fn content_range_must_continue_at_the_local_file_boundary() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_RANGE,
            HeaderValue::from_static("bytes 1024-2047/4096"),
        );
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

        let installed = service
            .states()
            .await
            .unwrap()
            .into_iter()
            .find(|state| state.model_id == "hy-mt2-1.8b-q4-k-m")
            .unwrap();
        assert!(installed.active);
        service.uninstall("hy-mt2-1.8b-q4-k-m").await.unwrap();

        assert!(!model_path.exists());
        assert!(
            database
                .get_setting("translation.hy_mt2_model_path")
                .await
                .unwrap()
                .is_none()
        );
        assert!(service.states().await.unwrap().is_empty());
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn catalog_packages_have_safe_paths_and_pinned_qwen_variants() {
        use std::path::Component;
        let mut paths = std::collections::HashSet::new();
        for item in model_catalog() {
            assert!(paths.insert(&item.file_name));
            assert!(
                Path::new(&item.file_name)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
            );
            assert!(!item.files.is_empty());
            let mut names = std::collections::HashSet::new();
            for file in &item.files {
                assert!(names.insert(&file.name));
                assert!(
                    Path::new(&file.name)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
                );
                if item.downloadable {
                    assert!(file.download_path.contains("/resolve/"));
                }
                if item.bundle && item.downloadable {
                    assert!(file.bytes > 0);
                    let revision = file
                        .download_path
                        .split("/resolve/")
                        .nth(1)
                        .unwrap()
                        .split('/')
                        .next()
                        .unwrap();
                    assert_eq!(revision.len(), 40);
                }
            }
        }
        for id in ["qwen-0.6b", "qwen-0.6b-4bit", "qwen-1.7b-8bit", "qwen-1.7b"] {
            let item = model_catalog().iter().find(|m| m.id == id).unwrap();
            assert_eq!(item.category, "transcription");
            assert!(item.downloadable && !item.selectable && item.bundle);
        }
        assert!(
            model_catalog()
                .iter()
                .find(|m| m.id == "hy-mt2-7b-q4-k-m")
                .unwrap()
                .retired
        );
    }

    #[tokio::test]
    async fn package_download_rejects_corruption_resumes_and_publishes_only_complete_packages() {
        use super::{DownloadSource, ModelFile};
        use axum::{
            Router,
            body::Body,
            extract::{Path as UrlPath, State},
            http::{HeaderMap, Response, StatusCode},
            routing::get,
        };
        use sha2::{Digest, Sha256};
        use std::sync::{
            Mutex as StdMutex,
            atomic::{AtomicBool, Ordering},
        };
        #[derive(Clone)]
        struct Server {
            bad: Arc<AtomicBool>,
            ranges: Arc<StdMutex<Vec<String>>>,
        }
        async fn serve(
            State(server): State<Server>,
            UrlPath(name): UrlPath<String>,
            headers: HeaderMap,
        ) -> Response<Body> {
            let range = headers
                .get("range")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            server
                .ranges
                .lock()
                .unwrap()
                .push(format!("{name}:{range}"));
            let bytes: &[u8] = if server.bad.load(Ordering::SeqCst) {
                b"bad-weights"
            } else {
                b"goodweights"
            };
            let start = range
                .strip_prefix("bytes=")
                .and_then(|n| n.strip_suffix('-'))
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
            let mut response = Response::builder().status(if start > 0 {
                StatusCode::PARTIAL_CONTENT
            } else {
                StatusCode::OK
            });
            if start > 0 {
                response = response.header(
                    "content-range",
                    format!("bytes {start}-{}/{}", bytes.len() - 1, bytes.len()),
                );
            }
            response.body(Body::from(bytes[start..].to_vec())).unwrap()
        }
        let nonce = uuid::Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("atogaki-package-test-{nonce}"));
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();
        let database = LocalDatabase::open(root.join("db.sqlite")).await.unwrap();
        let settings = DesktopSettingsService::new(
            database.clone(),
            MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider)),
            models.clone(),
            None,
            None,
            None,
            None,
        );
        let whisper_path = models.join("my-whisper.bin");
        fs::write(&whisper_path, b"existing selection").unwrap();
        settings
            .set_downloaded_model("whisper", &whisper_path)
            .await
            .unwrap();
        let service = ModelDownloadService::new(models.clone(), settings).unwrap();
        let mut model = model_catalog()
            .iter()
            .find(|m| m.id == "qwen-0.6b-4bit")
            .unwrap()
            .clone();
        model.files = [
            ("config.json", b"config".as_slice()),
            ("model.safetensors", b"goodweights".as_slice()),
        ]
        .into_iter()
        .map(|(name, bytes)| ModelFile {
            name: name.into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            download_path: name.into(),
        })
        .collect();
        service.states.lock().await.insert(
            model.id.clone(),
            super::ModelDownloadState {
                model_id: model.id.clone(),
                status: "queued".into(),
                downloaded_bytes: 0,
                total_bytes: Some(model.total_bytes()),
                path: None,
                error: None,
                source: None,
                active: false,
            },
        );
        let stage = service.partial_path(&model);
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join("config.json"), b"config").unwrap();
        let output = models.join(&model.file_name);
        let server = Server {
            bad: Arc::new(AtomicBool::new(true)),
            ranges: Arc::default(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/{name}", get(serve))
            .with_state(server.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let sources = |file: &ModelFile| {
            vec![DownloadSource {
                label: "fixture",
                url: format!("http://{address}/{}", file.name),
            }]
        };
        let error = service
            .download_package(model.clone(), &client, sources)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("SHA-256"));
        assert!(!output.exists() && !model.installed(&models));
        assert!(stage.join("config.json").is_file());
        assert!(!stage.join("model.safetensors.part").exists());
        server.bad.store(false, Ordering::SeqCst);
        fs::write(stage.join("model.safetensors.part"), b"goo").unwrap();
        service
            .download_package(model.clone(), &client, sources)
            .await
            .unwrap();
        assert!(output.is_dir() && !stage.exists() && model.installed(&models));
        let done = service.states.lock().await.get(&model.id).unwrap().clone();
        assert_eq!(done.status, "done");
        assert!(!done.active);
        assert_eq!(done.downloaded_bytes, model.total_bytes());
        let finishing = Arc::new(AtomicBool::new(false));
        service
            .cancellations
            .lock()
            .await
            .insert(model.id.clone(), finishing.clone());
        assert_eq!(service.cancel(&model.id).await.unwrap().status, "done");
        assert!(!finishing.load(Ordering::SeqCst));
        service.cancellations.lock().await.remove(&model.id);

        assert_eq!(
            server.ranges.lock().unwrap().as_slice(),
            ["model.safetensors:", "model.safetensors:bytes=3-"]
        );
        assert_eq!(
            service
                .settings
                .downloaded_model_path("whisper")
                .await
                .unwrap()
                .as_deref(),
            Some(whisper_path.as_path())
        );
        // Re-opening model management uses required files, never mere directory presence.
        fs::remove_file(output.join("config.json")).unwrap();
        assert!(!model.installed(&models));
        fs::write(output.join("keep-user-data"), b"keep").unwrap();
        assert!(
            service
                .download_package(model.clone(), &client, sources)
                .await
                .is_err()
        );
        assert_eq!(fs::read(output.join("keep-user-data")).unwrap(), b"keep");
        // Cancellation leaves a resumable staging directory and cannot publish it.
        fs::remove_dir_all(&output).unwrap();
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join("config.json"), b"config").unwrap();
        service
            .cancellations
            .lock()
            .await
            .insert(model.id.clone(), Arc::new(AtomicBool::new(true)));
        assert!(
            service
                .download_package(model.clone(), &client, sources)
                .await
                .is_err()
        );
        assert!(!output.exists() && stage.join("config.json").exists());
        service.cancellations.lock().await.remove(&model.id);
        assert!(service.select(&model.id).await.is_err());
        service.uninstall(&model.id).await.unwrap();
        assert!(!stage.exists() && whisper_path.exists());
        task.abort();
        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
