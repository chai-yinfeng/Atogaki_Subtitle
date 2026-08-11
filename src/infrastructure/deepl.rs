use std::{fmt, time::Duration};

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Deserialize;

use crate::{
    application::{
        TranslationFuture, TranslationOptions, TranslationProvider, TranslationProviderStatus,
    },
    domain::{LanguagePair, TranscriptSegment},
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
    let texts: Vec<String> = segments.iter().map(|s| s.source_text.clone()).collect();
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
    LanguagePair::new(options.source_language, options.target_language)
        .context("invalid DeepL language pair")?;
    let mut out = Vec::with_capacity(texts.len());

    for batch in texts.chunks(BATCH_SIZE) {
        let prepared = batch
            .iter()
            .map(|text| protect_translation_terms(text, &options.protected_terms))
            .collect::<Vec<_>>();
        let request_texts = prepared
            .iter()
            .map(|text| text.request_text.clone())
            .collect::<Vec<_>>();
        let body = translation_form(options, &request_texts, context);

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
        out.extend(
            decoded
                .translations
                .into_iter()
                .zip(prepared.iter())
                .map(|(item, protected)| restore_translation_terms(&item.text, protected)),
        );
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
        urlencoding::encode(options.source_language.deepl_source_code()),
        urlencoding::encode(options.target_language.deepl_target_code())
    );
    for text in texts {
        body.push_str("&text=");
        body.push_str(&urlencoding::encode(text));
    }
    if !options.protected_terms.is_empty() {
        body.push_str("&tag_handling=xml&ignore_tags=atogaki-term&non_splitting_tags=atogaki-term");
    }
    if let Some(context) = context.map(str::trim).filter(|context| !context.is_empty()) {
        body.push_str("&context=");
        let context = options
            .protected_terms
            .is_empty()
            .then_some(context.to_string())
            .unwrap_or_else(|| escape_xml(context));
        body.push_str(&urlencoding::encode(&context));
    }
    body
}

#[derive(Debug)]
struct ProtectedTranslationText {
    request_text: String,
    terms: Vec<String>,
}

fn protect_translation_terms(text: &str, protected_terms: &[String]) -> ProtectedTranslationText {
    if protected_terms.is_empty() {
        return ProtectedTranslationText {
            request_text: text.to_string(),
            terms: Vec::new(),
        };
    }
    let mut request_text = String::with_capacity(text.len());
    let mut terms = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        let found = protected_terms
            .iter()
            .filter_map(|term| remaining.find(term).map(|index| (index, term)))
            .min_by(|(left_index, left_term), (right_index, right_term)| {
                left_index
                    .cmp(right_index)
                    .then_with(|| right_term.len().cmp(&left_term.len()))
            });
        let Some((index, term)) = found else {
            request_text.push_str(&escape_xml(remaining));
            break;
        };
        request_text.push_str(&escape_xml(&remaining[..index]));
        let id = terms.len();
        request_text.push_str(&format!("<atogaki-term id=\"{id}\"/>"));
        terms.push((*term).clone());
        remaining = &remaining[index + term.len()..];
    }

    ProtectedTranslationText {
        request_text,
        terms,
    }
}

fn restore_translation_terms(text: &str, protected: &ProtectedTranslationText) -> String {
    let mut restored = text.to_string();
    for (id, term) in protected.terms.iter().enumerate() {
        let compact = format!("<atogaki-term id=\"{id}\"/>");
        let spaced = format!("<atogaki-term id=\"{id}\" />");
        restored = restored.replace(&compact, term).replace(&spaced, term);
    }
    unescape_xml(&restored)
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
    use super::{
        DeepLTranslationProvider, deepl_endpoint, protect_translation_terms,
        restore_translation_terms, translate_texts, translation_form,
    };
    use crate::application::{TranslationOptions, TranslationProvider};
    use crate::domain::LanguageCode;

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
    fn maps_english_to_simplified_chinese_provider_codes() {
        let body = translation_form(
            &TranslationOptions::new(LanguageCode::English, LanguageCode::SimplifiedChinese),
            &["Good evening".to_string()],
            None,
        );

        assert!(body.starts_with("source_lang=EN&target_lang=ZH-HANS"));
    }

    #[test]
    fn maps_korean_to_simplified_chinese_provider_codes() {
        let body = translation_form(
            &TranslationOptions::new(LanguageCode::Korean, LanguageCode::SimplifiedChinese),
            &["안녕하세요".to_string()],
            None,
        );

        assert!(body.starts_with("source_lang=KO&target_lang=ZH-HANS"));
    }

    #[test]
    fn preserves_selected_terms_with_deepl_xml_tags() {
        let options = TranslationOptions::default()
            .with_protected_terms(["盗作".to_string(), "ヨルシカ".to_string()]);
        let protected = protect_translation_terms("ヨルシカの盗作を聴く", &options.protected_terms);
        let body = translation_form(&options, &[protected.request_text.clone()], None);

        assert_eq!(
            protected.request_text,
            "<atogaki-term id=\"0\"/>の<atogaki-term id=\"1\"/>を聴く"
        );
        assert!(body.contains("tag_handling=xml"));
        assert!(body.contains("ignore_tags=atogaki-term"));
        assert_eq!(
            restore_translation_terms(
                "听<atogaki-term id=\"0\" />的<atogaki-term id=\"1\"/>",
                &protected
            ),
            "听ヨルシカ的盗作"
        );
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
