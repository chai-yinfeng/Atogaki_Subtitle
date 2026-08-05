use std::{fmt, time::Duration};

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Deserialize;

use crate::{
    application::{
        TranslationFuture, TranslationOptions, TranslationProvider, TranslationProviderStatus,
    },
    domain::TranscriptSegment,
    infrastructure::network::NetworkClientConfig,
};

#[derive(Clone)]
pub struct DeepLTranslationProvider {
    auth_key: Option<String>,
    client: Client,
}

impl DeepLTranslationProvider {
    pub fn new(auth_key: Option<String>) -> Self {
        Self::with_network_config(auth_key, &NetworkClientConfig::environment())
            .expect("default DeepL HTTP client must be valid")
    }

    pub fn with_network_config(
        auth_key: Option<String>,
        network: &NetworkClientConfig,
    ) -> Result<Self> {
        let client = network
            .apply(Client::builder().timeout(Duration::from_secs(60)))?
            .build()
            .context("failed to build DeepL client")?;
        Ok(Self {
            auth_key: auth_key.filter(|key| !key.trim().is_empty()),
            client,
        })
    }
}

impl fmt::Debug for DeepLTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepLTranslationProvider")
            .field("configured", &self.auth_key.is_some())
            .finish()
    }
}

impl TranslationProvider for DeepLTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        TranslationProviderStatus {
            id: "deepl".to_string(),
            name: "DeepL".to_string(),
            configured: self.auth_key.is_some(),
            model: None,
            configuration_hint: self
                .auth_key
                .is_none()
                .then(|| "请在设置中配置 DeepL API Key。".to_string()),
        }
    }

    fn translate<'a>(
        &'a self,
        options: &'a TranslationOptions,
        texts: &'a [String],
        context: Option<&'a str>,
    ) -> TranslationFuture<'a> {
        Box::pin(async move {
            let auth_key = self
                .auth_key
                .as_deref()
                .ok_or_else(|| anyhow!("DeepL API key is not configured"))?;
            translate_lines(
                &self.client,
                deepl_endpoint(auth_key),
                auth_key,
                options,
                texts,
                context,
            )
            .await
        })
    }
}

#[derive(Debug, Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslation {
    text: String,
}

pub async fn translate_segments(
    auth_key: &str,
    options: &TranslationOptions,
    segments: &mut [TranscriptSegment],
) -> Result<()> {
    let texts: Vec<String> = segments.iter().map(|s| s.ja_text.clone()).collect();
    let translated = translate_texts(auth_key, options, &texts).await?;

    for (segment, zh) in segments.iter_mut().zip(translated) {
        segment.set_translation(Some(zh));
    }

    Ok(())
}

pub async fn translate_texts(
    auth_key: &str,
    options: &TranslationOptions,
    texts: &[String],
) -> Result<Vec<String>> {
    translate_texts_with_context(auth_key, options, texts, None).await
}

pub async fn translate_texts_with_context(
    auth_key: &str,
    options: &TranslationOptions,
    texts: &[String],
    context: Option<&str>,
) -> Result<Vec<String>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("failed to build DeepL client")?;
    translate_lines(
        &client,
        deepl_endpoint(auth_key),
        auth_key,
        options,
        texts,
        context,
    )
    .await
}

async fn translate_lines(
    client: &Client,
    endpoint: &str,
    auth_key: &str,
    options: &TranslationOptions,
    texts: &[String],
    context: Option<&str>,
) -> Result<Vec<String>> {
    const BATCH_SIZE: usize = 12;
    let mut out = Vec::with_capacity(texts.len());

    for batch in texts.chunks(BATCH_SIZE) {
        let body = translation_form(options, batch, context);

        let resp = client
            .post(endpoint)
            .header("Authorization", format!("DeepL-Auth-Key {auth_key}"))
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded; charset=utf-8",
            )
            .body(body)
            .send()
            .await
            .context("failed to call DeepL")?;

        let status = resp.status();
        let data = resp.text().await.context("failed to read DeepL response")?;
        if !status.is_success() {
            anyhow::bail!("DeepL failed with {status}: {data}");
        }

        let decoded: DeepLResponse =
            serde_json::from_str(&data).context("failed to parse DeepL response")?;
        if decoded.translations.len() != batch.len() {
            return Err(anyhow!(
                "DeepL returned {} translations for {} source lines",
                decoded.translations.len(),
                batch.len()
            ));
        }
        out.extend(decoded.translations.into_iter().map(|item| item.text));
    }

    Ok(out)
}

