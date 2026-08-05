use std::{fmt::Debug, future::Future, pin::Pin};

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
