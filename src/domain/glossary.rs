use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::{domain::TranscriptSegment, interface::cli::WhisperArgs};

#[derive(Debug, Clone, Default)]
struct Glossary {
    terms: Vec<String>,
    replacements: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlossaryApplyReport {
    pub changed_segments: usize,
    pub cleared_translations: usize,
}

pub fn build_whisper_prompt(options: &WhisperArgs) -> Result<Option<String>> {
    let glossary = load_from_options(options)?;
    let mut parts = Vec::new();

    if let Some(prompt) = options
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(prompt.to_string());
    }

    if !glossary.terms.is_empty() {
        parts.push(format!(
            "以下の固有名詞が出る可能性があります: {}。",
            glossary.terms.join("、")
        ));
    }

    Ok((!parts.is_empty()).then(|| parts.join("\n")))
}

pub fn apply_to_segments(
    options: &WhisperArgs,
    segments: Vec<TranscriptSegment>,
) -> Result<Vec<TranscriptSegment>> {
    let glossary = load_from_options(options)?;
    let (segments, _) = apply_glossary_to_segments(&glossary, segments, false);
    Ok(segments)
}

pub fn apply_file_to_segments(
    path: &Path,
    segments: Vec<TranscriptSegment>,
    clear_changed_translations: bool,
) -> Result<(Vec<TranscriptSegment>, GlossaryApplyReport)> {
    let glossary = load(path)?;
    Ok(apply_glossary_to_segments(
        &glossary,
        segments,
        clear_changed_translations,
    ))
}

fn apply_glossary_to_segments(
    glossary: &Glossary,
    segments: Vec<TranscriptSegment>,
    clear_changed_translations: bool,
) -> (Vec<TranscriptSegment>, GlossaryApplyReport) {
    if glossary.replacements.is_empty() {
        return (segments, GlossaryApplyReport::default());
    }

    let mut report = GlossaryApplyReport::default();
    let segments = segments
        .into_iter()
        .map(|mut segment| {
            let original_ja = segment.ja_text.clone();
            segment.ja_text = apply_replacements_to_text(&segment.ja_text, &glossary.replacements);

            if segment.ja_text != original_ja {
                report.changed_segments += 1;
                if clear_changed_translations && segment.zh_text.take().is_some() {
                    report.cleared_translations += 1;
                }
            }

            if !clear_changed_translations && let Some(zh) = segment.zh_text.as_mut() {
                *zh = apply_replacements_to_text(zh, &glossary.replacements);
            }
            segment
        })
        .collect();

    (segments, report)
}

fn load_from_options(options: &WhisperArgs) -> Result<Glossary> {
    match options.glossary.as_deref() {
        Some(path) => load(path),
        None => Ok(Glossary::default()),
    }
}

fn load(path: &Path) -> Result<Glossary> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut glossary = Glossary::default();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((from, to)) = parse_replacement(line) {
            glossary
                .replacements
                .push((from.to_string(), to.to_string()));
            glossary.terms.push(to.to_string());
        } else {
            glossary.terms.push(line.to_string());
        }
    }

    glossary.terms.sort();
    glossary.terms.dedup();
    Ok(glossary)
}

fn parse_replacement(line: &str) -> Option<(&str, &str)> {
    line.split_once("=>")
        .or_else(|| line.split_once('\t'))
        .map(|(from, to)| (from.trim(), to.trim()))
        .filter(|(from, to)| !from.is_empty() && !to.is_empty())
}

fn apply_replacements_to_text(text: &str, replacements: &[(String, String)]) -> String {
    replacements
        .iter()
        .fold(text.to_string(), |text, (from, to)| {
            replace_glossary_term(&text, from, to)
        })
}

fn replace_glossary_term(text: &str, from: &str, to: &str) -> String {
    if should_require_katakana_boundaries(from) {
        replace_with_katakana_boundaries(text, from, to)
    } else {
        text.replace(from, to)
    }
}

fn should_require_katakana_boundaries(term: &str) -> bool {
    term.chars().all(is_katakana_letter)
}

fn replace_with_katakana_boundaries(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;

    for (start, _) in text.match_indices(from) {
        let end = start + from.len();
        let has_katakana_before = text[..start]
            .chars()
            .next_back()
            .is_some_and(is_katakana_letter);
        let has_katakana_after = text[end..].chars().next().is_some_and(is_katakana_letter);

        if has_katakana_before || has_katakana_after {
            continue;
        }

        out.push_str(&text[last..start]);
        out.push_str(to);
        last = end;
    }

    if last == 0 {
        return text.to_string();
    }

    out.push_str(&text[last..]);
    out
}

fn is_katakana_letter(ch: char) -> bool {
    matches!(
        ch,
        '\u{30A1}'..='\u{30FA}' | '\u{30FC}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9D}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_asr_replacements_with_katakana_boundaries() {
        let replacements = vec![
            ("スイ".to_string(), "suis".to_string()),
            ("ナブナ".to_string(), "n-buna".to_string()),
        ];

        let text = "スイさんとナブナさんがスイッチの話をした。";

        assert_eq!(
            apply_replacements_to_text(text, &replacements),
            "suisさんとn-bunaさんがスイッチの話をした。"
        );
    }

    #[test]
    fn leaves_katakana_compounds_unchanged() {
        let replacements = vec![("スイ".to_string(), "suis".to_string())];

        assert_eq!(
            apply_replacements_to_text("アイスイッチとスイッチ", &replacements),
            "アイスイッチとスイッチ"
        );
    }
}
