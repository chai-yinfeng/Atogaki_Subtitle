use std::{
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow, bail};
use atogaki_subtitle::{
    application::{
        MutableTranslationProvider, TranslationFuture, TranslationOptions, TranslationProvider,
        TranslationProviderStatus, UnconfiguredTranslationProvider,
    },
    infrastructure::{
        deepl::DeepLTranslationProvider,
        local_db::LocalDatabase,
        network::{NetworkClientConfig, normalize_https_endpoint},
    },
};
use serde::{Deserialize, Serialize};

use crate::credential_store::{CredentialStore, SystemCredentialStore};

const ONBOARDING_COMPLETED: &str = "desktop.onboarding_completed";
const WHISPER_MODEL_PATH: &str = "recognition.whisper_model_path";
const VAD_MODEL_PATH: &str = "recognition.vad_model_path";
const TRANSLATION_PROVIDER: &str = "translation.provider";
const NETWORK_PROXY_MODE: &str = "network.proxy_mode";
const NETWORK_PROXY_URL: &str = "network.proxy_url";
const MODEL_MIRROR_URL: &str = "network.model_mirror_url";
const DEEPL_PROVIDER_ID: &str = "deepl";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub onboarding_completed: bool,
    pub needs_onboarding: bool,
    pub whisper_model_path: Option<String>,
    pub whisper_model_ready: bool,
    pub vad_model_path: Option<String>,
    pub vad_model_ready: bool,
    pub translation_provider_id: String,
    pub translation_api_key_configured: bool,
    pub translation_api_key_source: Option<String>,
    pub credential_store: String,
    pub credential_error: Option<String>,
    pub models_directory: String,
    pub network_proxy_mode: String,
    pub network_proxy_url: Option<String>,
    pub model_mirror_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDesktopSettingsRequest {
    pub whisper_model_path: Option<String>,
    pub vad_model_path: Option<String>,
    pub translation_provider_id: String,
    pub api_key: Option<String>,
    pub network_proxy_mode: String,
    pub network_proxy_url: Option<String>,
    pub model_mirror_url: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub onboarding_completed: bool,
}

#[derive(Debug, Clone)]
pub struct DownloadNetworkSettings {
    pub client: NetworkClientConfig,
    pub model_mirror_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DesktopSettingsService {
    database: LocalDatabase,
    credentials: Arc<dyn CredentialStore>,
    provider: MutableTranslationProvider,
    models_directory: PathBuf,
    environment_deepl_key: Option<String>,
    environment_whisper_model: Option<PathBuf>,
    environment_vad_model: Option<PathBuf>,
    credential_cache: Arc<Mutex<CredentialCache>>,
}

#[derive(Debug, Default)]
struct CredentialCache {
    loaded: bool,
    secret: Option<String>,
    error: Option<String>,
}

/// Avoids asking macOS for a Keychain unlock while the user is only opening the
/// local application or configuring model downloads. The secret is resolved on
/// the first actual translation request and is cached for the rest of the run.
#[derive(Clone)]
struct DeferredDeepLTranslationProvider {
    credentials: Arc<dyn CredentialStore>,
    credential_cache: Arc<Mutex<CredentialCache>>,
    environment_key: Option<String>,
    network: NetworkClientConfig,
}

impl DeferredDeepLTranslationProvider {
    fn new(
        credentials: Arc<dyn CredentialStore>,
        credential_cache: Arc<Mutex<CredentialCache>>,
        environment_key: Option<String>,
        network: NetworkClientConfig,
    ) -> Self {
        Self {
            credentials,
            credential_cache,
            environment_key,
            network,
        }
    }

    fn resolve(&self) -> Result<DeepLTranslationProvider> {
        let (stored_key, credential_error) =
            cached_deepl_key(self.credentials.as_ref(), self.credential_cache.as_ref());
        let key = stored_key
            .or_else(|| self.environment_key.clone())
            .ok_or_else(|| {
                credential_error
                    .map(|error| anyhow!("无法读取 DeepL Key：{error}"))
                    .unwrap_or_else(|| anyhow!("请先在设置中配置 DeepL API Key。"))
            })?;
        DeepLTranslationProvider::with_network_config(Some(key), &self.network)
    }
}

impl fmt::Debug for DeferredDeepLTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeferredDeepLTranslationProvider")
            .field(
                "credential_loaded",
                &credential_cache_loaded(self.credential_cache.as_ref()),
            )
            .finish()
    }
}