fn translation_form(
    options: &TranslationOptions,
    texts: &[String],
    context: Option<&str>,
) -> String {
    let mut body = format!(
        "source_lang={}&target_lang={}",
        urlencoding::encode(&options.source_language.to_ascii_uppercase()),
        urlencoding::encode(&options.target_language.to_ascii_uppercase())
    );
    for text in texts {
        body.push_str("&text=");
        body.push_str(&urlencoding::encode(text));
    }
    if let Some(context) = context.map(str::trim).filter(|context| !context.is_empty()) {
        body.push_str("&context=");
        body.push_str(&urlencoding::encode(context));
    }
    body
}

fn deepl_endpoint(auth_key: &str) -> &'static str {
    if auth_key.trim().ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

#[cfg(test)]
mod tests {
    use super::{DeepLTranslationProvider, deepl_endpoint, translate_texts, translation_form};
    use crate::application::{TranslationOptions, TranslationProvider};

    #[test]
    fn provider_status_reports_configuration_without_exposing_the_key() {
        let provider = DeepLTranslationProvider::new(Some("secret-key:fx".to_string()));
        let status = provider.status();

        assert_eq!(status.id, "deepl");
        assert_eq!(status.name, "DeepL");
        assert!(status.configured);
        assert_eq!(status.model, None);
        assert_eq!(status.configuration_hint, None);
        assert!(!format!("{provider:?}").contains("secret-key"));
    }

    #[test]
    fn blank_key_leaves_the_provider_unconfigured() {
        let provider = DeepLTranslationProvider::new(Some("  ".to_string()));
        let status = provider.status();

        assert!(!status.configured);
        assert!(status.configuration_hint.is_some());
    }

    #[test]
    fn selects_the_endpoint_for_free_and_pro_keys() {
        assert_eq!(
            deepl_endpoint("free-key:fx"),
            "https://api-free.deepl.com/v2/translate"
        );
        assert_eq!(
            deepl_endpoint("pro-key"),
            "https://api.deepl.com/v2/translate"
        );
    }

    #[test]
    fn adds_one_shared_context_to_a_translation_request() {
        let body = translation_form(
            &TranslationOptions::default(),
            &["一行目".to_string(), "二行目".to_string()],
            Some("前の話\n後の話"),
        );

        assert_eq!(body.matches("&text=").count(), 2);
        assert_eq!(body.matches("&context=").count(), 1);
        assert!(body.contains("%E5%89%8D%E3%81%AE%E8%A9%B1%0A%E5%BE%8C%E3%81%AE%E8%A9%B1"));
    }

    #[test]
    fn omits_blank_translation_context() {
        let body = translation_form(
            &TranslationOptions::default(),
            &["一行目".to_string()],
            Some("  \n"),
        );

        assert!(!body.contains("&context="));
    }

    #[tokio::test]
    #[ignore = "uses the configured DeepL account and network"]
    async fn configured_account_translates_japanese_to_simplified_chinese() {
        let key = std::env::var("DEEPL_AUTH_KEY").expect("DEEPL_AUTH_KEY is required");
        let translated = translate_texts(
            &key,
            &TranslationOptions::default(),
            &["こんにちは。".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(translated.len(), 1);
        assert!(!translated[0].trim().is_empty());
    }
}
