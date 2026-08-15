use std::{
    fmt::{self, Debug},
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::application::TranslationOptions;

pub type TranslationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TranslationResponse>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TranslationProviderStatus {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub model: Option<String>,
    pub endpoint_kind: String,
    pub configuration_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationContextSegment {
    pub segment_id: String,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationTargetSegment {
    pub segment_id: String,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub options: TranslationOptions,
    pub before_context: Vec<TranslationContextSegment>,
    pub targets: Vec<TranslationTargetSegment>,
    pub after_context: Vec<TranslationContextSegment>,
    pub style_instruction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TranslationResult {
    pub segment_id: String,
    pub translated_text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationResponse {
    pub translations: Vec<TranslationResult>,
    pub model: Option<String>,
    pub usage: TranslationUsage,
}

pub trait TranslationProvider: Debug + Send + Sync {
    fn status(&self) -> TranslationProviderStatus;

    /// Translate the target segments while preserving their stable IDs.
    ///
    /// The application supplies semantic before/after context. Each adapter
    /// decides how to encode that context, but must return exactly one non-empty
    /// result for every target ID. Transactional persistence and source
    /// fingerprint checks remain the responsibility of the application service.
    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a>;
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

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        let provider = self.current();
        Box::pin(async move { provider.translate(request).await })
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
            endpoint_kind: "none".to_string(),
            configuration_hint: Some("请在设置中选择并配置翻译服务。".to_string()),
        }
    }

    fn translate<'a>(&'a self, _request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async { Err(anyhow!("translation provider is not configured")) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        MutableTranslationProvider, TranslationFuture, TranslationProvider,
        TranslationProviderStatus, TranslationRequest, TranslationResponse, TranslationResult,
        TranslationTargetSegment, TranslationUsage, UnconfiguredTranslationProvider,
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
                endpoint_kind: "test".to_string(),
                configuration_hint: None,
            }
        }

        fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
            Box::pin(async move {
                Ok(TranslationResponse {
                    translations: request
                        .targets
                        .into_iter()
                        .map(|target| TranslationResult {
                            segment_id: target.segment_id,
                            translated_text: target.source_text,
                        })
                        .collect(),
                    model: Some("test-v1".to_string()),
                    usage: TranslationUsage::default(),
                })
            })
        }
    }

    #[tokio::test]
    async fn replaces_the_active_provider_without_rebuilding_the_workspace_service() {
        let provider = MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider));
        assert!(!provider.status().configured);

        provider.replace(Arc::new(EchoProvider));
        assert_eq!(provider.status().id, "echo");
        let response = provider
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
            .unwrap();
        assert_eq!(response.translations[0].translated_text, "字幕");
    }
}
