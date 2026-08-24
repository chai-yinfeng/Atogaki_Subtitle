use std::{
    collections::HashMap,
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow, bail};
use atogaki_subtitle::{
    application::{
        MutableTranslationProvider, TranslationFuture, TranslationProvider,
        TranslationProviderStatus, TranslationRequest, UnconfiguredTranslationProvider,
    },
    infrastructure::{
        deepl::DeepLTranslationProvider,
        local_db::LocalDatabase,
        network::{NetworkClientConfig, normalize_https_endpoint},
        openai_compatible::{
            OpenAiCompatibleConfig, OpenAiCompatibleTranslationProvider,
        },
    },
};
use serde::{Deserialize, Serialize};

use crate::credential_store::{CredentialStore, SystemCredentialStore};

const ONBOARDING_COMPLETED: &str = "desktop.onboarding_completed";
const WHISPER_MODEL_PATH: &str = "recognition.whisper_model_path";
const VAD_MODEL_PATH: &str = "recognition.vad_model_path";
const TRANSLATION_PROVIDER: &str = "translation.provider";
const DEEPL_KEY_SAVED: &str = "translation.deepl_key_saved";
const DEEPSEEK_KEY_SAVED: &str = "translation.deepseek_key_saved";
const OPENAI_COMPATIBLE_KEY_SAVED: &str = "translation.openai_compatible_key_saved";
const CAMBRIDGE_DICTIONARY_KEY_SAVED: &str = "dictionary.cambridge_key_saved";
const COLLINS_DICTIONARY_KEY_SAVED: &str = "dictionary.collins_key_saved";
const MERRIAM_WEBSTER_DICTIONARY_KEY_SAVED: &str = "dictionary.merriam_webster_key_saved";
const DEEPSEEK_MODEL: &str = "translation.deepseek_model";
const OPENAI_BASE_URL: &str = "translation.openai_base_url";
const OPENAI_MODEL: &str = "translation.openai_model";
const LLM_STYLE_INSTRUCTION: &str = "translation.llm_style_instruction";
const NETWORK_PROXY_MODE: &str = "network.proxy_mode";
const NETWORK_PROXY_URL: &str = "network.proxy_url";
const MODEL_MIRROR_URL: &str = "network.model_mirror_url";
const DEEPL_PROVIDER_ID: &str = "deepl";
const DEEPSEEK_PROVIDER_ID: &str = "deepseek";
const OPENAI_COMPATIBLE_PROVIDER_ID: &str = "openai-compatible";
const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-v4-flash";
const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_LLM_STYLE: &str = "准确、自然的简体中文口语字幕；保留说话语气，不补充原文没有的信息。";

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
    pub translation_model: Option<String>,
    pub translation_base_url: Option<String>,
    pub translation_style_instruction: String,
    pub translation_api_key_configured: bool,
    pub translation_api_key_source: Option<String>,
    pub credential_store: String,
    pub credential_error: Option<String>,
    pub models_directory: String,
    pub network_proxy_mode: String,
    pub network_proxy_url: Option<String>,
    pub model_mirror_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCredentialCheck {
    pub provider_id: String,
    pub provider_name: String,
    pub stored_in_system: bool,
    pub available_from_environment: bool,
    pub credential_store: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryCredentialStatus {
    pub provider_id: String,
    pub provider_name: String,
    pub configured: bool,
    pub credential_store: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDictionaryCredentialRequest {
    pub provider_id: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDesktopSettingsRequest {
    pub whisper_model_path: Option<String>,
    pub vad_model_path: Option<String>,
    pub translation_provider_id: String,
    pub translation_model: Option<String>,
    pub translation_base_url: Option<String>,
    pub translation_style_instruction: Option<String>,
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
    environment_deepseek_key: Option<String>,
    environment_whisper_model: Option<PathBuf>,
    environment_vad_model: Option<PathBuf>,
    credential_cache: Arc<Mutex<HashMap<String, CredentialCache>>>,
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
    credential_cache: Arc<Mutex<HashMap<String, CredentialCache>>>,
    environment_key: Option<String>,
    network: NetworkClientConfig,
}

impl DeferredDeepLTranslationProvider {
    fn new(
        credentials: Arc<dyn CredentialStore>,
        credential_cache: Arc<Mutex<HashMap<String, CredentialCache>>>,
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
            cached_provider_key(
                DEEPL_PROVIDER_ID,
                self.credentials.as_ref(),
                self.credential_cache.as_ref(),
            );
        let key = if let Some(key) = stored_key.or_else(|| self.environment_key.clone()) {
            key
        } else if let Some(error) = credential_error {
            return Err(anyhow!("无法读取 DeepL Key：{error}"));
        } else {
            return Err(anyhow!("请先在设置中配置 DeepL API Key。"));
        };
        DeepLTranslationProvider::with_network_config(Some(key), &self.network)
    }
}

impl fmt::Debug for DeferredDeepLTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeferredDeepLTranslationProvider")
            .field(
                "credential_loaded",
                &credential_cache_loaded(
                    DEEPL_PROVIDER_ID,
                    self.credential_cache.as_ref(),
                ),
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
            endpoint_kind: "deepl-v2".to_string(),
            configuration_hint: Some("将在首次翻译时从系统凭据库读取 DeepL Key。".to_string()),
        }
    }

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async move { self.resolve()?.translate(request).await })
    }
}

