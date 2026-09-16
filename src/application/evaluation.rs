use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::domain::{LanguageCode, TranscriptSegment};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvaluationTranscript {
    pub version: u32,
    pub language: LanguageCode,
    pub cues: Vec<EvaluationCue>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvaluationCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    #[serde(default)]
    pub review_status: ReferenceReviewStatus,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceReviewStatus {
    #[default]
    Verified,
    Uncertain,
    Overlap,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AsrEvaluationReport {
    pub language: LanguageCode,
    pub primary_metric: &'static str,
    pub primary_error_rate: f64,
    pub reference_units: usize,
    pub edit_distance: usize,
    pub scored_cues: usize,
    pub excluded_cues: usize,
    pub normalized_reference: String,
    pub normalized_hypothesis: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TranscriptInput {
    Reference(EvaluationTranscript),
    Segments(Vec<TranscriptSegment>),
}

pub fn evaluate_asr_files(reference: &Path, hypothesis: &Path) -> Result<AsrEvaluationReport> {
    let reference = read_input(reference)?;
    let hypothesis = read_input(hypothesis)?;
    let TranscriptInput::Reference(reference) = reference else {
        bail!("reference must use the versioned evaluation transcript format");
    };
    if let TranscriptInput::Reference(transcript) = &hypothesis
        && transcript.language != reference.language
    {
        bail!(
            "hypothesis language {} does not match reference language {}",
            transcript.language,
            reference.language
        );
    }
    let scored_ranges = reference
        .cues
        .iter()
        .filter(|cue| cue.review_status == ReferenceReviewStatus::Verified)
        .map(|cue| (cue.start_ms, cue.end_ms))
        .collect::<Vec<_>>();
    let hypothesis_text = timed_texts(hypothesis)
        .into_iter()
        .filter(|(start_ms, end_ms, _)| {
            scored_ranges
                .iter()
                .any(|(start, end)| start_ms < end && end_ms > start)
        })
        .map(|(_, _, text)| text)
        .collect::<Vec<_>>()
        .join(" ");
    evaluate_asr(&reference, &hypothesis_text)
}

pub fn evaluate_asr(
    reference: &EvaluationTranscript,
    hypothesis: &str,
) -> Result<AsrEvaluationReport> {
    if reference.version != 1 {
        bail!(
            "unsupported evaluation transcript version: {}",
            reference.version
        );
    }
    let mut scored = Vec::new();
    let mut excluded_cues = 0;
    for cue in &reference.cues {
        if cue.end_ms <= cue.start_ms {
            bail!(
                "reference cue must have a positive duration: {}..{}",
                cue.start_ms,
                cue.end_ms
            );
        }
        if cue.review_status == ReferenceReviewStatus::Verified {
            scored.push(cue.text.as_str());
        } else {
            excluded_cues += 1;
        }
    }
    if scored.is_empty() {
        bail!("reference has no verified cues to score");
    }
    let source = scored.join(" ");
    let (primary_metric, normalized_reference, normalized_hypothesis, edit_distance, units) =
        match reference.language {
            LanguageCode::English => {
                let reference = normalize_english_words(&source);
                let hypothesis = normalize_english_words(hypothesis);
                let distance = levenshtein(&reference, &hypothesis);
                let units = reference.len();
                (
                    "wer",
                    reference.join(" "),
                    hypothesis.join(" "),
                    distance,
                    units,
                )
            }
            _ => {
                let reference = normalize_characters(&source);
                let hypothesis = normalize_characters(hypothesis);
                let distance = levenshtein(&reference, &hypothesis);
                let units = reference.len();
                (
                    "cer",
                    reference.into_iter().collect(),
                    hypothesis.into_iter().collect(),
                    distance,
                    units,
                )
            }
        };
    if units == 0 {
        bail!("reference is empty after normalization");
    }
    Ok(AsrEvaluationReport {
        language: reference.language,
        primary_metric,
        primary_error_rate: edit_distance as f64 / units as f64,
        reference_units: units,
        edit_distance,
        scored_cues: scored.len(),
        excluded_cues,
        normalized_reference,
        normalized_hypothesis,
    })
}

fn read_input(path: &Path) -> Result<TranscriptInput> {
    let data = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn timed_texts(input: TranscriptInput) -> Vec<(u64, u64, String)> {
    match input {
        TranscriptInput::Reference(transcript) => transcript
            .cues
            .into_iter()
            .map(|cue| (cue.start_ms, cue.end_ms, cue.text))
            .collect(),
        TranscriptInput::Segments(segments) => segments
            .into_iter()
            .map(|segment| (segment.start_ms, segment.end_ms, segment.source_text))
            .collect(),
    }
}

fn normalize_english_words(text: &str) -> Vec<String> {
    text.nfkc()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() || character == '\'' {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn normalize_characters(text: &str) -> Vec<char> {
    text.nfkc()
        .filter(|character| !character.is_whitespace() && !is_scoring_punctuation(*character))
        .collect()
}

fn is_scoring_punctuation(character: char) -> bool {
    matches!(
        character,
        '。' | '、'
            | '！'
            | '？'
            | '!'
            | '?'
            | '.'
            | ','
            | ':'
            | ';'
            | '：'
            | '；'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
            | '('
            | ')'
            | '['
            | ']'
            | '【'
            | '】'
            | '…'
            | '・'
            | '—'
            | '-'
            | '～'
            | '~'
            | '"'
            | '“'
            | '”'
            | '\''
            | '‘'
            | '’'
    )
}

fn levenshtein<T: Eq>(left: &[T], right: &[T]) -> usize {
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_value) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_value) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_value != right_value);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use crate::domain::LanguageCode;

    use super::{
        EvaluationCue, EvaluationTranscript, ReferenceReviewStatus, evaluate_asr,
        normalize_characters,
    };

    #[test]
    fn japanese_cer_uses_nfkc_and_ignores_layout_punctuation() {
        let reference = EvaluationTranscript {
            version: 1,
            language: LanguageCode::Japanese,
            cues: vec![EvaluationCue {
                start_ms: 0,
                end_ms: 1_000,
                text: "こんばんは、世界！".to_string(),
                review_status: ReferenceReviewStatus::Verified,
                notes: None,
            }],
        };

        let report = evaluate_asr(&reference, "こんばんは世界").unwrap();
        assert_eq!(report.primary_metric, "cer");
        assert_eq!(report.primary_error_rate, 0.0);
        assert_eq!(normalize_characters("Ａ Ｂ。"), vec!['A', 'B']);
    }

    #[test]
    fn uncertain_reference_cues_are_excluded() {
        let reference = EvaluationTranscript {
            version: 1,
            language: LanguageCode::Japanese,
            cues: vec![
                EvaluationCue {
                    start_ms: 0,
                    end_ms: 1_000,
                    text: "確かな文".to_string(),
                    review_status: ReferenceReviewStatus::Verified,
                    notes: None,
                },
                EvaluationCue {
                    start_ms: 1_000,
                    end_ms: 2_000,
                    text: "不明".to_string(),
                    review_status: ReferenceReviewStatus::Uncertain,
                    notes: Some("聞き取れない".to_string()),
                },
            ],
        };

        let report = evaluate_asr(&reference, "確かな文").unwrap();
        assert_eq!(report.primary_error_rate, 0.0);
        assert_eq!(report.scored_cues, 1);
        assert_eq!(report.excluded_cues, 1);
    }

    #[test]
    fn english_primary_metric_is_word_error_rate() {
        let reference = EvaluationTranscript {
            version: 1,
            language: LanguageCode::English,
            cues: vec![EvaluationCue {
                start_ms: 0,
                end_ms: 1_000,
                text: "It's a quiet night.".to_string(),
                review_status: ReferenceReviewStatus::Verified,
                notes: None,
            }],
        };

        let report = evaluate_asr(&reference, "it's quiet night").unwrap();
        assert_eq!(report.primary_metric, "wer");
        assert_eq!(report.reference_units, 4);
        assert_eq!(report.edit_distance, 1);
    }
}
