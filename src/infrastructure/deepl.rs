use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::domain::TranscriptSegment;

#[derive(Debug, Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslation {
    text: String,
}

pub async fn translate_segments(auth_key: &str, segments: &mut [TranscriptSegment]) -> Result<()> {
    let client = Client::new();
    let texts: Vec<String> = segments.iter().map(|s| s.ja_text.clone()).collect();
    let translated = translate_lines(&client, auth_key, &texts).await?;

    for (segment, zh) in segments.iter_mut().zip(translated) {
        segment.zh_text = Some(zh);
    }

    Ok(())
}

async fn translate_lines(client: &Client, auth_key: &str, texts: &[String]) -> Result<Vec<String>> {
    const BATCH_SIZE: usize = 12;
    let mut out = Vec::with_capacity(texts.len());

    for batch in texts.chunks(BATCH_SIZE) {
        let mut body = String::from("source_lang=JA&target_lang=ZH");
        for text in batch {
            body.push_str("&text=");
            body.push_str(&urlencoding::encode(text));
        }

        let resp = client
            .post("https://api-free.deepl.com/v2/translate")
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
        out.extend(decoded.translations.into_iter().map(|item| item.text));
    }

    Ok(out)
}
