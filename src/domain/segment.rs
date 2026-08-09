use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    #[serde(default)]
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    #[serde(alias = "ja_text")]
    pub source_text: String,
    #[serde(default, alias = "zh_text")]
    pub translated_text: Option<String>,
    #[serde(default)]
    pub source_edited: bool,
    #[serde(default)]
    pub translation_stale: bool,
}

impl TranscriptSegment {
    pub fn new(start_ms: u64, end_ms: u64, source_text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            start_ms,
            end_ms,
            source_text,
            translated_text: None,
            source_edited: false,
            translation_stale: false,
        }
    }

    /// Applies a manual source-text edit and preserves any previous translation
    /// for comparison while marking it as no longer trustworthy.
    pub fn set_source_text(&mut self, source_text: String) -> bool {
        if self.source_text == source_text {
            return false;
        }

        self.source_text = source_text;
        self.source_edited = true;
        self.translation_stale = self.translated_text.is_some();
        true
    }

    pub fn set_translation(&mut self, translated_text: Option<String>) {
        self.translated_text = translated_text;
        self.translation_stale = false;
    }

    pub fn mark_translation_stale(&mut self, clear_translation: bool) {
        if clear_translation {
            self.translated_text = None;
        }
        self.translation_stale = self.translated_text.is_some() || clear_translation;
    }

    /// Returns true when a legacy segment without an ID was upgraded.
    pub fn ensure_id(&mut self) -> bool {
        if !self.id.is_empty() {
            return false;
        }

        self.id = Uuid::new_v4().to_string();
        true
    }
}

pub fn refine(
    raw: Vec<TranscriptSegment>,
    source_language: crate::domain::LanguageCode,
) -> Vec<TranscriptSegment> {
    const MAX_DURATION_MS: u64 = 6_000;
    let max_chars = match source_language {
        crate::domain::LanguageCode::English => 72,
        _ => 44,
    };
    const MIN_DURATION_MS: u64 = 900;
    const PAUSE_SPLIT_MS: u64 = 750;

    let normalized = raw
        .into_iter()
        .filter_map(|seg| {
            let text = normalize_text(&seg.source_text, source_language);
            (!text.is_empty()).then_some(TranscriptSegment {
                source_text: text,
                ..seg
            })
        })
        .collect();
    let raw = drop_covered_timing_artifacts(coalesce_equal_ranges(normalized, source_language));

    let mut out = Vec::new();
    let mut current: Option<TranscriptSegment> = None;

    for seg in raw {
        match current.take() {
            None => current = Some(seg),
            Some(mut cur) => {
                let pause = seg.start_ms.saturating_sub(cur.end_ms);
                let merged_chars = char_count(&cur.source_text) + char_count(&seg.source_text);
                let merged_duration = seg.end_ms.saturating_sub(cur.start_ms);
                let cur_duration = cur.end_ms.saturating_sub(cur.start_ms);
                let should_flush = ends_sentence(&cur.source_text)
                    && cur_duration >= MIN_DURATION_MS
                    || pause >= PAUSE_SPLIT_MS
                    || merged_duration > MAX_DURATION_MS
                    || merged_chars > max_chars;

                if should_flush {
                    push_split(&mut out, cur, max_chars);
                    current = Some(seg);
                } else {
                    cur.end_ms = seg.end_ms;
                    cur.source_text =
                        merge_text(&cur.source_text, &seg.source_text, source_language);
                    current = Some(cur);
                }
            }
        }
    }

    if let Some(cur) = current {
        push_split(&mut out, cur, max_chars);
    }

    out
}

fn coalesce_equal_ranges(
    raw: Vec<TranscriptSegment>,
    source_language: crate::domain::LanguageCode,
) -> Vec<TranscriptSegment> {
    let mut out: Vec<TranscriptSegment> = Vec::with_capacity(raw.len());

    for seg in raw {
        if let Some(previous) = out.last_mut()
            && previous.start_ms == seg.start_ms
            && previous.end_ms == seg.end_ms
        {
            previous.source_text =
                merge_text(&previous.source_text, &seg.source_text, source_language);
        } else {
            out.push(seg);
        }
    }

    out
}

fn drop_covered_timing_artifacts(raw: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    const MAX_ARTIFACT_DURATION_MS: u64 = 250;
    const MIN_IMPLAUSIBLE_CHARS: usize = 24;

    raw.iter()
        .enumerate()
        .filter(|(index, seg)| {
            let duration = seg.end_ms.saturating_sub(seg.start_ms);
            let covered_by_next = raw
                .get(index + 1)
                .is_some_and(|next| next.start_ms <= seg.start_ms && next.end_ms > seg.end_ms);

            !(duration <= MAX_ARTIFACT_DURATION_MS
                && char_count(&seg.source_text) >= MIN_IMPLAUSIBLE_CHARS
                && covered_by_next)
        })
        .map(|(_, seg)| seg.clone())
        .collect()
}

