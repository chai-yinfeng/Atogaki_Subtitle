use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    #[serde(default)]
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub ja_text: String,
    pub zh_text: Option<String>,
    #[serde(default)]
    pub source_edited: bool,
    #[serde(default)]
    pub translation_stale: bool,
}

impl TranscriptSegment {
    pub fn new(start_ms: u64, end_ms: u64, ja_text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            start_ms,
            end_ms,
            ja_text,
            zh_text: None,
            source_edited: false,
            translation_stale: false,
        }
    }

    /// Applies a manual source-text edit and preserves any previous translation
    /// for comparison while marking it as no longer trustworthy.
    pub fn set_source_text(&mut self, ja_text: String) -> bool {
        if self.ja_text == ja_text {
            return false;
        }

        self.ja_text = ja_text;
        self.source_edited = true;
        self.translation_stale = self.zh_text.is_some();
        true
    }

    pub fn set_translation(&mut self, zh_text: Option<String>) {
        self.zh_text = zh_text;
        self.translation_stale = false;
    }

    pub fn mark_translation_stale(&mut self, clear_translation: bool) {
        if clear_translation {
            self.zh_text = None;
        }
        self.translation_stale = self.zh_text.is_some() || clear_translation;
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

pub fn refine(raw: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    const MAX_DURATION_MS: u64 = 6_000;
    const MAX_CHARS: usize = 44;
    const MIN_DURATION_MS: u64 = 900;
    const PAUSE_SPLIT_MS: u64 = 750;

    let mut out = Vec::new();
    let mut current: Option<TranscriptSegment> = None;

    for seg in raw {
        let text = normalize_text(&seg.ja_text);
        if text.is_empty() {
            continue;
        }

        let seg = TranscriptSegment {
            ja_text: text,
            ..seg
        };

        match current.take() {
            None => current = Some(seg),
            Some(mut cur) => {
                let pause = seg.start_ms.saturating_sub(cur.end_ms);
                let merged_chars = char_count(&cur.ja_text) + char_count(&seg.ja_text);
                let merged_duration = seg.end_ms.saturating_sub(cur.start_ms);
                let cur_duration = cur.end_ms.saturating_sub(cur.start_ms);
                let should_flush = ends_sentence(&cur.ja_text) && cur_duration >= MIN_DURATION_MS
                    || pause >= PAUSE_SPLIT_MS
                    || merged_duration > MAX_DURATION_MS
                    || merged_chars > MAX_CHARS;

                if should_flush {
                    push_split(&mut out, cur, MAX_CHARS);
                    current = Some(seg);
                } else {
                    cur.end_ms = seg.end_ms;
                    cur.ja_text = merge_text(&cur.ja_text, &seg.ja_text);
                    current = Some(cur);
                }
            }
        }
    }

    if let Some(cur) = current {
        push_split(&mut out, cur, MAX_CHARS);
    }

    out
}

fn push_split(out: &mut Vec<TranscriptSegment>, seg: TranscriptSegment, max_chars: usize) {
    if char_count(&seg.ja_text) <= max_chars {
        out.push(seg);
        return;
    }

    let parts = split_long_text(&seg.ja_text, max_chars);
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

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join("")
}

fn merge_text(left: &str, right: &str) -> String {
    format!("{left}{right}")
}

fn ends_sentence(text: &str) -> bool {
    text.chars().last().is_some_and(sentence_punctuation)
}

fn sentence_punctuation(ch: char) -> bool {
    matches!(ch, '。' | '！' | '？' | '!' | '?')
}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::TranscriptSegment;

    #[test]
    fn source_edits_mark_existing_translations_stale() {
        let mut segment = TranscriptSegment::new(0, 1_000, "元の文".to_string());
        segment.set_translation(Some("原来的句子".to_string()));

        assert!(segment.set_source_text("修正後の文".to_string()));
        assert_eq!(segment.zh_text.as_deref(), Some("原来的句子"));
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
}
