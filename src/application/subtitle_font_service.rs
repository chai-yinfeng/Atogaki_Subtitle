use std::{collections::BTreeMap, sync::Arc};

use fontdb::{Database, Family, Query, Stretch, Style, Weight};
use serde::Serialize;

use crate::domain::subtitle::{SubtitleStyle, SubtitleStyleSet};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SubtitleFontFamily {
    pub family: String,
    pub aliases: Vec<String>,
    pub face_count: usize,
    pub monospaced: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SubtitleFontCoverage {
    pub requested_family: String,
    pub installed: bool,
    pub matched_family: Option<String>,
    pub post_script_name: Option<String>,
    pub missing_characters: Vec<String>,
    pub needs_fallback: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SubtitleFontReport {
    pub source: SubtitleFontCoverage,
    pub translation: SubtitleFontCoverage,
}

#[derive(Clone)]
pub struct SubtitleFontService {
    database: Arc<Database>,
    families: Arc<Vec<SubtitleFontFamily>>,
}

impl SubtitleFontService {
    pub fn load_system() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        let mut grouped = BTreeMap::<String, (Vec<String>, usize, bool)>::new();
        for face in database.faces() {
            let Some((primary, _)) = face.families.first() else {
                continue;
            };
            if !is_user_facing_font_family(primary) {
                continue;
            }
            let entry = grouped
                .entry(primary.clone())
                .or_insert_with(|| (Vec::new(), 0, true));
            entry.1 += 1;
            entry.2 &= face.monospaced;
            for (alias, _) in &face.families {
                if alias != primary && is_user_facing_font_family(alias) && !entry.0.contains(alias)
                {
                    entry.0.push(alias.clone());
                }
            }
        }
        let families = grouped
            .into_iter()
            .map(
                |(family, (mut aliases, face_count, monospaced))| SubtitleFontFamily {
                    family,
                    aliases: {
                        aliases.sort_by_key(|alias| alias.to_lowercase());
                        aliases
                    },
                    face_count,
                    monospaced,
                },
            )
            .collect();
        Self {
            database: Arc::new(database),
            families: Arc::new(families),
        }
    }

    pub fn families(&self) -> &[SubtitleFontFamily] {
        &self.families
    }

    pub fn check_styles(
        &self,
        styles: &SubtitleStyleSet,
        source_text: &str,
        translation_text: &str,
    ) -> SubtitleFontReport {
        SubtitleFontReport {
            source: self.check_style(&styles.source, source_text),
            translation: self.check_style(&styles.translation, translation_text),
        }
    }

    fn check_style(&self, style: &SubtitleStyle, text: &str) -> SubtitleFontCoverage {
        let requested = style.font_family.trim();
        let matched_name = self
            .families
            .iter()
            .find(|family| {
                family.family.eq_ignore_ascii_case(requested)
                    || family
                        .aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(requested))
            })
            .map(|family| family.family.clone())
            .or_else(|| {
                self.database.faces().find_map(|face| {
                    face.families
                        .iter()
                        .find(|(family, _)| family.eq_ignore_ascii_case(requested))
                        .map(|(family, _)| family.clone())
                })
            });
        let Some(matched_name) = matched_name else {
            return SubtitleFontCoverage {
                requested_family: requested.to_string(),
                installed: false,
                matched_family: None,
                post_script_name: None,
                missing_characters: unique_visible_characters(text, 32),
                needs_fallback: !text.trim().is_empty(),
            };
        };
        let families = [Family::Name(&matched_name)];
        let query = Query {
            families: &families,
            weight: if style.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            },
            stretch: Stretch::Normal,
            style: if style.italic {
                Style::Italic
            } else {
                Style::Normal
            },
        };
        let face_id = self.database.query(&query).or_else(|| {
            self.database
                .faces()
                .find(|face| {
                    face.families
                        .iter()
                        .any(|(family, _)| family == &matched_name)
                })
                .map(|face| face.id)
        });
        let post_script_name = face_id
            .and_then(|id| self.database.face(id))
            .map(|face| face.post_script_name.clone());
        let missing_characters = face_id
            .and_then(|id| {
                self.database.with_face_data(id, |data, index| {
                    ttf_parser::Face::parse(data, index).ok().map(|face| {
                        unique_visible_characters(text, usize::MAX)
                            .into_iter()
                            .filter(|character| {
                                character
                                    .chars()
                                    .next()
                                    .is_some_and(|character| face.glyph_index(character).is_none())
                            })
                            .take(32)
                            .collect::<Vec<_>>()
                    })
                })
            })
            .flatten()
            .unwrap_or_else(|| unique_visible_characters(text, 32));
        SubtitleFontCoverage {
            requested_family: requested.to_string(),
            installed: true,
            matched_family: Some(matched_name),
            post_script_name,
            needs_fallback: !missing_characters.is_empty(),
            missing_characters,
        }
    }
}

fn is_user_facing_font_family(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && !name.starts_with('.')
}

fn unique_visible_characters(text: &str, limit: usize) -> Vec<String> {
    let mut characters = Vec::new();
    for character in text.chars() {
        if character.is_control() || character.is_whitespace() {
            continue;
        }
        let value = character.to_string();
        if !characters.contains(&value) {
            characters.push(value);
            if characters.len() == limit {
                break;
            }
        }
    }
    characters
}

#[cfg(test)]
mod tests {
    use super::{is_user_facing_font_family, unique_visible_characters};

    #[test]
    fn coverage_samples_deduplicate_and_ignore_layout_characters() {
        assert_eq!(unique_visible_characters("a a\n学a", 32), ["a", "学"]);
    }

    #[test]
    fn internal_dot_prefixed_font_families_are_not_user_choices() {
        assert!(!is_user_facing_font_family(".AppleSystemUIFont"));
        assert!(!is_user_facing_font_family("  .SF NS  "));
        assert!(is_user_facing_font_family("Hiragino Sans"));
    }
}
