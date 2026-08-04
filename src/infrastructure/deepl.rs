use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Deserialize;

use crate::{application::TranslationOptions, domain::TranscriptSegment};

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
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("failed to build DeepL client")?;
    translate_lines(&client, deepl_endpoint(auth_key), auth_key, options, texts).await
}

async fn translate_lines(
    client: &Client,
    endpoint: &str,
    auth_key: &str,
    options: &TranslationOptions,
    texts: &[String],
) -> Result<Vec<String>> {
    const BATCH_SIZE: usize = 12;
    let mut out = Vec::with_capacity(texts.len());

    for batch in texts.chunks(BATCH_SIZE) {
        let mut body = format!(
            "source_lang={}&target_lang={}",
            urlencoding::encode(&options.source_language.to_ascii_uppercase()),
            urlencoding::encode(&options.target_language.to_ascii_uppercase())
        );
        for text in batch {
            body.push_str("&text=");
            body.push_str(&urlencoding::encode(text));
        }

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

fn deepl_endpoint(auth_key: &str) -> &'static str {
    if auth_key.trim().ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

#[cfg(test)]
mod tests {
    use super::{deepl_endpoint, translate_texts};
    use crate::application::TranslationOptions;

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
