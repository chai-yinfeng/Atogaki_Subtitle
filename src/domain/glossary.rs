use std::{collections::HashSet, fs, path::Path};

use anyhow::{Context, Result, anyhow};

use crate::{application::TranscriptionOptions, domain::TranscriptSegment};

// whisper.cpp only keeps a limited initial-prompt context. Japanese text is
// close enough to one token per character that a conservative character cap
// prevents large optional packs from crowding out the highest-priority terms.
const MAX_GENERATED_WHISPER_PROMPT_CHARS: usize = 210;
const WHISPER_GLOSSARY_PREFIX: &str = "以下の固有名詞が出る可能性があります: ";
const WHISPER_GLOSSARY_SUFFIX: &str = "。";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Glossary {
    entries: Vec<GlossaryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub source_text: String,
    pub target_text: Option<String>,
    pub include_in_prompt: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlossaryApplyReport {
    pub changed_segments: usize,
    pub cleared_translations: usize,
}

pub fn build_whisper_prompt(options: &TranscriptionOptions) -> Result<Option<String>> {
    let glossary = load_from_options(options)?;
    Ok(glossary.whisper_prompt(options.prompt.as_deref()))
}

pub fn apply_to_segments(
    options: &TranscriptionOptions,
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

impl Glossary {
    pub fn from_entries(entries: impl IntoIterator<Item = GlossaryEntry>) -> Result<Self> {
        let mut entries_out = Vec::new();
        let mut seen = HashSet::new();
        for entry in entries {
            let source = entry.source_text.trim();
            if source.is_empty() {
                return Err(anyhow!("glossary source text cannot be empty"));
            }
            let target = entry
                .target_text
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty());
            if target == Some(source) {
                return Err(anyhow!(
                    "glossary replacement source and target cannot be identical: {source}"
                ));
            }
            if target.is_none() && !entry.include_in_prompt {
                return Err(anyhow!(
                    "a correction-only glossary entry requires a canonical target: {source}"
                ));
            }
            let target = target.map(str::to_string);
            if seen.insert((source.to_string(), target.clone())) {
                entries_out.push(GlossaryEntry {
                    source_text: source.to_string(),
                    target_text: target,
                    include_in_prompt: entry.include_in_prompt,
                });
            } else if entry.include_in_prompt
                && let Some(existing) = entries_out.iter_mut().find(|existing| {
                    existing.source_text == source && existing.target_text == target
                })
            {
                existing.include_in_prompt = true;
            }
        }
        Ok(Self {
            entries: entries_out,
        })
    }

    pub fn to_file_text(&self) -> String {
        let lines = self
            .entries
            .iter()
            .map(|entry| match entry.target_text.as_deref() {
                Some(target) if entry.include_in_prompt => {
                    format!("{} => {target}", entry.source_text)
                }
                Some(target) => format!("@correction-only {} => {target}", entry.source_text),
                None => entry.source_text.clone(),
            })
            .collect::<Vec<_>>();
        if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        }
    }

    pub fn corrected_text(&self, text: &str) -> String {
        let text = remove_legacy_prompt_metadata(text, &self.entries);
        apply_replacements_to_text(&text, &self.replacements())
    }

    pub fn whisper_prompt(&self, initial_prompt: Option<&str>) -> Option<String> {
        let mut parts = initial_prompt
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
            .map(|prompt| vec![prompt.to_string()])
            .unwrap_or_default();
        let glossary_prompt = self.whisper_prompt_terms();
        if !glossary_prompt.is_empty() {
            parts.push(format!(
                "{WHISPER_GLOSSARY_PREFIX}{}{WHISPER_GLOSSARY_SUFFIX}",
                glossary_prompt.join("、")
            ));
        }
        (!parts.is_empty()).then(|| parts.join("\n"))
    }

    pub fn whisper_prompt_term_count(&self) -> usize {
        self.whisper_prompt_terms().len()
    }

    pub fn available_whisper_prompt_term_count(&self) -> usize {
        self.canonical_whisper_prompt_terms().len()
    }

    /// Proper nouns selected for the Whisper prompt are also protected from
    /// machine translation. Include both the heard and canonical spellings so
    /// a correction such as `ナブナ => n-buna` cannot be translated either way.
    pub fn translation_protected_terms(&self) -> Vec<String> {
        let mut terms = self
            .entries
            .iter()
            .filter(|entry| entry.include_in_prompt)
            .flat_map(|entry| {
                std::iter::once(entry.source_text.clone()).chain(entry.target_text.clone())
            })
            .collect::<Vec<_>>();
        terms.sort_by(|left, right| {
            right
                .chars()
                .count()
                .cmp(&left.chars().count())
                .then_with(|| left.cmp(right))
        });
        terms.dedup();
        terms
    }

    fn whisper_prompt_terms(&self) -> Vec<String> {
        let mut character_count =
            WHISPER_GLOSSARY_PREFIX.chars().count() + WHISPER_GLOSSARY_SUFFIX.chars().count();
        let mut included = Vec::new();
        for term in self.canonical_whisper_prompt_terms() {
            let separator_count = usize::from(!included.is_empty());
            let next_count = character_count + separator_count + term.chars().count();
            if next_count > MAX_GENERATED_WHISPER_PROMPT_CHARS {
                continue;
            }
            character_count = next_count;
            included.push(term);
        }
        included
    }

    fn canonical_whisper_prompt_terms(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.entries
            .iter()
            .filter(|entry| entry.include_in_prompt)
            .map(|entry| {
                entry
                    .target_text
                    .clone()
                    .unwrap_or_else(|| entry.source_text.clone())
            })
            .filter(|term| seen.insert(term.clone()))
            .collect()
    }

    fn replacements(&self) -> Vec<(String, String)> {
        let mut replacements = self
            .entries
            .iter()
            .filter_map(|entry| {
                entry
                    .target_text
                    .as_ref()
                    .map(|target| (entry.source_text.clone(), target.clone()))
            })
            .collect::<Vec<_>>();
        replacements.sort_by(|(left, _), (right, _)| {
            right
                .chars()
                .count()
                .cmp(&left.chars().count())
                .then_with(|| left.cmp(right))
        });
        replacements
    }
}