#[derive(Clone)]
struct DeferredOpenAiCompatibleTranslationProvider {
    provider_id: String,
    provider_name: String,
    credentials: Arc<dyn CredentialStore>,
    credential_cache: Arc<Mutex<HashMap<String, CredentialCache>>>,
    environment_key: Option<String>,
    network: NetworkClientConfig,
    base_url: String,
    model: String,
    style_instruction: String,
    disable_deepseek_thinking: bool,
}

impl DeferredOpenAiCompatibleTranslationProvider {
    fn resolve(&self) -> Result<OpenAiCompatibleTranslationProvider> {
        let (stored_key, credential_error) = cached_provider_key(
            &self.provider_id,
            self.credentials.as_ref(),
            self.credential_cache.as_ref(),
        );
        let key = if let Some(key) = stored_key.or_else(|| self.environment_key.clone()) {
            key
        } else if let Some(error) = credential_error {
            return Err(anyhow!("无法读取 {} Key：{error}", self.provider_name));
        } else {
            return Err(anyhow!("请先在设置中配置 {} API Key。", self.provider_name));
        };
        OpenAiCompatibleTranslationProvider::with_network_config(
            OpenAiCompatibleConfig {
                provider_id: self.provider_id.clone(),
                provider_name: self.provider_name.clone(),
                api_key: Some(key),
                base_url: self.base_url.clone(),
                model: self.model.clone(),
                style_instruction: self.style_instruction.clone(),
                disable_deepseek_thinking: self.disable_deepseek_thinking,
            },
            &self.network,
        )
    }
}

impl fmt::Debug for DeferredOpenAiCompatibleTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeferredOpenAiCompatibleTranslationProvider")
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .field(
                "credential_loaded",
                &credential_cache_loaded(&self.provider_id, self.credential_cache.as_ref()),
            )
            .finish()
    }
}

impl TranslationProvider for DeferredOpenAiCompatibleTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        TranslationProviderStatus {
            id: self.provider_id.clone(),
            name: self.provider_name.clone(),
            configured: true,
            model: Some(self.model.clone()),
            endpoint_kind: "openai-chat-completions".to_string(),
            configuration_hint: Some(format!(
                "将在首次翻译时从系统凭据库读取 {} Key。",
                self.provider_name
            )),
        }
    }

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async move { self.resolve()?.translate(request).await })
    }
}

