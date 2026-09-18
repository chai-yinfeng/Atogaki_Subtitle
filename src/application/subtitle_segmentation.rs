use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    application::{TimedUnit, TimedUnitKind},
    domain::{LanguageCode, TranscriptSegment, segment},
};

pub const LEGACY_SEGMENTATION_POLICY: &str = "legacy-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateCueSet {
    pub schema_version: u32,
    pub segmentation_policy: String,
    pub cues: Vec<CandidateCue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateCue {
    #[serde(flatten)]
    pub cue: TranscriptSegment,
    pub timed_unit_ids: Vec<String>,
}

impl CandidateCueSet {
    pub fn transcript_segments(&self) -> Vec<TranscriptSegment> {
        self.cues.iter().map(|item| item.cue.clone()).collect()
    }

    pub fn replace_transcript_segments(&mut self, segments: Vec<TranscriptSegment>) -> Result<()> {
        if segments.len() != self.cues.len() {
            return Err(anyhow!(
                "cue transformation changed the segmentation structure"
            ));
        }
        for (candidate, segment) in self.cues.iter_mut().zip(segments) {
            candidate.cue = segment;
        }
        Ok(())
    }
}

/// Produces human-facing cue candidates from provider evidence.
///
/// Provider-segment timing preserves the existing `segment::refine` behavior.
/// Word/token evidence is grouped before the legacy refiner can split a cue by
/// character ratio, so real unit boundaries remain the only timing source.
pub fn segment_timed_units(
    units: &[TimedUnit],
    source_language: LanguageCode,
) -> Result<CandidateCueSet> {
    let evidence = units
        .iter()
        .filter(|unit| !unit.text.trim().is_empty())
        .map(|unit| {
            let start_ms = unit
                .start_ms
                .ok_or_else(|| anyhow!("timed unit {} has no start time", unit.id))?;
            let end_ms = unit
                .end_ms
                .ok_or_else(|| anyhow!("timed unit {} has no end time", unit.id))?;
            if end_ms <= start_ms {
                return Err(anyhow!("timed unit {} has an invalid time range", unit.id));
            }
            Ok((
                unit,
                TranscriptSegment::new(start_ms, end_ms, unit.text.clone()),
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    let refined = if evidence
        .iter()
        .all(|(unit, _)| unit.kind == TimedUnitKind::ProviderSegment)
    {
        segment::refine(
            evidence
                .iter()
                .map(|(_, segment)| segment.clone())
                .collect(),
            source_language,
        )
    } else {
        segment_precise_units(
            evidence
                .iter()
                .map(|(_, segment)| segment.clone())
                .collect(),
            source_language,
        )
    };

    let cues = refined
        .into_iter()
        .map(|cue| CandidateCue {
            timed_unit_ids: evidence
                .iter()
                .filter(|(_, unit)| unit.start_ms < cue.end_ms && unit.end_ms > cue.start_ms)
                .map(|(unit, _)| unit.id.clone())
                .collect(),
            cue,
        })
        .collect();
    Ok(CandidateCueSet {
        schema_version: 1,
        segmentation_policy: LEGACY_SEGMENTATION_POLICY.to_string(),
        cues,
    })
}

fn segment_precise_units(
    units: Vec<TranscriptSegment>,
    source_language: LanguageCode,
) -> Vec<TranscriptSegment> {
    const MAX_DURATION_MS: u64 = 6_000;
    const MIN_DURATION_MS: u64 = 900;
    const PAUSE_SPLIT_MS: u64 = 750;
    let max_chars = if source_language == LanguageCode::English {
        72
    } else {
        44
    };
    let mut output = Vec::new();
    let mut current: Option<TranscriptSegment> = None;
    for unit in units {
        let Some(mut cue) = current.take() else {
            current = Some(normalize_unit(unit, source_language));
            continue;
        };
        let unit = normalize_unit(unit, source_language);
        let pause = unit.start_ms.saturating_sub(cue.end_ms);
        let merged_text = merge_text(&cue.source_text, &unit.source_text, source_language);
        let should_flush = (ends_sentence(&cue.source_text)
            && cue.end_ms.saturating_sub(cue.start_ms) >= MIN_DURATION_MS)
            || pause >= PAUSE_SPLIT_MS
            || unit.end_ms.saturating_sub(cue.start_ms) > MAX_DURATION_MS
            || merged_text.chars().count() > max_chars;
        if should_flush {
            output.push(cue);
            current = Some(unit);
        } else {
            cue.end_ms = unit.end_ms;
            cue.source_text = merged_text;
            current = Some(cue);
        }
    }
    if let Some(cue) = current {
        output.push(cue);
    }
    output
}

fn normalize_unit(mut unit: TranscriptSegment, language: LanguageCode) -> TranscriptSegment {
    let separator = if language == LanguageCode::English {
        " "
    } else {
        ""
    };
    unit.source_text = unit
        .source_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(separator);
    unit
}

fn merge_text(left: &str, right: &str, language: LanguageCode) -> String {
    if language == LanguageCode::English {
        format!("{} {}", left.trim(), right.trim())
    } else {
        format!("{left}{right}")
    }
}

fn ends_sentence(text: &str) -> bool {
    text.chars()
        .last()
        .is_some_and(|ch| matches!(ch, '。' | '！' | '？' | '.' | '!' | '?'))
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{TimedUnit, TimedUnitKind, TimingSource},
        domain::LanguageCode,
    };

    use super::segment_timed_units;

    fn word(id: &str, text: &str, start_ms: u64, end_ms: u64) -> TimedUnit {
        TimedUnit {
            id: id.into(),
            text: text.into(),
            kind: TimedUnitKind::Word,
            start_ms: Some(start_ms),
            end_ms: Some(end_ms),
            timing_source: TimingSource::Model,
            provider_confidence: None,
            speaker: None,
        }
    }

    #[test]
    fn provider_segments_preserve_legacy_text_and_timing() {
        let mut first = word("first", "  The   night  ", 0, 700);
        first.kind = TimedUnitKind::ProviderSegment;
        let mut second = word("second", "is quiet.", 700, 1_400);
        second.kind = TimedUnitKind::ProviderSegment;
        let result = segment_timed_units(&[first, second], LanguageCode::English).unwrap();
        assert_eq!(result.cues.len(), 1);
        assert_eq!(result.cues[0].cue.start_ms, 0);
        assert_eq!(result.cues[0].cue.end_ms, 1_400);
        assert_eq!(result.cues[0].cue.source_text, "The night is quiet.");
        assert_eq!(result.cues[0].timed_unit_ids, vec!["first", "second"]);
    }

    #[test]
    fn word_evidence_uses_real_boundaries_and_records_sources() {
        let units = vec![
            word("one", "one", 0, 400),
            word("two", "two.", 400, 1_000),
            word("three", "three", 2_000, 2_500),
        ];
        let result = segment_timed_units(&units, LanguageCode::English).unwrap();
        assert_eq!(result.cues.len(), 2);
        assert_eq!(result.cues[0].cue.source_text, "one two.");
        assert_eq!(result.cues[0].cue.end_ms, 1_000);
        assert_eq!(result.cues[0].timed_unit_ids, vec!["one", "two"]);
        assert_eq!(result.cues[1].cue.start_ms, 2_000);
    }

    #[test]
    fn refuses_to_invent_missing_word_timing() {
        let mut unit = word("unknown", "word", 0, 100);
        unit.start_ms = None;
        assert!(segment_timed_units(&[unit], LanguageCode::English).is_err());
    }
}
