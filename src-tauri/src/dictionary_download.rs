use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use tokio::{
    fs,
    io::AsyncWriteExt,
    sync::Mutex,
};

use crate::desktop_settings::DesktopSettingsService;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryCatalogItem {
    pub id: &'static str,
    pub name: &'static str,
    pub language_pair: &'static str,
    pub version_label: &'static str,
    pub size_label: &'static str,
    pub description: &'static str,
    pub license: &'static str,
    pub attribution: &'static str,
    pub source_url: &'static str,
    #[serde(skip)]
    file_name: &'static str,
    #[serde(skip)]
    source: DictionarySource,
}

#[derive(Debug, Clone, Copy)]
enum DictionarySource {
    Fixed {
        url: &'static str,
        checksum: &'static str,
        algorithm: ChecksumAlgorithm,
    },
    GithubLatest {
        repository: &'static str,
        asset_prefix: &'static str,
        asset_suffix: &'static str,
    },
}

#[derive(Debug, Clone, Copy)]
enum ChecksumAlgorithm {
    Sha256,
    Sha512,
}

#[derive(Debug, Clone)]
struct ResolvedDownload {
    url: String,
    checksum: String,
    algorithm: ChecksumAlgorithm,
    version: String,
    total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryDownloadState {
    pub dictionary_id: String,
    pub status: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DictionaryDownloadService {
    directory: PathBuf,
    settings: DesktopSettingsService,
    states: Arc<Mutex<HashMap<String, DictionaryDownloadState>>>,
}

impl DictionaryDownloadService {
    pub fn new(directory: PathBuf, settings: DesktopSettingsService) -> Result<Self> {
        std::fs::create_dir_all(&directory).with_context(|| {
            format!("failed to create dictionary directory {}", directory.display())
        })?;
        cleanup_partial_downloads(&directory)?;
        let mut states = HashMap::new();
        for item in dictionary_catalog() {
            let path = directory.join(item.file_name);
            if path.is_file() {
                let version = std::fs::read_to_string(version_path(&path))
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                states.insert(
                    item.id.to_string(),
                    DictionaryDownloadState {
                        dictionary_id: item.id.to_string(),
                        status: "done".to_string(),
                        downloaded_bytes: path.metadata().map(|value| value.len()).unwrap_or(0),
                        total_bytes: path.metadata().map(|value| value.len()).ok(),
                        path: Some(path.display().to_string()),
                        version,
                        error: None,
                        source: Some("本地已安装".to_string()),
                    },
                );
            }
        }
        Ok(Self {
            directory,
            settings,
            states: Arc::new(Mutex::new(states)),
        })
    }

    pub fn catalog(&self) -> Vec<DictionaryCatalogItem> {
        dictionary_catalog().to_vec()
    }

    pub fn directory(&self) -> String {
        self.directory.display().to_string()
    }

    pub async fn states(&self) -> Vec<DictionaryDownloadState> {
        let mut states: Vec<_> = self.states.lock().await.values().cloned().collect();
        states.sort_by(|left, right| left.dictionary_id.cmp(&right.dictionary_id));
        states
    }

    pub async fn start(&self, dictionary_id: &str) -> Result<DictionaryDownloadState> {
        let item = dictionary_catalog()
            .iter()
            .find(|item| item.id == dictionary_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown dictionary: {dictionary_id}"))?;
        let mut states = self.states.lock().await;
        if states
            .values()
            .any(|state| matches!(state.status.as_str(), "queued" | "resolving" | "downloading"))
        {
            bail!("another dictionary download is already running");
        }
        let state = DictionaryDownloadState {
            dictionary_id: item.id.to_string(),
            status: "queued".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            path: None,
            version: None,
            error: None,
            source: None,
        };
        states.insert(item.id.to_string(), state.clone());
        drop(states);

        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service.download(item.clone()).await {
                let _ = fs::remove_file(service.partial_path(&item)).await;
                service
                    .update_state(item.id, |state| {
                        state.status = "failed".to_string();
                        state.error = Some(format!("{error:#}"));
                    })
                    .await;
            }
        });
        Ok(state)
    }

    async fn download(&self, item: DictionaryCatalogItem) -> Result<()> {
        self.update_state(item.id, |state| state.status = "resolving".to_string())
            .await;
        let network = self.settings.download_network_settings().await?;
        let client = network
            .client
            .apply(
                reqwest::Client::builder()
                    .user_agent("Atogaki/0.1 dictionary downloader")
                    .connect_timeout(Duration::from_secs(20))
                    .read_timeout(Duration::from_secs(90)),
            )?
            .build()
            .context("failed to build dictionary download client")?;
        let resolved = resolve_download(&client, &item).await?;
        self.update_state(item.id, |state| {
            state.status = "downloading".to_string();
            state.total_bytes = resolved.total_bytes;
            state.version = Some(resolved.version.clone());
            state.source = Some(source_host(&resolved.url));
        })
        .await;

        let response = client
            .get(&resolved.url)
            .send()
            .await
            .context("无法连接词典包来源")?;
        if !response.status().is_success() {
            bail!("词典包来源返回 {}", response.status());
        }
        let total_bytes = response.content_length().or(resolved.total_bytes);
        self.update_state(item.id, |state| state.total_bytes = total_bytes)
            .await;
        let part_path = self.partial_path(&item);
        let _ = fs::remove_file(&part_path).await;
        let mut output = fs::File::create(&part_path)
            .await
            .with_context(|| format!("failed to create {}", part_path.display()))?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0_u64;
        let mut digest = StreamingDigest::new(resolved.algorithm);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("dictionary download was interrupted")?;
            output.write_all(&chunk).await.context("failed to write dictionary package")?;
            digest.update(&chunk);
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            self.update_state(item.id, |state| state.downloaded_bytes = downloaded)
                .await;
        }
        output.flush().await.context("failed to flush dictionary package")?;
        drop(output);
        if downloaded == 0 {
            bail!("下载来源返回了空文件");
        }
        let actual = digest.finalize();
        if !actual.eq_ignore_ascii_case(&resolved.checksum) {
            bail!("词典包校验失败，期望 {}，实际 {actual}", resolved.checksum);
        }
        let output_path = self.directory.join(item.file_name);
        install_package(&part_path, &output_path).await?;
        fs::write(version_path(&output_path), resolved.version.as_bytes())
            .await
            .context("failed to save dictionary package version")?;
        self.update_state(item.id, |state| {
            state.status = "done".to_string();
            state.path = Some(output_path.display().to_string());
            state.downloaded_bytes = downloaded;
        })
        .await;
        Ok(())
    }