impl TranslationProvider for DeferredDeepLTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        TranslationProviderStatus {
            id: DEEPL_PROVIDER_ID.to_string(),
            name: "DeepL".to_string(),
            // DeepL is deliberately selectable before reading Keychain. A missing or denied
            // secret is reported on the first translation rather than during local startup.
            configured: true,
            model: None,
            configuration_hint: Some("将在首次翻译时从系统凭据库读取 DeepL Key。".to_string()),
        }
    }

    fn translate<'a>(
        &'a self,
        options: &'a TranslationOptions,
        texts: &'a [String],
        context: Option<&'a str>,
    ) -> TranslationFuture<'a> {
        let options = options.clone();
        let texts = texts.to_vec();
        let context = context.map(str::to_string);
        Box::pin(async move {
            self.resolve()?
                .translate(&options, &texts, context.as_deref())
                .await
        })
    }
}

impl DesktopSettingsService {
    pub fn new(
        database: LocalDatabase,
        provider: MutableTranslationProvider,
        models_directory: PathBuf,
        environment_deepl_key: Option<String>,
        environment_whisper_model: Option<PathBuf>,
        environment_vad_model: Option<PathBuf>,
    ) -> Self {
        Self {
            database,
            credentials: Arc::new(SystemCredentialStore),
            provider,
            models_directory,
            environment_deepl_key: normalized_secret(environment_deepl_key),
            environment_whisper_model: existing_file(environment_whisper_model),
            environment_vad_model: existing_file(environment_vad_model),
            credential_cache: Arc::new(Mutex::new(CredentialCache::default())),
        }
    }