impl DesktopSettingsService {
    pub fn new(
        database: LocalDatabase,
        provider: MutableTranslationProvider,
        models_directory: PathBuf,
        environment_deepl_key: Option<String>,
        environment_deepseek_key: Option<String>,
        environment_whisper_model: Option<PathBuf>,
        environment_vad_model: Option<PathBuf>,
    ) -> Self {
        Self {
            database,
            credentials: Arc::new(SystemCredentialStore),
            provider,
            models_directory,
            environment_deepl_key: normalized_secret(environment_deepl_key),
            environment_deepseek_key: normalized_secret(environment_deepseek_key),
            environment_whisper_model: existing_file(environment_whisper_model),
            environment_vad_model: existing_file(environment_vad_model),
            credential_cache: Arc::new(Mutex::new(HashMap::new())),
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
            environment_deepseek_key: None,
            environment_whisper_model: None,
            environment_vad_model: None,
            credential_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn initialize(&self) -> Result<DesktopSettings> {
        std::fs::create_dir_all(&self.models_directory)?;
        let settings = self.load().await?;
        let network = NetworkClientConfig::new(
            &settings.network_proxy_mode,
            settings.network_proxy_url.clone(),
        )?;
        self.replace_provider(
            &settings.translation_provider_id,
            &network,
            settings.translation_model.as_deref(),
            settings.translation_base_url.as_deref(),
            &settings.translation_style_instruction,
        )?;
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
            .filter(|provider_id| {
                matches!(
                    provider_id.as_str(),
                    "none"
                        | DEEPL_PROVIDER_ID
                        | DEEPSEEK_PROVIDER_ID
                        | OPENAI_COMPATIBLE_PROVIDER_ID
                )
            })
            .unwrap_or_else(|| "none".to_string());
        let translation_model = match translation_provider_id.as_str() {
            DEEPSEEK_PROVIDER_ID => Some(
                self.database
                    .get_setting(DEEPSEEK_MODEL)
                    .await?
                    .unwrap_or_else(|| DEFAULT_DEEPSEEK_MODEL.to_string()),
            ),
            OPENAI_COMPATIBLE_PROVIDER_ID => self.database.get_setting(OPENAI_MODEL).await?,
            _ => None,
        };
        let translation_base_url = match translation_provider_id.as_str() {
            DEEPSEEK_PROVIDER_ID => Some(DEFAULT_DEEPSEEK_BASE_URL.to_string()),
            OPENAI_COMPATIBLE_PROVIDER_ID => Some(
                self.database
                    .get_setting(OPENAI_BASE_URL)
                    .await?
                    .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string()),
            ),
            _ => None,
        };
        let translation_style_instruction = self
            .database
            .get_setting(LLM_STYLE_INSTRUCTION)
            .await?
            .unwrap_or_else(|| DEFAULT_LLM_STYLE.to_string());
        let key_saved = if let Some(key) = key_saved_setting(&translation_provider_id) {
            self.database.get_setting(key).await?.as_deref() == Some("true")
        } else {
            false
        };
        let (stored_key, credential_error, credential_loaded) =
            self.cached_provider_key_snapshot(&translation_provider_id);
        let environment_key = match translation_provider_id.as_str() {
            DEEPL_PROVIDER_ID => self.environment_deepl_key.as_ref(),
            DEEPSEEK_PROVIDER_ID => self.environment_deepseek_key.as_ref(),
            _ => None,
        };
        let (translation_api_key_configured, translation_api_key_source) = if stored_key.is_some() {
            (true, Some("system".to_string()))
        } else if environment_key.is_some() {
            (true, Some("environment".to_string()))
        } else if key_saved {
            (true, Some("saved".to_string()))
        } else if translation_provider_id != "none" && !credential_loaded {
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
            translation_model,
            translation_base_url,
            translation_style_instruction,
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
        let translation_model = request
            .translation_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let translation_base_url = request
            .translation_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let translation_style_instruction = request
            .translation_style_instruction
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_LLM_STYLE)
            .to_string();
        validate_llm_settings(
            &request.translation_provider_id,
            translation_model.as_deref(),
            translation_base_url.as_deref(),
            &translation_style_instruction,
            &network,
        )?;

        save_optional_path(&self.database, WHISPER_MODEL_PATH, whisper_model.as_ref()).await?;
        save_optional_path(&self.database, VAD_MODEL_PATH, vad_model.as_ref()).await?;
        self.database
            .set_setting(TRANSLATION_PROVIDER, &request.translation_provider_id)
            .await?;
        if request.translation_provider_id == DEEPSEEK_PROVIDER_ID {
            self.database
                .set_setting(
                    DEEPSEEK_MODEL,
                    translation_model
                        .as_deref()
                        .unwrap_or(DEFAULT_DEEPSEEK_MODEL),
                )
                .await?;
        }
        if request.translation_provider_id == OPENAI_COMPATIBLE_PROVIDER_ID {
            self.database
                .set_setting(
                    OPENAI_BASE_URL,
                    translation_base_url
                        .as_deref()
                        .unwrap_or(DEFAULT_OPENAI_BASE_URL),
                )
                .await?;
            self.database
                .set_setting(
                    OPENAI_MODEL,
                    translation_model
                        .as_deref()
                        .ok_or_else(|| anyhow!("OpenAI-compatible model cannot be empty"))?,
                )
                .await?;
        }
        self.database
            .set_setting(LLM_STYLE_INSTRUCTION, &translation_style_instruction)
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

        if request.clear_api_key && request.translation_provider_id != "none" {
            self.credentials.delete(&request.translation_provider_id)?;
            self.replace_cached_provider_key(&request.translation_provider_id, None, None);
            if let Some(key) = key_saved_setting(&request.translation_provider_id) {
                self.database.delete_setting(key).await?;
            }
        }
        if let Some(secret) = normalized_secret(request.api_key) {
            if request.translation_provider_id == "none" {
                bail!("cannot save an API key while translation is disabled");
            }
            self.credentials
                .set(&request.translation_provider_id, &secret)?;
            self.replace_cached_provider_key(
                &request.translation_provider_id,
                Some(secret),
                None,
            );
            if let Some(key) = key_saved_setting(&request.translation_provider_id) {
                self.database.set_setting(key, "true").await?;
            }
        }

        self.replace_provider(
            &request.translation_provider_id,
            &network,
            translation_model.as_deref(),
            translation_base_url.as_deref(),
            &translation_style_instruction,
        )?;
        self.load().await
    }

    /// Explicitly checks one provider credential after a user action. Normal startup and
    /// settings loading remain credential-free so macOS does not show Keychain prompts merely
    /// because the application was opened.
    pub async fn check_translation_api_key(
        &self,
        provider_id: &str,
    ) -> Result<TranslationCredentialCheck> {
        validate_provider_id(provider_id)?;
        if provider_id == "none" {
            bail!("select a translation provider before checking its API key");
        }
        let secret = match self.credentials.get(provider_id) {
            Ok(secret) => normalized_secret(secret),
            Err(error) => {
                self.replace_cached_provider_key(provider_id, None, Some(format!("{error:#}")));
                return Err(error);
            }
        };
        self.replace_cached_provider_key(provider_id, secret.clone(), None);
        if let Some(key) = key_saved_setting(provider_id) {
            if secret.is_some() {
                self.database.set_setting(key, "true").await?;
            } else {
                self.database.delete_setting(key).await?;
            }
        }
        let available_from_environment = match provider_id {
            DEEPL_PROVIDER_ID => self.environment_deepl_key.is_some(),
            DEEPSEEK_PROVIDER_ID => self.environment_deepseek_key.is_some(),
            _ => false,
        };
        Ok(TranslationCredentialCheck {
            provider_id: provider_id.to_string(),
            provider_name: provider_display_name(provider_id).to_string(),
            stored_in_system: secret.is_some(),
            available_from_environment,
            credential_store: self.credentials.backend_name().to_string(),
        })
    }

    /// Lists only non-secret saved markers. Opening settings must not trigger a Keychain prompt.
    pub async fn dictionary_credential_statuses(&self) -> Result<Vec<DictionaryCredentialStatus>> {
        let mut statuses = Vec::new();
        for provider_id in ["cambridge", "collins", "merriam-webster"] {
            let configured = self
                .database
                .get_setting(dictionary_key_saved_setting(provider_id)?)
                .await?
                .as_deref()
                == Some("true");
            statuses.push(DictionaryCredentialStatus {
                provider_id: provider_id.to_string(),
                provider_name: dictionary_provider_display_name(provider_id)?.to_string(),
                configured,
                credential_store: self.credentials.backend_name().to_string(),
            });
        }
        Ok(statuses)
    }

    pub async fn save_dictionary_credential(
        &self,
        request: SaveDictionaryCredentialRequest,
    ) -> Result<DictionaryCredentialStatus> {
        let marker = dictionary_key_saved_setting(&request.provider_id)?;
        let credential_id = dictionary_credential_id(&request.provider_id)?;
        let secret = normalized_secret(request.api_key);
        if request.clear && secret.is_some() {
            bail!("cannot save and clear the same dictionary credential");
        }
        if request.clear {
            self.credentials.delete(&credential_id)?;
            self.database.delete_setting(marker).await?;
        } else if let Some(secret) = secret {
            self.credentials.set(&credential_id, &secret)?;
            self.database.set_setting(marker, "true").await?;
        } else {
            bail!("dictionary API key cannot be empty");
        }
        Ok(DictionaryCredentialStatus {
            provider_id: request.provider_id.clone(),
            provider_name: dictionary_provider_display_name(&request.provider_id)?.to_string(),
            configured: !request.clear,
            credential_store: self.credentials.backend_name().to_string(),
        })
    }

    /// Explicit user action: reads one provider entry and repairs its non-secret marker.
    pub async fn check_dictionary_credential(
        &self,
        provider_id: &str,
    ) -> Result<DictionaryCredentialStatus> {
        let marker = dictionary_key_saved_setting(provider_id)?;
        let credential_id = dictionary_credential_id(provider_id)?;
        let configured = normalized_secret(self.credentials.get(&credential_id)?).is_some();
        if configured {
            self.database.set_setting(marker, "true").await?;
        } else {
            self.database.delete_setting(marker).await?;
        }
        Ok(DictionaryCredentialStatus {
            provider_id: provider_id.to_string(),
            provider_name: dictionary_provider_display_name(provider_id)?.to_string(),
            configured,
            credential_store: self.credentials.backend_name().to_string(),
        })
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

    fn replace_provider(
        &self,
        provider_id: &str,
        network: &NetworkClientConfig,
        model: Option<&str>,
        base_url: Option<&str>,
        style_instruction: &str,
    ) -> Result<()> {
        let provider: Arc<dyn TranslationProvider> = match provider_id {
            "none" => Arc::new(UnconfiguredTranslationProvider),
            DEEPL_PROVIDER_ID => Arc::new(DeferredDeepLTranslationProvider::new(
                Arc::clone(&self.credentials),
                Arc::clone(&self.credential_cache),
                self.environment_deepl_key.clone(),
                network.clone(),
            )),
            DEEPSEEK_PROVIDER_ID => Arc::new(DeferredOpenAiCompatibleTranslationProvider {
                provider_id: DEEPSEEK_PROVIDER_ID.to_string(),
                provider_name: "DeepSeek".to_string(),
                credentials: Arc::clone(&self.credentials),
                credential_cache: Arc::clone(&self.credential_cache),
                environment_key: self.environment_deepseek_key.clone(),
                network: network.clone(),
                base_url: DEFAULT_DEEPSEEK_BASE_URL.to_string(),
                model: model.unwrap_or(DEFAULT_DEEPSEEK_MODEL).to_string(),
                style_instruction: style_instruction.to_string(),
                disable_deepseek_thinking: true,
            }),
            OPENAI_COMPATIBLE_PROVIDER_ID => {
                Arc::new(DeferredOpenAiCompatibleTranslationProvider {
                    provider_id: OPENAI_COMPATIBLE_PROVIDER_ID.to_string(),
                    provider_name: "OpenAI-compatible".to_string(),
                    credentials: Arc::clone(&self.credentials),
                    credential_cache: Arc::clone(&self.credential_cache),
                    environment_key: None,
                    network: network.clone(),
                    base_url: base_url.unwrap_or(DEFAULT_OPENAI_BASE_URL).to_string(),
                    model: model
                        .ok_or_else(|| anyhow!("OpenAI-compatible model cannot be empty"))?
                        .to_string(),
                    style_instruction: style_instruction.to_string(),
                    disable_deepseek_thinking: false,
                })
            }
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

    fn cached_provider_key_snapshot(
        &self,
        provider_id: &str,
    ) -> (Option<String>, Option<String>, bool) {
        let cache = self
            .credential_cache
            .lock()
            .expect("credential cache lock poisoned");
        cache
            .get(provider_id)
            .map(|entry| (entry.secret.clone(), entry.error.clone(), entry.loaded))
            .unwrap_or((None, None, false))
    }

    fn replace_cached_provider_key(
        &self,
        provider_id: &str,
        secret: Option<String>,
        error: Option<String>,
    ) {
        let mut caches = self
            .credential_cache
            .lock()
            .expect("credential cache lock poisoned");
        caches.insert(
            provider_id.to_string(),
            CredentialCache {
                loaded: true,
                secret,
                error,
            },
        );
    }
}

fn cached_provider_key(
    provider_id: &str,
    credentials: &dyn CredentialStore,
    credential_cache: &Mutex<HashMap<String, CredentialCache>>,
) -> (Option<String>, Option<String>) {
    let mut caches = credential_cache
        .lock()
        .expect("credential cache lock poisoned");
    let cache = caches.entry(provider_id.to_string()).or_default();
    if !cache.loaded {
        cache.loaded = true;
        match credentials.get(provider_id) {
            Ok(secret) => cache.secret = normalized_secret(secret),
            Err(error) => cache.error = Some(format!("{error:#}")),
        }
    }
    (cache.secret.clone(), cache.error.clone())
}

fn credential_cache_loaded(
    provider_id: &str,
    credential_cache: &Mutex<HashMap<String, CredentialCache>>,
) -> bool {
    credential_cache
        .lock()
        .expect("credential cache lock poisoned")
        .get(provider_id)
        .is_some_and(|cache| cache.loaded)
}

fn validate_provider_id(provider_id: &str) -> Result<()> {
    if matches!(
        provider_id,
        "none" | DEEPL_PROVIDER_ID | DEEPSEEK_PROVIDER_ID | OPENAI_COMPATIBLE_PROVIDER_ID
    ) {
        Ok(())
    } else {
        bail!("unsupported translation provider: {provider_id}")
    }
}

fn key_saved_setting(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        DEEPL_PROVIDER_ID => Some(DEEPL_KEY_SAVED),
        DEEPSEEK_PROVIDER_ID => Some(DEEPSEEK_KEY_SAVED),
        OPENAI_COMPATIBLE_PROVIDER_ID => Some(OPENAI_COMPATIBLE_KEY_SAVED),
        _ => None,
    }
}

fn provider_display_name(provider_id: &str) -> &'static str {
    match provider_id {
        DEEPL_PROVIDER_ID => "DeepL",
        DEEPSEEK_PROVIDER_ID => "DeepSeek",
        OPENAI_COMPATIBLE_PROVIDER_ID => "OpenAI-compatible",
        _ => "翻译服务",
    }
}

fn dictionary_key_saved_setting(provider_id: &str) -> Result<&'static str> {
    match provider_id {
        "cambridge" => Ok(CAMBRIDGE_DICTIONARY_KEY_SAVED),
        "collins" => Ok(COLLINS_DICTIONARY_KEY_SAVED),
        "merriam-webster" => Ok(MERRIAM_WEBSTER_DICTIONARY_KEY_SAVED),
        _ => bail!("unsupported dictionary provider: {provider_id}"),
    }
}

fn dictionary_credential_id(provider_id: &str) -> Result<String> {
    dictionary_provider_display_name(provider_id)?;
    Ok(format!("dictionary:{provider_id}"))
}

fn dictionary_provider_display_name(provider_id: &str) -> Result<&'static str> {
    match provider_id {
        "cambridge" => Ok("Cambridge Dictionary"),
        "collins" => Ok("Collins Dictionary"),
        "merriam-webster" => Ok("Merriam-Webster"),
        _ => bail!("unsupported dictionary provider: {provider_id}"),
    }
}