    async fn update_state(
        &self,
        dictionary_id: &str,
        update: impl FnOnce(&mut DictionaryDownloadState),
    ) {
        if let Some(state) = self.states.lock().await.get_mut(dictionary_id) {
            update(state);
        }
    }

    fn partial_path(&self, item: &DictionaryCatalogItem) -> PathBuf {
        self.directory.join(format!("{}.part", item.file_name))
    }
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

async fn resolve_download(
    client: &reqwest::Client,
    item: &DictionaryCatalogItem,
) -> Result<ResolvedDownload> {
    match item.source {
        DictionarySource::Fixed {
            url,
            checksum,
            algorithm,
        } => Ok(ResolvedDownload {
            url: url.to_string(),
            checksum: checksum.to_string(),
            algorithm,
            version: item.version_label.to_string(),
            total_bytes: None,
        }),
        DictionarySource::GithubLatest {
            repository,
            asset_prefix,
            asset_suffix,
        } => {
            let metadata_url = format!("https://api.github.com/repos/{repository}/releases/latest");
            let release = client
                .get(&metadata_url)
                .header(reqwest::header::ACCEPT, "application/vnd.github+json")
                .send()
                .await
                .context("无法读取词典发布元数据")?
                .error_for_status()
                .context("词典发布元数据返回错误")?
                .json::<GithubRelease>()
                .await
                .context("无法解析词典发布元数据")?;
            let asset = release
                .assets
                .into_iter()
                .find(|asset| {
                    asset.name.starts_with(asset_prefix) && asset.name.ends_with(asset_suffix)
                })
                .ok_or_else(|| anyhow!("最新发布中没有找到预期词典包"))?;
            let digest = asset
                .digest
                .as_deref()
                .and_then(|digest| digest.strip_prefix("sha256:"))
                .filter(|digest| digest.len() == 64)
                .ok_or_else(|| anyhow!("发布方没有提供可用的 SHA-256，拒绝安装"))?;
            Ok(ResolvedDownload {
                url: asset.browser_download_url,
                checksum: digest.to_string(),
                algorithm: ChecksumAlgorithm::Sha256,
                version: release.tag_name,
                total_bytes: Some(asset.size),
            })
        }
    }
}

enum StreamingDigest {
    Sha256(Sha256),
    Sha512(Sha512),
}

impl StreamingDigest {
    fn new(algorithm: ChecksumAlgorithm) -> Self {
        match algorithm {
            ChecksumAlgorithm::Sha256 => Self::Sha256(Sha256::new()),
            ChecksumAlgorithm::Sha512 => Self::Sha512(Sha512::new()),
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha256(value) => value.update(bytes),
            Self::Sha512(value) => value.update(bytes),
        }
    }

