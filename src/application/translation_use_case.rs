use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};

use crate::application::{
    TranslationGroup, TranslationOptions, TranslationProvider, TranslationRequest,
    TranslationResponse, TranslationResult,
};

/// Executes one semantic translation group through the configured provider and
/// enforces the stable cue-ID contract shared by CLI and desktop workflows.
pub async fn execute_translation_group(
    provider: &dyn TranslationProvider,
    options: TranslationOptions,
    group: TranslationGroup,
    style_instruction: Option<String>,
) -> Result<TranslationResponse> {
    let status = provider.status();
    if !status.configured {
        return Err(anyhow!(
            "{} is not configured. {}",
            status.name,
            status
                .configuration_hint
                .as_deref()
                .unwrap_or("Configure the translation provider and restart Atogaki")
        ));
    }

    let expected_ids = group
        .targets
        .iter()
        .map(|target| target.segment_id.clone())
        .collect::<Vec<_>>();
    let response = provider
        .translate(TranslationRequest {
            options,
            before_context: group.before_context,
            targets: group.targets,
            after_context: group.after_context,
            style_instruction,
        })
        .await?;
    validate_translation_results(&status.name, &expected_ids, &response.translations)?;
    Ok(response)
}

fn validate_translation_results(
    provider_name: &str,
    expected_ids: &[String],
    translations: &[TranslationResult],
) -> Result<()> {
    if translations.len() != expected_ids.len() {
        return Err(anyhow!(
            "{} returned {} translations for {} subtitle cues",
            provider_name,
            translations.len(),
            expected_ids.len()
        ));
    }
    let expected = expected_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut returned = HashMap::with_capacity(translations.len());
    for translation in translations {
        if returned
            .insert(translation.segment_id.as_str(), translation)
            .is_some()
        {
            return Err(anyhow!("{} returned duplicate subtitle IDs", provider_name));
        }
        if !expected.contains(translation.segment_id.as_str()) {
            return Err(anyhow!(
                "{} returned an unexpected subtitle ID: {}",
                provider_name,
                translation.segment_id
            ));
        }
    }
    if let Some(missing) = expected_ids
        .iter()
        .find(|segment_id| !returned.contains_key(segment_id.as_str()))
    {
        return Err(anyhow!(
            "{} did not return translation for subtitle {}",
            provider_name,
            missing
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::execute_translation_group;
    use crate::application::{
        TranslationFuture, TranslationGroup, TranslationOptions, TranslationProvider,
        TranslationProviderStatus, TranslationResponse, TranslationResult,
        TranslationTargetSegment, TranslationUsage,
    };
    use crate::domain::LanguageCode;

    #[derive(Debug)]
    struct FixedProvider {
        translations: Vec<TranslationResult>,
    }

    impl TranslationProvider for FixedProvider {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: "fixed".to_string(),
                name: "Fixed".to_string(),
                configured: true,
                model: Some("fixed-v1".to_string()),
                endpoint_kind: "test".to_string(),
                configuration_hint: None,
            }
        }

        fn translate<'a>(
            &'a self,
            _request: crate::application::TranslationRequest,
        ) -> TranslationFuture<'a> {
            let translations = self.translations.clone();
            Box::pin(async move {
                Ok(TranslationResponse {
                    translations,
                    model: Some("fixed-v1".to_string()),
                    usage: TranslationUsage::default(),
                })
            })
        }
    }

    fn group() -> TranslationGroup {
        TranslationGroup {
            id: "group-1".to_string(),
            before_context: Vec::new(),
            targets: ["cue-1", "cue-2"]
                .into_iter()
                .map(|id| TranslationTargetSegment {
                    segment_id: id.to_string(),
                    source_text: id.to_string(),
                })
                .collect(),
            after_context: Vec::new(),
        }
    }

    #[tokio::test]
    async fn accepts_results_in_any_order_when_ids_are_complete() {
        let provider = FixedProvider {
            translations: vec![
                TranslationResult {
                    segment_id: "cue-2".to_string(),
                    translated_text: "二".to_string(),
                },
                TranslationResult {
                    segment_id: "cue-1".to_string(),
                    translated_text: "一".to_string(),
                },
            ],
        };

        let response = execute_translation_group(
            &provider,
            TranslationOptions::new(LanguageCode::Japanese, LanguageCode::SimplifiedChinese),
            group(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(response.translations.len(), 2);
    }

    #[tokio::test]
    async fn rejects_duplicate_or_missing_ids() {
        let provider = FixedProvider {
            translations: vec![
                TranslationResult {
                    segment_id: "cue-1".to_string(),
                    translated_text: "一".to_string(),
                },
                TranslationResult {
                    segment_id: "cue-1".to_string(),
                    translated_text: "重复".to_string(),
                },
            ],
        };

        let error = execute_translation_group(
            &provider,
            TranslationOptions::new(LanguageCode::Japanese, LanguageCode::SimplifiedChinese),
            group(),
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("duplicate subtitle IDs"));
    }
}
