use std::{collections::HashMap, fmt, future::Future, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;

use crate::{
    application::{
        TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
        TranslationResponse, TranslationResult, TranslationUsage,
    },
    infrastructure::network::NetworkClientConfig,
};

#[derive(Clone)]
pub struct OpenAiCompatibleTranslationProvider {
    provider_id: String,
    provider_name: String,
    api_key: Option<String>,
    base_url: Url,
    model: String,
    style_instruction: String,
    disable_deepseek_thinking: bool,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleConfig {
    pub provider_id: String,
    pub provider_name: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub style_instruction: String,
    pub disable_deepseek_thinking: bool,
}

impl OpenAiCompatibleTranslationProvider {
    pub fn with_network_config(
        config: OpenAiCompatibleConfig,
        network: &NetworkClientConfig,
    ) -> Result<Self> {
        let base_url = validate_openai_base_url(&config.base_url)?;
        let model = config.model.trim().to_string();
        if model.is_empty() {
            bail!("OpenAI-compatible model cannot be empty");
        }
        let client = network
            .apply(Client::builder().timeout(Duration::from_secs(120)))?
            .build()
            .context("failed to build OpenAI-compatible client")?;
        Ok(Self {
            provider_id: config.provider_id,
            provider_name: config.provider_name,
            api_key: config.api_key.filter(|key| !key.trim().is_empty()),
            base_url,
            model,
            style_instruction: config.style_instruction.trim().to_string(),
            disable_deepseek_thinking: config.disable_deepseek_thinking,
            client,
        })
    }

    async fn translate_once(&self, request: &TranslationRequest) -> Result<TranslationResponse> {
        if request.targets.is_empty() {
            return Ok(TranslationResponse {
                translations: Vec::new(),
                model: Some(self.model.clone()),
                usage: TranslationUsage::default(),
            });
        }
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow!("{} API key is not configured", self.provider_name))?;
        let prepared = request
            .targets
            .iter()
            .map(|target| {
                let protected =
                    protect_terms(&target.source_text, &request.options.protected_terms);
                (target.segment_id.clone(), protected)
            })
            .collect::<Vec<_>>();
        let body = self.build_request_body(request, &prepared);
        let prepared_by_id = prepared.into_iter().collect::<HashMap<_, _>>();
        let endpoint = self
            .base_url
            .join("chat/completions")
            .context("failed to build OpenAI-compatible chat endpoint")?;
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to call {}", self.provider_name))?;
        let status = response.status();
        let response_text = response
            .text()
            .await
            .with_context(|| format!("failed to read {} response", self.provider_name))?;
        if !status.is_success() {
            bail!(
                "{} failed with {}: {}",
                self.provider_name,
                status,
                response_text
            );
        }
        self.decode_response(&response_text, &prepared_by_id)
    }

    fn build_request_body(
        &self,
        request: &TranslationRequest,
        prepared: &[(String, ProtectedText)],
    ) -> serde_json::Value {
        let user_payload = json!({
            "source_language": request.options.source_language.as_str(),
            "target_language": request.options.target_language.as_str(),
            "before_context": request.before_context.iter().map(|segment| json!({
                "segment_id": segment.segment_id,
                "source_text": segment.source_text,
            })).collect::<Vec<_>>(),
            "target_segments": prepared.iter().map(|(segment_id, protected)| json!({
                "segment_id": segment_id,
                "source_text": protected.request_text,
            })).collect::<Vec<_>>(),
            "after_context": request.after_context.iter().map(|segment| json!({
                "segment_id": segment.segment_id,
                "source_text": segment.source_text,
            })).collect::<Vec<_>>(),
        });
        let style = request
            .style_instruction
            .as_deref()
            .filter(|style| !style.trim().is_empty())
            .unwrap_or(&self.style_instruction);
        let system_prompt = format!(
            "You translate segmented spoken-language subtitles into {}. Use the surrounding context only to resolve meaning; do not translate or return context segments. Preserve every target segment_id exactly once. Preserve every token shaped like [[ATOGAKI_TERM_N]] exactly. Return JSON only with this schema: {{\"translations\":[{{\"segment_id\":\"...\",\"translated_text\":\"...\"}}]}}. Do not merge, split, omit, reorder semantically, or add commentary. Translation style: {}",
            request.options.target_language.display_name_zh(),
            if style.is_empty() {
                "accurate and natural spoken subtitles"
            } else {
                style
            },
        );
        let mut body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": format!("Translate the target_segments in this JSON input:\n{}", user_payload)},
            ],
            "response_format": {"type": "json_object"},
            "stream": false,
        });
        if self.disable_deepseek_thinking {
            body["thinking"] = json!({"type": "disabled"});
        }
        body
    }

    fn decode_response(
        &self,
        response_text: &str,
        prepared_by_id: &HashMap<String, ProtectedText>,
    ) -> Result<TranslationResponse> {
        let response: ChatCompletionResponse = serde_json::from_str(response_text)
            .with_context(|| format!("failed to parse {} response", self.provider_name))?;
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| anyhow!("{} returned empty JSON content", self.provider_name))?;
        let decoded: StructuredTranslations = serde_json::from_str(content)
            .with_context(|| format!("failed to parse {} translation JSON", self.provider_name))?;
        let translations = decoded
            .translations
            .into_iter()
            .map(|translation| {
                let protected = prepared_by_id.get(&translation.segment_id).ok_or_else(|| {
                    anyhow!(
                        "{} returned unknown subtitle ID {}",
                        self.provider_name,
                        translation.segment_id
                    )
                })?;
                Ok(TranslationResult {
                    segment_id: translation.segment_id,
                    translated_text: restore_terms(&translation.translated_text, protected)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(TranslationResponse {
            translations,
            model: response.model.or_else(|| Some(self.model.clone())),
            usage: TranslationUsage {
                input_tokens: response
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.prompt_tokens),
                output_tokens: response
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.completion_tokens),
            },
        })
    }
}