    fn finalize(self) -> String {
        match self {
            Self::Sha256(value) => format!("{:x}", value.finalize()),
            Self::Sha512(value) => format!("{:x}", value.finalize()),
        }
    }
}

fn source_host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "词典发布源".to_string())
}

fn version_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.version",
        path.extension().and_then(|value| value.to_str()).unwrap_or("package")
    ))
}

fn cleanup_partial_downloads(directory: &Path) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("part") {
            std::fs::remove_file(&path).with_context(|| {
                format!("failed to clean interrupted dictionary download {}", path.display())
            })?;
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with(".backup"))
        {
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("backup filename was checked")
                .trim_end_matches(".backup");
            let output_path = path.with_file_name(file_name);
            if output_path.is_file() {
                std::fs::remove_file(&path)?;
            } else {
                std::fs::rename(&path, &output_path).with_context(|| {
                    format!("failed to restore dictionary backup {}", path.display())
                })?;
            }
        }
    }
    Ok(())
}

async fn install_package(part_path: &Path, output_path: &Path) -> Result<()> {
    if !output_path.is_file() {
        return fs::rename(part_path, output_path)
            .await
            .with_context(|| format!("failed to install {}", output_path.display()));
    }
    let backup_path = output_path.with_extension(format!(
        "{}.backup",
        output_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("package")
    ));
    let _ = fs::remove_file(&backup_path).await;
    fs::rename(output_path, &backup_path)
        .await
        .with_context(|| format!("failed to prepare update for {}", output_path.display()))?;
    if let Err(error) = fs::rename(part_path, output_path).await {
        let _ = fs::rename(&backup_path, output_path).await;
        return Err(error)
            .with_context(|| format!("failed to update {}; previous package restored", output_path.display()));
    }
    fs::remove_file(&backup_path)
        .await
        .with_context(|| format!("failed to remove update backup {}", backup_path.display()))?;
    Ok(())
}

