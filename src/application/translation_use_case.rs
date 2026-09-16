use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;

use crate::application::{
    TRANSLATION_GROUPING_STRATEGY, TranslationGroup, TranslationOptions, TranslationPlanner,
    TranslationPlannerCue, TranslationProvider, TranslationRequest, TranslationResponse,
    TranslationResult,
};
use crate::domain::TranscriptSegment;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TranslationExecutionSummary {
    pub grouping_strategy: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: Option<String>,
    pub group_count: usize,
    pub segment_count: usize,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

/// Translates a complete in-memory cue timeline without partially mutating it
/// when a later semantic group fails. Used by the CLI and quality harness.
pub async fn translate_transcript(
    provider: &dyn TranslationProvider,
    options: &TranslationOptions,
    segments: &mut [TranscriptSegment],
) -> Result<TranslationExecutionSummary> {
    let timeline = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            Ok(TranslationPlannerCue {
                segment_id: segment.id.clone(),
                segment_index: i64::try_from(index)
                    .context("subtitle index exceeds translation planner range")?,
                start_ms: i64::try_from(segment.start_ms)
                    .context("subtitle start time exceeds translation planner range")?,
                end_ms: i64::try_from(segment.end_ms)
                    .context("subtitle end time exceeds translation planner range")?,
                source_text: segment.source_text.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let target_ids = timeline
        .iter()
        .map(|cue| cue.segment_id.clone())
        .collect::<HashSet<_>>();
    let plan = TranslationPlanner::default().plan(&timeline, &target_ids)?;
    let status = provider.status();
    let group_count = plan.groups.len();
    let mut translated_by_id = HashMap::with_capacity(segments.len());
    let mut response_model = status.model.clone();
    let mut input_tokens = None;
    let mut output_tokens = None;
    for group in plan.groups {
        let response = execute_translation_group(provider, options.clone(), group, None)
            .await
            .with_context(|| format!("failed to translate subtitle group with {}", status.name))?;
        response_model = response.model.or(response_model);
        accumulate_optional(&mut input_tokens, response.usage.input_tokens);
        accumulate_optional(&mut output_tokens, response.usage.output_tokens);
        translated_by_id.extend(
            response
                .translations
                .into_iter()
                .map(|translation| (translation.segment_id, translation.translated_text)),
        );
    }
    for segment in segments.iter_mut() {
        let translated = translated_by_id.remove(&segment.id).ok_or_else(|| {
            anyhow!(
                "translation plan did not produce a result for subtitle {}",
                segment.id
            )
        })?;
        segment.set_translation(Some(translated));
    }
    Ok(TranslationExecutionSummary {
        grouping_strategy: TRANSLATION_GROUPING_STRATEGY.to_string(),
        provider_id: status.id,
        provider_name: status.name,
        model: response_model,
        group_count,
        segment_count: segments.len(),
        input_tokens,
        output_tokens,
    })
}

fn accumulate_optional(total: &mut Option<i64>, value: Option<i64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or_default().saturating_add(value));
    }
}

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
    use super::{execute_translation_group, translate_transcript};
    use crate::application::{
        TranslationFuture, TranslationGroup, TranslationOptions, TranslationProvider,
        TranslationProviderStatus, TranslationResponse, TranslationResult,
        TranslationTargetSegment, TranslationUsage,
    };
    use crate::domain::{LanguageCode, TranscriptSegment};

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

    #[tokio::test]
    async fn complete_timeline_waits_for_all_groups_before_mutating_cues() {
        let provider = FixedProvider {
            translations: vec![TranslationResult {
                segment_id: "unexpected".to_string(),
                translated_text: "错误".to_string(),
            }],
        };
        let mut segments = vec![TranscriptSegment::new(0, 1_000, "第一句。".into())];
        let original = segments[0].translated_text.clone();

        assert!(
            translate_transcript(
                &provider,
                &TranslationOptions::new(LanguageCode::Japanese, LanguageCode::SimplifiedChinese),
                &mut segments,
            )
            .await
            .is_err()
        );
        assert_eq!(segments[0].translated_text, original);
    }
}