    #[cfg(test)]
    pub fn with_credentials(
        database: LocalDatabase,
        provider: MutableTranslationProvider,
        models_directory: PathBuf,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            database,
            credentials,
            provider,
            models_directory,
            environment_deepl_key: None,
            environment_whisper_model: None,
            environment_vad_model: None,
            credential_cache: Arc::new(Mutex::new(CredentialCache::default())),
        }
    }

    pub async fn initialize(&self) -> Result<DesktopSettings> {
        std::fs::create_dir_all(&self.models_directory)?;
        let settings = self.load().await?;
        let network = NetworkClientConfig::new(
            &settings.network_proxy_mode,
            settings.network_proxy_url.clone(),
        )?;
        self.replace_provider(&settings.translation_provider_id, &network)?;
        Ok(settings)
    }

    pub async fn load(&self) -> Result<DesktopSettings> {
        let onboarding_completed = self
            .database
            .get_setting(ONBOARDING_COMPLETED)
            .await?
            .as_deref()
            == Some("true");
        let whisper_model = self
            .database
            .get_setting(WHISPER_MODEL_PATH)
            .await?
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| self.environment_whisper_model.clone());
        let vad_model = self
            .database
            .get_setting(VAD_MODEL_PATH)
            .await?
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| self.environment_vad_model.clone());
        let translation_provider_id = self
            .database
            .get_setting(TRANSLATION_PROVIDER)
            .await?
            .filter(|provider_id| matches!(provider_id.as_str(), "none" | DEEPL_PROVIDER_ID))
            .unwrap_or_else(|| "none".to_string());
        let (stored_key, credential_error, credential_loaded) = self.cached_deepl_key_snapshot();
        let (translation_api_key_configured, translation_api_key_source) = if stored_key.is_some() {
            (true, Some("system".to_string()))
        } else if self.environment_deepl_key.is_some() {
            (true, Some("environment".to_string()))
        } else if translation_provider_id == DEEPL_PROVIDER_ID && !credential_loaded {
            (false, Some("deferred".to_string()))
        } else {
            (false, None)
        };
        let whisper_model_ready = whisper_model.as_ref().is_some_and(|path| path.is_file());
        let vad_model_ready = vad_model.as_ref().is_some_and(|path| path.is_file());
        let download_network = self.download_network_settings().await?;

        Ok(DesktopSettings {
            onboarding_completed,
            needs_onboarding: !onboarding_completed || !whisper_model_ready,
            whisper_model_path: display_path(whisper_model.as_ref()),
            whisper_model_ready,
            vad_model_path: display_path(vad_model.as_ref()),
            vad_model_ready,
            translation_provider_id,
            translation_api_key_configured,
            translation_api_key_source,
            credential_store: self.credentials.backend_name().to_string(),
            credential_error,
            models_directory: self.models_directory.display().to_string(),
            network_proxy_mode: download_network.client.proxy_mode().as_str().to_string(),
            network_proxy_url: download_network
                .client
                .custom_proxy_url()
                .map(str::to_string),
            model_mirror_url: download_network.model_mirror_url,
        })
    }

    pub async fn save(&self, request: SaveDesktopSettingsRequest) -> Result<DesktopSettings> {
        validate_provider_id(&request.translation_provider_id)?;
        let whisper_model = normalized_optional_path(request.whisper_model_path);
        let vad_model = normalized_optional_path(request.vad_model_path);
        validate_optional_model(&whisper_model, "Whisper")?;
        validate_optional_model(&vad_model, "VAD")?;
        let network = NetworkClientConfig::new(
            &request.network_proxy_mode,
            request.network_proxy_url.clone(),
        )?;
        let model_mirror_url = normalize_https_endpoint(request.model_mirror_url.clone())?;

        save_optional_path(&self.database, WHISPER_MODEL_PATH, whisper_model.as_ref()).await?;
        save_optional_path(&self.database, VAD_MODEL_PATH, vad_model.as_ref()).await?;
        self.database
            .set_setting(TRANSLATION_PROVIDER, &request.translation_provider_id)
            .await?;
        self.database
            .set_setting(
                ONBOARDING_COMPLETED,
                if request.onboarding_completed {
                    "true"
                } else {
                    "false"
                },
            )
            .await?;
        self.persist_download_network_settings(&network, model_mirror_url.as_deref())
            .await?;

        if request.clear_api_key {
            self.credentials.delete(DEEPL_PROVIDER_ID)?;
            self.replace_cached_deepl_key(None, None);
        }
        if let Some(secret) = normalized_secret(request.api_key) {
            self.credentials.set(DEEPL_PROVIDER_ID, &secret)?;
            self.replace_cached_deepl_key(Some(secret), None);
        }

        self.replace_provider(&request.translation_provider_id, &network)?;
        self.load().await
    }

    /// Persists only the non-secret network draft used by the model downloader.
    /// This deliberately bypasses provider construction and credential-store access so a
    /// download can use the visible proxy settings without prompting for a DeepL key.
    pub async fn save_download_network_settings(
        &self,
        proxy_mode: &str,
        proxy_url: Option<String>,
        model_mirror_url: Option<String>,
    ) -> Result<()> {
        let network = NetworkClientConfig::new(proxy_mode, proxy_url)?;
        let model_mirror_url = normalize_https_endpoint(model_mirror_url)?;
        self.persist_download_network_settings(&network, model_mirror_url.as_deref())
            .await
    }

    pub async fn download_network_settings(&self) -> Result<DownloadNetworkSettings> {
        let proxy_mode = self
            .database
            .get_setting(NETWORK_PROXY_MODE)
            .await?
            .unwrap_or_else(|| "environment".to_string());
        let proxy_url = self.database.get_setting(NETWORK_PROXY_URL).await?;
        let model_mirror_url =
            normalize_https_endpoint(self.database.get_setting(MODEL_MIRROR_URL).await?)?;
        Ok(DownloadNetworkSettings {
            client: NetworkClientConfig::new(&proxy_mode, proxy_url)?,
            model_mirror_url,
        })
    }

    pub async fn set_downloaded_model(&self, kind: &str, path: &std::path::Path) -> Result<()> {
        let key = match kind {
            "whisper" => WHISPER_MODEL_PATH,
            "vad" => VAD_MODEL_PATH,
            _ => bail!("unsupported model kind: {kind}"),
        };
        self.database
            .set_setting(key, &path.display().to_string())
            .await
    }

    fn replace_provider(&self, provider_id: &str, network: &NetworkClientConfig) -> Result<()> {
        let provider: Arc<dyn TranslationProvider> = match provider_id {
            "none" => Arc::new(UnconfiguredTranslationProvider),
            DEEPL_PROVIDER_ID => Arc::new(DeferredDeepLTranslationProvider::new(
                Arc::clone(&self.credentials),
                Arc::clone(&self.credential_cache),
                self.environment_deepl_key.clone(),
                network.clone(),
            )),
            _ => return Err(anyhow!("unsupported translation provider: {provider_id}")),
        };
        self.provider.replace(provider);
        Ok(())
    }

    async fn persist_download_network_settings(
        &self,
        network: &NetworkClientConfig,
        model_mirror_url: Option<&str>,
    ) -> Result<()> {
        self.database
            .set_setting(NETWORK_PROXY_MODE, network.proxy_mode().as_str())
            .await?;
        save_optional_string(
            &self.database,
            NETWORK_PROXY_URL,
            network.custom_proxy_url(),
        )
        .await?;
        save_optional_string(&self.database, MODEL_MIRROR_URL, model_mirror_url).await
    }

    fn cached_deepl_key_snapshot(&self) -> (Option<String>, Option<String>, bool) {
        let cache = self
            .credential_cache
            .lock()
            .expect("credential cache lock poisoned");
        (cache.secret.clone(), cache.error.clone(), cache.loaded)
    }

    fn replace_cached_deepl_key(&self, secret: Option<String>, error: Option<String>) {
        let mut cache = self
            .credential_cache
            .lock()
            .expect("credential cache lock poisoned");
        cache.loaded = true;
        cache.secret = secret;
        cache.error = error;
    }
}