fn apply_glossary_to_segments(
    glossary: &Glossary,
    segments: Vec<TranscriptSegment>,
    clear_changed_translations: bool,
) -> (Vec<TranscriptSegment>, GlossaryApplyReport) {
    let replacements = glossary.replacements();
    if replacements.is_empty() {
        return (segments, GlossaryApplyReport::default());
    }

    let mut report = GlossaryApplyReport::default();
    let segments = segments
        .into_iter()
        .map(|mut segment| {
            let original_ja = segment.source_text.clone();
            let without_prompt_metadata =
                remove_legacy_prompt_metadata(&segment.source_text, &glossary.entries);
            segment.source_text =
                apply_replacements_to_text(&without_prompt_metadata, &replacements);

            if segment.source_text != original_ja {
                report.changed_segments += 1;
                if segment.translated_text.is_some() {
                    if clear_changed_translations {
                        report.cleared_translations += 1;
                    }
                    segment.mark_translation_stale(clear_changed_translations);
                }
            }

            if !clear_changed_translations && let Some(zh) = segment.translated_text.as_mut() {
                *zh = apply_replacements_to_text(zh, &replacements);
            }
            segment
        })
        .collect();

    (segments, report)
}

fn remove_legacy_prompt_metadata(text: &str, entries: &[GlossaryEntry]) -> String {
    entries
        .iter()
        .filter_map(|entry| {
            entry.target_text.as_ref().map(|target| {
                [
                    format!("{}（表記:", entry.source_text),
                    format!("{target}（表記:"),
                ]
            })
        })
        .flatten()
        .fold(text.to_string(), |text, marker| text.replace(&marker, ""))
}

fn load_from_options(options: &TranscriptionOptions) -> Result<Glossary> {
    match options.glossary.as_deref() {
        Some(path) => load(path),
        None => Ok(Glossary::default()),
    }
}