fn push_split(out: &mut Vec<TranscriptSegment>, seg: TranscriptSegment, max_chars: usize) {
    if char_count(&seg.source_text) <= max_chars {
        out.push(seg);
        return;
    }

    let parts = split_long_text(&seg.source_text, max_chars);
    if parts.len() <= 1 {
        out.push(seg);
        return;
    }

    let duration = seg.end_ms.saturating_sub(seg.start_ms).max(1);
    let total_chars: usize = parts.iter().map(|s| char_count(s)).sum();
    let mut start = seg.start_ms;

    for (idx, part) in parts.iter().enumerate() {
        let end = if idx == parts.len() - 1 {
            seg.end_ms
        } else {
            start + duration * char_count(part) as u64 / total_chars.max(1) as u64
        };
        out.push(TranscriptSegment::new(start, end, part.clone()));
        start = end;
    }
}

fn split_long_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();

    for ch in text.chars() {
        buf.push(ch);
        if sentence_punctuation(ch) || char_count(&buf) >= max_chars {
            parts.push(buf.trim().to_string());
            buf.clear();
        }
    }

    if !buf.trim().is_empty() {
        parts.push(buf.trim().to_string());
    }

    parts
}

fn normalize_text(text: &str, source_language: crate::domain::LanguageCode) -> String {
    let separator = match source_language {
        crate::domain::LanguageCode::English => " ",
        _ => "",
    };
    text.split_whitespace().collect::<Vec<_>>().join(separator)
}

fn merge_text(left: &str, right: &str, source_language: crate::domain::LanguageCode) -> String {
    match source_language {
        crate::domain::LanguageCode::English => format!("{} {}", left.trim(), right.trim()),
        _ => format!("{left}{right}"),
    }
}

fn ends_sentence(text: &str) -> bool {
    text.chars().last().is_some_and(sentence_punctuation)
}

fn sentence_punctuation(ch: char) -> bool {
    matches!(ch, '。' | '！' | '？' | '.' | '!' | '?')
}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use crate::domain::LanguageCode;

    use super::{TranscriptSegment, refine};

    #[test]
    fn source_edits_mark_existing_translations_stale() {
        let mut segment = TranscriptSegment::new(0, 1_000, "元の文".to_string());
        segment.set_translation(Some("原来的句子".to_string()));

        assert!(segment.set_source_text("修正後の文".to_string()));
        assert_eq!(segment.translated_text.as_deref(), Some("原来的句子"));
        assert!(segment.source_edited);
        assert!(segment.translation_stale);

        segment.set_translation(Some("修正后的句子".to_string()));
        assert!(!segment.translation_stale);
    }

    #[test]
    fn missing_legacy_id_is_repaired_once() {
        let mut segment = TranscriptSegment::new(0, 1_000, "文".to_string());
        segment.id.clear();

        assert!(segment.ensure_id());
        let id = segment.id.clone();
        assert!(!id.is_empty());
        assert!(!segment.ensure_id());
        assert_eq!(segment.id, id);
    }

    #[test]
    fn english_refinement_preserves_word_boundaries() {
        let raw = vec![
            TranscriptSegment::new(0, 700, "  The   night  ".to_string()),
            TranscriptSegment::new(700, 1_400, "is quiet.".to_string()),
        ];

        let refined = refine(raw, LanguageCode::English);

        assert_eq!(refined.len(), 1);
        assert_eq!(refined[0].source_text, "The night is quiet.");
    }

    #[test]
    fn covered_whisper_timing_artifacts_are_dropped() {
        let raw = vec![
            TranscriptSegment::new(
                51_330,
                53_280,
                "fewer things but feeling like you have too many things.".to_string(),
            ),
            TranscriptSegment::new(53_280, 53_380, "And that's what minimalism is".to_string()),
            TranscriptSegment::new(53_280, 53_380, "all about, having fewer things".to_string()),
            TranscriptSegment::new(53_280, 53_380, "but".to_string()),
            TranscriptSegment::new(53_280, 55_280, "lighter, freer, and happier.".to_string()),
        ];

        let refined = refine(raw, LanguageCode::English);

        assert_eq!(refined.len(), 2);
        assert_eq!(
            refined[0].source_text,
            "fewer things but feeling like you have too many things."
        );
        assert_eq!(refined[1].source_text, "lighter, freer, and happier.");
        assert_eq!((refined[1].start_ms, refined[1].end_ms), (53_280, 55_280));
    }

    #[test]
    fn japanese_refinement_keeps_compact_text() {
        let raw = vec![
            TranscriptSegment::new(0, 700, "夜 は".to_string()),
            TranscriptSegment::new(700, 1_400, "静か。".to_string()),
        ];

        let refined = refine(raw, LanguageCode::Japanese);

        assert_eq!(refined.len(), 1);
        assert_eq!(refined[0].source_text, "夜は静か。");
    }

    #[test]
    fn legacy_language_specific_json_fields_remain_readable() {
        let segment: TranscriptSegment = serde_json::from_str(
            r#"{"start_ms":0,"end_ms":1000,"ja_text":"こんばんは","zh_text":"晚上好"}"#,
        )
        .unwrap();

        assert_eq!(segment.source_text, "こんばんは");
        assert_eq!(segment.translated_text.as_deref(), Some("晚上好"));
    }
}
