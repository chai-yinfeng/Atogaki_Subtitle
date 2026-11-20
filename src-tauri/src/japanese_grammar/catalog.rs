use std::{collections::HashMap, sync::OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    pub id: String,
    pub pattern: String,
    pub level: String,
    #[serde(default)]
    pub romaji: String,
    #[serde(default)]
    pub reading: String,
    pub meaning: String,
    pub formation: String,
    #[serde(default)]
    pub notes: String,
    pub examples: Vec<Example>,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lesson: String,
    #[serde(skip)]
    localized: bool,
    #[serde(skip)]
    translated_from_openjlpt: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Example {
    pub ja: String,
    #[serde(alias = "en")]
    pub translation: String,
}

#[derive(Deserialize)]
struct OpenJlptTranslation {
    id: String,
    formation: String,
    meaning: String,
    #[serde(default)]
    notes: String,
    examples: Vec<String>,
}

impl OpenJlptTranslation {
    fn apply(self, entry: &mut Entry) {
        assert_eq!(self.examples.len(), entry.examples.len(), "{}", self.id);
        entry.formation = self.formation;
        entry.meaning = self.meaning;
        entry.notes = self.notes;
        for (example, translation) in entry.examples.iter_mut().zip(self.examples) {
            example.translation = translation;
        }
        entry.localized = true;
        entry.translated_from_openjlpt = true;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Summary<'a> {
    pub id: &'a str,
    pub pattern: &'a str,
    pub level: &'a str,
    pub meaning: &'a str,
    pub reading: &'a str,
    pub romaji: &'a str,
    pub language: &'static str,
    pub automatic: bool,
    pub translated_from_openjlpt: bool,
}

#[derive(Serialize)]
pub(crate) struct Detail {
    pub entry: &'static Entry,
    pub summary: Summary<'static>,
    pub related: Vec<Summary<'static>>,
}

impl Entry {
    pub fn summary(&self) -> Summary<'_> {
        Summary {
            id: &self.id,
            pattern: &self.pattern,
            level: &self.level,
            meaning: &self.meaning,
            reading: &self.reading,
            romaji: &self.romaji,
            language: self.language(),
            automatic: super::rules::rule_ids().any(|id| id == self.id),
            translated_from_openjlpt: self.translated_from_openjlpt,
        }
    }

    pub fn language(&self) -> &'static str {
        if self.localized { "zh-Hans" } else { "en" }
    }
}

pub(crate) fn entries() -> &'static [Entry] {
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    DATA.get_or_init(|| {
        const OPENJLPT: &[&str] = &[
            include_str!("../../../assets/japanese-grammar/openjlpt/n5.json"),
            include_str!("../../../assets/japanese-grammar/openjlpt/n4.json"),
            include_str!("../../../assets/japanese-grammar/openjlpt/n3.json"),
            include_str!("../../../assets/japanese-grammar/openjlpt/n2.json"),
            include_str!("../../../assets/japanese-grammar/openjlpt/n1.json"),
        ];
        let mut overlay: HashMap<String, Entry> = serde_json::from_str::<Vec<Entry>>(include_str!(
            "../../../assets/japanese-grammar/zh-Hans.json"
        ))
        .expect("bundled Chinese grammar data must parse")
        .into_iter()
        .map(|mut entry| {
            entry.localized = true;
            (entry.id.clone(), entry)
        })
        .collect();

        let mut translations: HashMap<String, OpenJlptTranslation> =
            serde_json::from_str::<Vec<OpenJlptTranslation>>(include_str!(
                "../../../assets/japanese-grammar/zh-Hans-openjlpt.json"
            ))
            .expect("bundled OpenJLPT translations must parse")
            .into_iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect();

        let mut entries: Vec<_> = OPENJLPT
            .iter()
            .flat_map(|data| {
                serde_json::from_str::<Vec<Entry>>(data).expect("bundled OpenJLPT must parse")
            })
            .map(|mut entry| match overlay.remove(&entry.id) {
                Some(mut zh) => {
                    zh.romaji = entry.romaji;
                    zh.reading = entry.reading;
                    zh
                }
                None => {
                    if let Some(translation) = translations.remove(&entry.id) {
                        translation.apply(&mut entry);
                    }
                    entry
                }
            })
            .collect();
        let mut concepts = overlay.into_values().collect::<Vec<_>>();
        concepts.sort_by(|a, b| {
            (a.id != "grammar-basics", &a.id).cmp(&(b.id != "grammar-basics", &b.id))
        });
        entries.extend(concepts);
        entries
    })
}

pub(crate) fn get(id: &str) -> Option<&'static Entry> {
    entries().iter().find(|entry| entry.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashSet};

    #[test]
    fn catalog_has_unique_levelled_cards_and_resolvable_relationships() {
        let entries = super::entries();
        let mut ids = HashSet::new();
        let mut levels = BTreeMap::new();
        for entry in entries {
            assert!(ids.insert(entry.id.as_str()), "duplicate grammar ID");
            *levels.entry(entry.level.as_str()).or_insert(0) += 1;
            assert!(
                !entry.pattern.is_empty()
                    && !entry.formation.is_empty()
                    && !entry.meaning.is_empty()
            );
            assert!(!entry.examples.is_empty());
            assert!(entry.examples.iter().all(|example| {
                !example.ja.trim().is_empty() && !example.translation.trim().is_empty()
            }));
            if entry.language() == "zh-Hans" {
                assert!(!entry.notes.trim().is_empty(), "{}", entry.id);
            }
        }
        assert_eq!(
            levels,
            BTreeMap::from([
                ("N1", 123),
                ("N2", 123),
                ("N3", 101),
                ("N4", 98),
                ("N5", 81),
                ("基础概念", 21),
            ])
        );
        assert!(entries.iter().all(|entry| entry.language() == "zh-Hans"));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.translated_from_openjlpt)
                .count(),
            384
        );
        for entry in entries {
            assert!(entry.related.iter().all(|id| ids.contains(id.as_str())));
        }
        assert_eq!(super::get("gatera").unwrap().level, "N1");
        assert_eq!(super::get("grammar-basics").unwrap().level, "基础概念");
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.level == "基础概念")
                .unwrap()
                .id,
            "grammar-basics"
        );
        let mut rule_ids = HashSet::new();
        for id in super::super::rules::rule_ids() {
            assert!(rule_ids.insert(id), "duplicate rule ID {id}");
            assert_eq!(super::get(id).unwrap().language(), "zh-Hans");
        }
        assert_eq!(rule_ids.len(), 55);
    }
}