fn dictionary_catalog() -> &'static [DictionaryCatalogItem] {
    const CATALOG: &[DictionaryCatalogItem] = &[
        DictionaryCatalogItem {
            id: "jmdict-en",
            name: "JMdict（日英）",
            language_pair: "日语 → 英语",
            version_label: "下载时获取最新版",
            size_label: "约 11 MiB",
            description: "EDRDG JMdict 的结构化 JSON 包；用于词形、读音、词性与英文义项。",
            license: "CC BY-SA 4.0",
            attribution: "EDRDG；JSON packaging by scriptin/jmdict-simplified",
            source_url: "https://github.com/scriptin/jmdict-simplified/releases",
            file_name: "jmdict-eng.json.tgz",
            source: DictionarySource::GithubLatest {
                repository: "scriptin/jmdict-simplified",
                asset_prefix: "jmdict-eng-",
                asset_suffix: ".json.tgz",
            },
        },
        DictionaryCatalogItem {
            id: "tomoshi-open",
            name: "Tomoshi 开放数据层（日中）",
            language_pair: "日语 → 简体中文",
            version_label: "下载时获取最新版",
            size_label: "以发布页为准",
            description: "JMdict 衍生 SQLite 与中文释义；含 LLM 辅助生成并复核的衍生内容。",
            license: "按表：CC BY-SA 4.0 / 3.0",
            attribution: "Tomoshi (Y1Z) 与各上游数据项目",
            source_url: "https://github.com/tomoshi-app/tomoshi-dict-data/releases",
            file_name: "tomoshi-dict-open.db.zst",
            source: DictionarySource::GithubLatest {
                repository: "tomoshi-app/tomoshi-dict-data",
                asset_prefix: "tomoshi-dict-open.db",
                asset_suffix: ".zst",
            },
        },
        DictionaryCatalogItem {
            id: "freedict-eng-zho",
            name: "FreeDict 英中",
            language_pair: "英语 → 中文",
            version_label: "2025.11.23",
            size_label: "1.6 MiB · 26,660 词头",
            description: "来自 WikDict/Wiktionary 的免费离线补充；不等同于商业学习词典。",
            license: "FreeDict/WikDict 数据许可（包内声明）",
            attribution: "FreeDict Project 与 WikDict/Wiktionary contributors",
            source_url: "https://freedict.org/downloads/",
            file_name: "freedict-eng-zho.stardict.tar.xz",
            source: DictionarySource::Fixed {
                url: "https://download.freedict.org/dictionaries/eng-zho/2025.11.23/freedict-eng-zho-2025.11.23.stardict.tar.xz",
                checksum: "059f9aca26fdc3a5a2c0c0e8fc92e111a34bf8fd438f70d267cccf35f5e47a2d45c46650999a1b3a48c3bffc3e16e0db897232128fe822d1bc59cf34f40b395c",
                algorithm: ChecksumAlgorithm::Sha512,
            },
        },
    ];
    CATALOG
}

#[cfg(test)]
mod tests {
    use super::{ChecksumAlgorithm, StreamingDigest, cleanup_partial_downloads, dictionary_catalog};
    use sha2::{Digest, Sha256, Sha512};

    #[test]
    fn catalog_has_unique_ids_and_https_sources() {
        let catalog = dictionary_catalog();
        let mut ids: Vec<_> = catalog.iter().map(|item| item.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), catalog.len());
        assert!(catalog.iter().all(|item| item.source_url.starts_with("https://")));
    }

    #[test]
    fn streaming_digest_supports_both_manifest_algorithms() {
        let mut sha256 = StreamingDigest::new(ChecksumAlgorithm::Sha256);
        sha256.update(b"atogaki");
        assert_eq!(sha256.finalize(), format!("{:x}", Sha256::digest(b"atogaki")));
        let mut sha512 = StreamingDigest::new(ChecksumAlgorithm::Sha512);
        sha512.update(b"atogaki");
        assert_eq!(sha512.finalize(), format!("{:x}", Sha512::digest(b"atogaki")));
    }

    #[test]
    fn interrupted_packages_are_cleaned_without_touching_installed_files() {
        let root = std::env::temp_dir().join(format!("atogaki-dict-cleanup-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let installed = root.join("dictionary.db.zst");
        let partial = root.join("dictionary.db.zst.part");
        std::fs::write(&installed, b"installed").unwrap();
        std::fs::write(&partial, b"partial").unwrap();
        cleanup_partial_downloads(&root).unwrap();
        assert!(installed.is_file());
        assert!(!partial.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_update_restores_the_previous_package() {
        let root =
            std::env::temp_dir().join(format!("atogaki-dict-backup-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let installed = root.join("dictionary.db.zst");
        let backup = root.join("dictionary.db.zst.backup");
        std::fs::write(&backup, b"previous package").unwrap();
        cleanup_partial_downloads(&root).unwrap();
        assert_eq!(std::fs::read(&installed).unwrap(), b"previous package");
        assert!(!backup.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