fn validate_llm_settings(
    provider_id: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    style_instruction: &str,
    network: &NetworkClientConfig,
) -> Result<()> {
    let config = match provider_id {
        DEEPSEEK_PROVIDER_ID => Some(OpenAiCompatibleConfig {
            provider_id: provider_id.to_string(),
            provider_name: "DeepSeek".to_string(),
            api_key: Some("validation-only".to_string()),
            base_url: DEFAULT_DEEPSEEK_BASE_URL.to_string(),
            model: model.unwrap_or(DEFAULT_DEEPSEEK_MODEL).to_string(),
            style_instruction: style_instruction.to_string(),
            disable_deepseek_thinking: true,
        }),
        OPENAI_COMPATIBLE_PROVIDER_ID => Some(OpenAiCompatibleConfig {
            provider_id: provider_id.to_string(),
            provider_name: "OpenAI-compatible".to_string(),
            api_key: Some("validation-only".to_string()),
            base_url: base_url.unwrap_or(DEFAULT_OPENAI_BASE_URL).to_string(),
            model: model.unwrap_or_default().to_string(),
            style_instruction: style_instruction.to_string(),
            disable_deepseek_thinking: false,
        }),
        _ => None,
    };
    if let Some(config) = config {
        OpenAiCompatibleTranslationProvider::with_network_config(config, network)?;
    }
    Ok(())
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
        MutableTranslationProvider, TranslationOptions, TranslationProvider, TranslationRequest,
        TranslationTargetSegment,
        UnconfiguredTranslationProvider,
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

    #[derive(Debug)]
    struct FailingCredentialStore;

    impl CredentialStore for FailingCredentialStore {
        fn backend_name(&self) -> &'static str {
            "failing credential store"
        }

        fn get(&self, _provider_id: &str) -> Result<Option<String>> {
            anyhow::bail!("user interaction was denied")
        }

        fn set(&self, _provider_id: &str, _secret: &str) -> Result<()> {
            unreachable!()
        }

        fn delete(&self, _provider_id: &str) -> Result<()> {
            unreachable!()
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
                translation_model: None,
                translation_base_url: None,
                translation_style_instruction: None,
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
        assert_eq!(
            database
                .get_setting("translation.deepl_key_saved")
                .await
                .unwrap()
                .as_deref(),
            Some("true")
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
    async fn saves_deepseek_configuration_without_putting_the_secret_in_sqlite() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-deepseek-settings-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
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
                whisper_model_path: None,
                vad_model_path: None,
                translation_provider_id: "deepseek".to_string(),
                translation_model: Some("deepseek-v4-flash".to_string()),
                translation_base_url: Some("https://ignored.example/v1".to_string()),
                translation_style_instruction: Some("自然、简洁的电台口语字幕。".to_string()),
                api_key: Some("deepseek-test-secret".to_string()),
                network_proxy_mode: "direct".to_string(),
                network_proxy_url: None,
                model_mirror_url: None,
                clear_api_key: false,
                onboarding_completed: true,
            })
            .await
            .unwrap();

        assert_eq!(saved.translation_provider_id, "deepseek");
        assert_eq!(
            saved.translation_model.as_deref(),
            Some("deepseek-v4-flash")
        );
        assert_eq!(
            saved.translation_base_url.as_deref(),
            Some("https://api.deepseek.com")
        );
        assert_eq!(provider.status().id, "deepseek");
        assert_eq!(provider.status().model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(
            credentials.secret.lock().unwrap().as_deref(),
            Some("deepseek-test-secret")
        );
        assert_eq!(
            database
                .get_setting("translation.deepseek_key_saved")
                .await
                .unwrap()
                .as_deref(),
            Some("true")
        );
        assert_eq!(
            database
                .get_setting("translation.deepseek_model")
                .await
                .unwrap()
                .as_deref(),
            Some("deepseek-v4-flash")
        );
        assert!(
            database
                .get_setting("deepseek-test-secret")
                .await
                .unwrap()
                .is_none()
        );

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn explicit_credential_check_reads_once_and_repairs_the_saved_marker() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-key-check-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let credentials = Arc::new(MemoryCredentialStore::default());
        *credentials.secret.lock().unwrap() = Some("existing-deepseek-key".to_string());
        let service = DesktopSettingsService::with_credentials(
            database.clone(),
            MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider)),
            root.join("models"),
            credentials.clone(),
        );

        let checked = service
            .check_translation_api_key("deepseek")
            .await
            .unwrap();

        assert!(checked.stored_in_system);
        assert!(!checked.available_from_environment);
        assert_eq!(checked.provider_name, "DeepSeek");
        assert_eq!(*credentials.reads.lock().unwrap(), 1);
        assert_eq!(
            database
                .get_setting("translation.deepseek_key_saved")
                .await
                .unwrap()
                .as_deref(),
            Some("true")
        );

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn dictionary_credentials_are_provider_scoped_and_startup_uses_only_markers() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-dictionary-key-test-{nonce}"));
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

        let saved = service
            .save_dictionary_credential(super::SaveDictionaryCredentialRequest {
                provider_id: "merriam-webster".to_string(),
                api_key: Some("dictionary-secret".to_string()),
                clear: false,
            })
            .await
            .unwrap();
        assert!(saved.configured);
        assert_eq!(
            database
                .get_setting("dictionary.merriam_webster_key_saved")
                .await
                .unwrap()
                .as_deref(),
            Some("true")
        );
        assert!(database.get_setting("dictionary-secret").await.unwrap().is_none());
        assert_eq!(*credentials.reads.lock().unwrap(), 0);

        let statuses = service.dictionary_credential_statuses().await.unwrap();
        assert_eq!(*credentials.reads.lock().unwrap(), 0);
        assert!(
            statuses
                .iter()
                .find(|status| status.provider_id == "merriam-webster")
                .unwrap()
                .configured
        );
        assert!(
            !statuses
                .iter()
                .find(|status| status.provider_id == "cambridge")
                .unwrap()
                .configured
        );

        service
            .save_dictionary_credential(super::SaveDictionaryCredentialRequest {
                provider_id: "merriam-webster".to_string(),
                api_key: None,
                clear: true,
            })
            .await
            .unwrap();
        assert!(
            database
                .get_setting("dictionary.merriam_webster_key_saved")
                .await
                .unwrap()
                .is_none()
        );

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

    #[tokio::test]
    async fn denied_keychain_access_keeps_the_full_translation_error_chain() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-denied-key-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .set_setting("translation.provider", "deepl")
            .await
            .unwrap();
        let provider = MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider));
        let service = DesktopSettingsService::with_credentials(
            database.clone(),
            provider.clone(),
            root.join("models"),
            Arc::new(FailingCredentialStore),
        );
        service.initialize().await.unwrap();

        let error = provider
            .translate(TranslationRequest {
                options: TranslationOptions::default(),
                before_context: Vec::new(),
                targets: vec![TranslationTargetSegment {
                    segment_id: "segment-1".to_string(),
                    source_text: "字幕".to_string(),
                }],
                after_context: Vec::new(),
                style_instruction: None,
            })
            .await
            .unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            "无法读取 DeepL Key：user interaction was denied"
        );
        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