fn cached_deepl_key(
    credentials: &dyn CredentialStore,
    credential_cache: &Mutex<CredentialCache>,
) -> (Option<String>, Option<String>) {
    let mut cache = credential_cache
        .lock()
        .expect("credential cache lock poisoned");
    if !cache.loaded {
        cache.loaded = true;
        match credentials.get(DEEPL_PROVIDER_ID) {
            Ok(secret) => cache.secret = normalized_secret(secret),
            Err(error) => cache.error = Some(error.to_string()),
        }
    }
    (cache.secret.clone(), cache.error.clone())
}

fn credential_cache_loaded(credential_cache: &Mutex<CredentialCache>) -> bool {
    credential_cache
        .lock()
        .expect("credential cache lock poisoned")
        .loaded
}

fn validate_provider_id(provider_id: &str) -> Result<()> {
    if matches!(provider_id, "none" | DEEPL_PROVIDER_ID) {
        Ok(())
    } else {
        bail!("unsupported translation provider: {provider_id}")
    }
}

fn normalized_secret(secret: Option<String>) -> Option<String> {
    secret
        .map(|secret| secret.trim().to_string())
        .filter(|secret| !secret.is_empty())
}

fn normalized_optional_path(path: Option<String>) -> Option<PathBuf> {
    path.map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn validate_optional_model(path: &Option<PathBuf>, label: &str) -> Result<()> {
    if let Some(path) = path
        && !path.is_file()
    {
        bail!("{label} model does not exist: {}", path.display());
    }
    Ok(())
}

async fn save_optional_path(
    database: &LocalDatabase,
    key: &str,
    path: Option<&PathBuf>,
) -> Result<()> {
    if let Some(path) = path {
        database.set_setting(key, &path.display().to_string()).await
    } else {
        database.delete_setting(key).await
    }
}

async fn save_optional_string(
    database: &LocalDatabase,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    if let Some(value) = value {
        database.set_setting(key, value).await
    } else {
        database.delete_setting(key).await
    }
}

fn existing_file(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| path.is_file())
}

