use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::{cli::WhisperArgs, segment::TranscriptSegment};

#[derive(Debug, Clone, Default)]
struct Glossary {
    terms: Vec<String>,
    replacements: Vec<(String, String)>,
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
    if glossary.replacements.is_empty() {
        return Ok(segments);
    }

    Ok(segments
        .into_iter()
        .map(|mut segment| {
            for (from, to) in &glossary.replacements {
                segment.ja_text = segment.ja_text.replace(from, to);
                if let Some(zh) = segment.zh_text.as_mut() {
                    *zh = zh.replace(from, to);
                }
            }
            segment
        })
        .collect())
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
