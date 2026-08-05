use std::{path::PathBuf, sync::Arc};

use anyhow::{Result, anyhow, bail};
use atogaki_subtitle::{
    application::{
        MutableTranslationProvider, TranslationProvider, UnconfiguredTranslationProvider,
    },
    infrastructure::{deepl::DeepLTranslationProvider, local_db::LocalDatabase},
};
use serde::{Deserialize, Serialize};

use crate::credential_store::{CredentialStore, SystemCredentialStore};

const ONBOARDING_COMPLETED: &str = "desktop.onboarding_completed";
const WHISPER_MODEL_PATH: &str = "recognition.whisper_model_path";
const VAD_MODEL_PATH: &str = "recognition.vad_model_path";
const TRANSLATION_PROVIDER: &str = "translation.provider";
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDesktopSettingsRequest {
    pub whisper_model_path: Option<String>,
    pub vad_model_path: Option<String>,
    pub translation_provider_id: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub onboarding_completed: bool,
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
        }
    }

    pub async fn initialize(&self) -> Result<DesktopSettings> {
        std::fs::create_dir_all(&self.models_directory)?;
        let settings = self.load().await?;
        self.replace_provider(&settings.translation_provider_id)?;
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
            .unwrap_or_else(|| DEEPL_PROVIDER_ID.to_string());
        let (stored_key, credential_error) = match self.credentials.get(DEEPL_PROVIDER_ID) {
            Ok(secret) => (normalized_secret(secret), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let (translation_api_key_configured, translation_api_key_source) = if stored_key.is_some() {
            (true, Some("system".to_string()))
        } else if self.environment_deepl_key.is_some() {
            (true, Some("environment".to_string()))
        } else {
            (false, None)
        };
        let whisper_model_ready = whisper_model.as_ref().is_some_and(|path| path.is_file());
        let vad_model_ready = vad_model.as_ref().is_some_and(|path| path.is_file());

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
        })
    }

    pub async fn save(&self, request: SaveDesktopSettingsRequest) -> Result<DesktopSettings> {
        validate_provider_id(&request.translation_provider_id)?;
        let whisper_model = normalized_optional_path(request.whisper_model_path);
        let vad_model = normalized_optional_path(request.vad_model_path);
        validate_optional_model(&whisper_model, "Whisper")?;
        validate_optional_model(&vad_model, "VAD")?;

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

        if request.clear_api_key {
            self.credentials.delete(DEEPL_PROVIDER_ID)?;
        }
        if let Some(secret) = normalized_secret(request.api_key) {
            self.credentials.set(DEEPL_PROVIDER_ID, &secret)?;
        }

        self.replace_provider(&request.translation_provider_id)?;
        self.load().await
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

    fn replace_provider(&self, provider_id: &str) -> Result<()> {
        let provider: Arc<dyn TranslationProvider> = match provider_id {
            "none" => Arc::new(UnconfiguredTranslationProvider),
            DEEPL_PROVIDER_ID => {
                let key = self
                    .credentials
                    .get(DEEPL_PROVIDER_ID)
                    .ok()
                    .flatten()
                    .or_else(|| self.environment_deepl_key.clone());
                Arc::new(DeepLTranslationProvider::new(key))
            }
            _ => return Err(anyhow!("unsupported translation provider: {provider_id}")),
        };
        self.provider.replace(provider);
        Ok(())
    }
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
    }

    impl CredentialStore for MemoryCredentialStore {
        fn backend_name(&self) -> &'static str {
            "test credential store"
        }

        fn get(&self, _provider_id: &str) -> Result<Option<String>> {
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
                clear_api_key: false,
                onboarding_completed: true,
            })
            .await
            .unwrap();

        assert!(!saved.needs_onboarding);
        assert!(saved.translation_api_key_configured);
        assert_eq!(saved.translation_api_key_source.as_deref(), Some("system"));
        assert_eq!(
            credentials.secret.lock().unwrap().as_deref(),
            Some("test-secret:fx")
        );
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
}
