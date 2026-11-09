//! Read-only research preview using the production glossary engine.
//! Input variants contain already-resolved entries; this does not select task
//! content groups, read a database, apply changes to a job, or call a model.
use std::{collections::HashSet, io};

use anyhow::{Result, ensure};
use atogaki_subtitle::domain::glossary::{Glossary, GlossaryEntry};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Input {
    variants: Vec<Variant>,
    sources: Vec<Source>,
}

#[derive(Deserialize)]
struct Variant {
    name: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    source_text: String,
    target_text: Option<String>,
    include_in_prompt: bool,
}

#[derive(Deserialize)]
struct Source {
    id: String,
    text: String,
}

fn main() -> Result<()> {
    let input: Input = serde_json::from_reader(io::stdin().lock())?;
    ensure!(
        input
            .sources
            .iter()
            .map(|source| &source.id)
            .collect::<HashSet<_>>()
            .len()
            == input.sources.len(),
        "duplicate source IDs"
    );
    ensure!(
        input
            .variants
            .iter()
            .map(|variant| &variant.name)
            .collect::<HashSet<_>>()
            .len()
            == input.variants.len(),
        "duplicate variant names"
    );
    let mut variants = Vec::new();
    for variant in input.variants {
        let glossary =
            Glossary::from_entries(variant.entries.into_iter().map(|entry| GlossaryEntry {
                source_text: entry.source_text,
                target_text: entry.target_text,
                include_in_prompt: entry.include_in_prompt,
            }))?;
        let sources = input
            .sources
            .iter()
            .map(|source| {
                json!({
                    "id": source.id,
                    "original_text": source.text,
                    "corrected_text": glossary.corrected_text(&source.text),
                })
            })
            .collect::<Vec<_>>();
        variants.push(json!({
            "name": variant.name,
            "whisper_prompt": glossary.whisper_prompt(None),
            "protected_terms": glossary.translation_protected_terms(),
            "sources": sources,
        }));
    }
    serde_json::to_writer_pretty(io::stdout().lock(), &json!({"variants": variants}))?;
    println!();
    Ok(())
}