pub fn load(path: &Path) -> Result<Glossary> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut entries = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (line, include_in_prompt) = line
            .strip_prefix("@correction-only ")
            .map(|line| (line.trim(), false))
            .unwrap_or((line, true));
        if let Some((from, to)) = parse_replacement(line) {
            entries.push(GlossaryEntry {
                source_text: from.to_string(),
                target_text: Some(to.to_string()),
                include_in_prompt,
            });
        } else {
            entries.push(GlossaryEntry {
                source_text: line.to_string(),
                target_text: None,
                include_in_prompt: true,
            });
        }
    }

    Glossary::from_entries(entries)
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
    use std::fs;

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

    #[test]
    fn structured_entries_round_trip_to_the_file_format() {
        let glossary = Glossary::from_entries([
            GlossaryEntry {
                source_text: "ヨルシカ".to_string(),
                target_text: None,
                include_in_prompt: true,
            },
            GlossaryEntry {
                source_text: "ナブナ".to_string(),
                target_text: Some("n-buna".to_string()),
                include_in_prompt: true,
            },
            GlossaryEntry {
                source_text: "水井".to_string(),
                target_text: Some("suis".to_string()),
                include_in_prompt: false,
            },
        ])
        .unwrap();

        let text = glossary.to_file_text();
        assert!(text.contains("ヨルシカ"));
        assert!(text.contains("ナブナ => n-buna"));
        assert!(text.contains("@correction-only 水井 => suis"));
        assert!(!text.lines().any(|line| line == "n-buna"));
        assert!(
            glossary
                .whisper_prompt_terms()
                .contains(&"n-buna".to_string())
        );
        assert!(
            !glossary
                .whisper_prompt_terms()
                .iter()
                .any(|term| term.contains("水井"))
        );
        assert_eq!(
            glossary.corrected_text("ナブナとヨルシカ"),
            "n-bunaとヨルシカ"
        );
    }

    #[test]
    fn correction_rules_prompt_whisper_with_canonical_spelling_only() {
        let path = std::env::temp_dir().join(format!(
            "atogaki-glossary-prompt-test-{}.txt",
            uuid::Uuid::new_v4()
        ));
        fs::write(&path, "スイ => suis\n").unwrap();
        let mut options = TranscriptionOptions::japanese("model.bin".into());
        options.glossary = Some(path.clone());

        let prompt = build_whisper_prompt(&options).unwrap().unwrap();
        assert!(prompt.contains("suis"));
        assert!(!prompt.contains("スイ"));
        assert!(!prompt.contains("表記:"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn generated_prompt_is_bounded_and_keeps_whole_terms() {
        let glossary = Glossary::from_entries((0..100).map(|index| GlossaryEntry {
            source_text: format!("固有名詞{index}"),
            target_text: None,
            include_in_prompt: true,
        }))
        .unwrap();

        let prompt = glossary.whisper_prompt(None).unwrap();
        assert!(prompt.chars().count() <= MAX_GENERATED_WHISPER_PROMPT_CHARS);
        assert!(glossary.whisper_prompt_term_count() < 100);
        assert!(prompt.ends_with('。'));
    }

    #[test]
    fn removes_legacy_prompt_metadata_leakage_before_correction() {
        let glossary = Glossary::from_entries([GlossaryEntry {
            source_text: "やまおかゆり".to_string(),
            target_text: Some("山岡ゆり".to_string()),
            include_in_prompt: true,
        }])
        .unwrap();

        assert_eq!(
            glossary.corrected_text("やまおかゆり（表記: 今日は最終回です"),
            " 今日は最終回です"
        );
    }

    #[test]
    fn prompt_terms_are_also_translation_protected() {
        let glossary = Glossary::from_entries([
            GlossaryEntry {
                source_text: "盗作".to_string(),
                target_text: None,
                include_in_prompt: true,
            },
            GlossaryEntry {
                source_text: "ナブナ".to_string(),
                target_text: Some("n-buna".to_string()),
                include_in_prompt: true,
            },
            GlossaryEntry {
                source_text: "水井".to_string(),
                target_text: Some("suis".to_string()),
                include_in_prompt: false,
            },
        ])
        .unwrap();

        assert_eq!(
            glossary.translation_protected_terms(),
            vec!["n-buna", "ナブナ", "盗作"]
        );
    }
}