impl fmt::Debug for OpenAiCompatibleTranslationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiCompatibleTranslationProvider")
            .field("provider_id", &self.provider_id)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("configured", &self.api_key.is_some())
            .finish()
    }
}

impl TranslationProvider for OpenAiCompatibleTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        TranslationProviderStatus {
            id: self.provider_id.clone(),
            name: self.provider_name.clone(),
            configured: self.api_key.is_some(),
            model: Some(self.model.clone()),
            endpoint_kind: "openai-chat-completions".to_string(),
            configuration_hint: self
                .api_key
                .is_none()
                .then(|| format!("请在设置中配置 {} API Key。", self.provider_name)),
        }
    }

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(
            async move { retry_once(&self.provider_name, || self.translate_once(&request)).await },
        )
    }
}

async fn retry_once<T, F, Fut>(provider_name: &str, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    match operation().await {
        Ok(value) => Ok(value),
        Err(first_error) => operation().await.with_context(|| {
            format!(
                "{provider_name} translation failed after one retry; first error: {first_error:#}"
            )
        }),
    }
}

fn validate_openai_base_url(value: &str) -> Result<Url> {
    let value = format!("{}/", value.trim().trim_end_matches('/'));
    let url = Url::parse(&value).context("invalid OpenAI-compatible base URL")?;
    let loopback = url
        .host_str()
        .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        bail!("OpenAI-compatible base URL must use HTTPS or loopback HTTP");
    }
    if url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("OpenAI-compatible base URL cannot contain credentials, query, or fragment");
    }
    Ok(url)
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    model: Option<String>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct StructuredTranslations {
    translations: Vec<StructuredTranslation>,
}

#[derive(Debug, Deserialize)]
struct StructuredTranslation {
    segment_id: String,
    translated_text: String,
}

#[derive(Debug)]
struct ProtectedText {
    request_text: String,
    terms: Vec<String>,
}

fn protect_terms(text: &str, protected_terms: &[String]) -> ProtectedText {
    let mut request_text = String::new();
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
            request_text.push_str(remaining);
            break;
        };
        request_text.push_str(&remaining[..index]);
        request_text.push_str(&format!("[[ATOGAKI_TERM_{}]]", terms.len()));
        terms.push((*term).clone());
        remaining = &remaining[index + term.len()..];
    }
    ProtectedText {
        request_text,
        terms,
    }
}