fn display_path(path: Option<&PathBuf>) -> Option<String> {
    path.map(|path| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;
    use atogaki_subtitle::application::{
        MutableTranslationProvider, TranslationProvider, UnconfiguredTranslationProvider,
    };

    use super::{DesktopSettingsService, SaveDesktopSettingsRequest};
    use crate::credential_store::CredentialStore;
    use atogaki_subtitle::infrastructure::local_db::LocalDatabase;

    #[derive(Debug, Default)]
    struct MemoryCredentialStore {
        secret: Mutex<Option<String>>,
        reads: Mutex<usize>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn backend_name(&self) -> &'static str {
            "test credential store"
        }

        fn get(&self, _provider_id: &str) -> Result<Option<String>> {
            *self.reads.lock().unwrap() += 1;
            Ok(self.secret.lock().unwrap().clone())
        }

        fn set(&self, _provider_id: &str, secret: &str) -> Result<()> {
            *self.secret.lock().unwrap() = Some(secret.to_string());
            Ok(())
        }

        fn delete(&self, _provider_id: &str) -> Result<()> {
            *self.secret.lock().unwrap() = None;
            Ok(())
        }
    }

    #[tokio::test]
    async fn saves_paths_in_sqlite_but_keeps_api_key_in_credential_store() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-desktop-settings-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let model = root.join("ggml-small.bin");
        fs::write(&model, b"model placeholder").unwrap();
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let provider = MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider));
        let credentials = Arc::new(MemoryCredentialStore::default());
        let service = DesktopSettingsService::with_credentials(
            database.clone(),
            provider.clone(),
            root.join("models"),
            credentials.clone(),
        );

        let saved = service
            .save(SaveDesktopSettingsRequest {
                whisper_model_path: Some(model.display().to_string()),
                vad_model_path: None,
                translation_provider_id: "deepl".to_string(),
                api_key: Some("test-secret:fx".to_string()),
                network_proxy_mode: "custom".to_string(),
                network_proxy_url: Some("http://127.0.0.1:7897".to_string()),
                model_mirror_url: Some("https://mirror.example/hf/".to_string()),
                clear_api_key: false,
                onboarding_completed: true,
            })
            .await
            .unwrap();

        assert!(!saved.needs_onboarding);
        assert!(saved.translation_api_key_configured);
        assert_eq!(saved.translation_api_key_source.as_deref(), Some("system"));
        assert_eq!(saved.network_proxy_mode, "custom");
        assert_eq!(
            saved.network_proxy_url.as_deref(),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            saved.model_mirror_url.as_deref(),
            Some("https://mirror.example/hf")
        );
        assert_eq!(
            credentials.secret.lock().unwrap().as_deref(),
            Some("test-secret:fx")
        );
        assert_eq!(*credentials.reads.lock().unwrap(), 0);
        assert!(provider.status().configured);
        assert_eq!(
            database
                .get_setting("recognition.whisper_model_path")
                .await
                .unwrap()
                .as_deref(),
            Some(model.display().to_string().as_str())
        );
        assert!(database.get_setting("deepl").await.unwrap().is_none());

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn saves_download_network_without_reading_the_credential_store() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-network-settings-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let credentials = Arc::new(MemoryCredentialStore::default());
        let service = DesktopSettingsService::with_credentials(
            database.clone(),
            MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider)),
            root.join("models"),
            credentials.clone(),
        );

        service
            .save_download_network_settings(
                "custom",
                Some("http://127.0.0.1:7897".to_string()),
                Some("https://hf-mirror.com/".to_string()),
            )
            .await
            .unwrap();

        let network = service.download_network_settings().await.unwrap();
        assert_eq!(network.client.proxy_mode().as_str(), "custom");
        assert_eq!(
            network.client.custom_proxy_url(),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            network.model_mirror_url.as_deref(),
            Some("https://hf-mirror.com")
        );
        assert_eq!(*credentials.reads.lock().unwrap(), 0);

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn startup_defers_keychain_access_until_a_translation_is_requested() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-deferred-key-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .set_setting("translation.provider", "deepl")
            .await
            .unwrap();
        let credentials = Arc::new(MemoryCredentialStore::default());
        *credentials.secret.lock().unwrap() = Some("existing-key:fx".to_string());
        let provider = MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider));
        let service = DesktopSettingsService::with_credentials(
            database.clone(),
            provider.clone(),
            root.join("models"),
            credentials.clone(),
        );

        let settings = service.initialize().await.unwrap();
        assert_eq!(
            settings.translation_api_key_source.as_deref(),
            Some("deferred")
        );
        assert_eq!(*credentials.reads.lock().unwrap(), 0);
        assert!(provider.status().configured);

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
