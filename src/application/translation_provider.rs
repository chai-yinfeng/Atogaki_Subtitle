use std::{
    fmt::{self, Debug},
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::application::TranslationOptions;

pub type TranslationFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TranslationProviderStatus {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub model: Option<String>,
    pub configuration_hint: Option<String>,
}

pub trait TranslationProvider: Debug + Send + Sync {
    fn status(&self) -> TranslationProviderStatus;

    /// Translate each source string independently while preserving input order.
    ///
    /// Providers may use `context` to improve the batch, but must return exactly
    /// one non-empty result for each source string. Stable subtitle IDs and
    /// transactional persistence remain the responsibility of the application
    /// service rather than the provider adapter.
    fn translate<'a>(
        &'a self,
        options: &'a TranslationOptions,
        texts: &'a [String],
        context: Option<&'a str>,
    ) -> TranslationFuture<'a>;
}

#[derive(Clone)]
pub struct MutableTranslationProvider {
    provider: Arc<RwLock<Arc<dyn TranslationProvider>>>,
}

impl MutableTranslationProvider {
    pub fn new(provider: Arc<dyn TranslationProvider>) -> Self {
        Self {
            provider: Arc::new(RwLock::new(provider)),
        }
    }

    pub fn replace(&self, provider: Arc<dyn TranslationProvider>) {
        *self
            .provider
            .write()
            .expect("translation provider lock poisoned") = provider;
    }

    fn current(&self) -> Arc<dyn TranslationProvider> {
        Arc::clone(
            &self
                .provider
                .read()
                .expect("translation provider lock poisoned"),
        )
    }
}

impl Debug for MutableTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MutableTranslationProvider")
            .field("status", &self.status())
            .finish()
    }
}

impl TranslationProvider for MutableTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        self.current().status()
    }

    fn translate<'a>(
        &'a self,
        options: &'a TranslationOptions,
        texts: &'a [String],
        context: Option<&'a str>,
    ) -> TranslationFuture<'a> {
        let provider = self.current();
        let options = options.clone();
        let texts = texts.to_vec();
        let context = context.map(str::to_string);
        Box::pin(async move {
            provider
                .translate(&options, &texts, context.as_deref())
                .await
        })
    }
}

#[derive(Debug, Default)]
pub struct UnconfiguredTranslationProvider;

impl TranslationProvider for UnconfiguredTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        TranslationProviderStatus {
            id: "none".to_string(),
            name: "翻译服务".to_string(),
            configured: false,
            model: None,
            configuration_hint: Some("请在设置中选择并配置翻译服务。".to_string()),
        }
    }

    fn translate<'a>(
        &'a self,
        _options: &'a TranslationOptions,
        _texts: &'a [String],
        _context: Option<&'a str>,
    ) -> TranslationFuture<'a> {
        Box::pin(async { Err(anyhow!("translation provider is not configured")) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        MutableTranslationProvider, TranslationFuture, TranslationProvider,
        TranslationProviderStatus, UnconfiguredTranslationProvider,
    };
    use crate::application::TranslationOptions;

    #[derive(Debug)]
    struct EchoProvider;

    impl TranslationProvider for EchoProvider {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: "echo".to_string(),
                name: "Echo".to_string(),
                configured: true,
                model: Some("test-v1".to_string()),
                configuration_hint: None,
            }
        }

        fn translate<'a>(
            &'a self,
            _options: &'a TranslationOptions,
            texts: &'a [String],
            _context: Option<&'a str>,
        ) -> TranslationFuture<'a> {
            let texts = texts.to_vec();
            Box::pin(async move { Ok(texts) })
        }
    }

    #[tokio::test]
    async fn replaces_the_active_provider_without_rebuilding_the_workspace_service() {
        let provider = MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider));
        assert!(!provider.status().configured);

        provider.replace(Arc::new(EchoProvider));
        assert_eq!(provider.status().id, "echo");
        assert_eq!(
            provider
                .translate(&TranslationOptions::default(), &["字幕".to_string()], None,)
                .await
                .unwrap(),
            ["字幕"]
        );
    }
}