fn restore_terms(text: &str, protected: &ProtectedText) -> Result<String> {
    let mut restored = text.to_string();
    for (index, term) in protected.terms.iter().enumerate() {
        let token = format!("[[ATOGAKI_TERM_{index}]]");
        if restored.matches(&token).count() != 1 {
            bail!("translation did not preserve protected term token {token}");
        }
        restored = restored.replace(&token, term);
    }
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use serde_json::json;

    use super::{
        OpenAiCompatibleConfig, OpenAiCompatibleTranslationProvider, protect_terms, restore_terms,
        retry_once, validate_openai_base_url,
    };
    use crate::{
        application::{TranslationRequest, TranslationTargetSegment},
        infrastructure::network::NetworkClientConfig,
    };

    #[test]
    fn validates_cloud_https_and_loopback_http_only() {
        assert!(validate_openai_base_url("https://api.deepseek.com").is_ok());
        assert!(validate_openai_base_url("http://127.0.0.1:11434/v1").is_ok());
        assert!(validate_openai_base_url("http://example.com/v1").is_err());
        assert!(validate_openai_base_url("https://user:secret@example.com").is_err());
    }

    #[test]
    fn protected_terms_round_trip_through_opaque_tokens() {
        let protected = protect_terms("盗作とsuis", &["盗作".to_string(), "suis".to_string()]);
        assert_eq!(
            protected.request_text,
            "[[ATOGAKI_TERM_0]]と[[ATOGAKI_TERM_1]]"
        );
        assert_eq!(
            restore_terms("[[ATOGAKI_TERM_0]]和[[ATOGAKI_TERM_1]]", &protected).unwrap(),
            "盗作和suis"
        );
        assert!(restore_terms("缺少占位符", &protected).is_err());
    }

    #[tokio::test]
    async fn retries_a_failed_provider_operation_exactly_once() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let result = retry_once("Test Provider", || {
            let attempts = Arc::clone(&attempts);
            async move {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    anyhow::bail!("temporary failure");
                }
                Ok("translated")
            }
        })
        .await
        .unwrap();

        assert_eq!(result, "translated");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn builds_and_decodes_structured_translation_contract_without_a_socket() {
        let provider = OpenAiCompatibleTranslationProvider::with_network_config(
            OpenAiCompatibleConfig {
                provider_id: "deepseek".to_string(),
                provider_name: "DeepSeek".to_string(),
                api_key: Some("secret".to_string()),
                base_url: "https://api.deepseek.com".to_string(),
                model: "deepseek-v4-flash".to_string(),
                style_instruction: "自然口语".to_string(),
                disable_deepseek_thinking: true,
            },
            &NetworkClientConfig::new("direct", None).unwrap(),
        )
        .unwrap();
        let request = TranslationRequest {
            options: crate::application::TranslationOptions::default()
                .with_protected_terms(["盗作".to_string()]),
            before_context: Vec::new(),
            targets: vec![TranslationTargetSegment {
                segment_id: "s1".to_string(),
                source_text: "盗作の新作".to_string(),
            }],
            after_context: Vec::new(),
            style_instruction: None,
        };
        let protected = protect_terms("盗作の新作", &request.options.protected_terms);
        let prepared = vec![("s1".to_string(), protected)];
        let body = provider.build_request_body(&request, &prepared);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(
            body["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("[[ATOGAKI_TERM_0]]")
        );

        let prepared_by_id = prepared.into_iter().collect::<HashMap<_, _>>();
        let response = provider
            .decode_response(
                &json!({
                    "model": "deepseek-v4-flash-202608",
                    "choices": [{
                        "message": {
                            "content": "{\"translations\":[{\"segment_id\":\"s1\",\"translated_text\":\"[[ATOGAKI_TERM_0]]的新作品\"}]}"
                        }
                    }],
                    "usage": {"prompt_tokens": 120, "completion_tokens": 18}
                })
                .to_string(),
                &prepared_by_id,
            )
            .unwrap();
        assert_eq!(response.translations[0].segment_id, "s1");
        assert_eq!(response.translations[0].translated_text, "盗作的新作品");
        assert_eq!(response.model.as_deref(), Some("deepseek-v4-flash-202608"));
        assert_eq!(response.usage.input_tokens, Some(120));
        assert_eq!(response.usage.output_tokens, Some(18));
    }
}
