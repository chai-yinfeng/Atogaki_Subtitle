use crate::domain::{LanguageCode, LanguagePair};

/// Options for a translation run, independent from a particular user interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationOptions {
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
    /// Terms that must remain verbatim in machine-translation output.
    ///
    /// These come from the immutable glossary snapshot selected for a task,
    /// rather than from mutable global glossary state.
    pub protected_terms: Vec<String>,
}

impl TranslationOptions {
    pub fn new(source_language: LanguageCode, target_language: LanguageCode) -> Self {
        Self {
            source_language,
            target_language,
            protected_terms: Vec::new(),
        }
    }

    pub fn from_pair(pair: LanguagePair) -> Self {
        Self::new(pair.source, pair.target)
    }

    pub fn pair(&self) -> LanguagePair {
        LanguagePair {
            source: self.source_language,
            target: self.target_language,
        }
    }

    pub fn with_protected_terms(
        mut self,
        protected_terms: impl IntoIterator<Item = String>,
    ) -> Self {
        self.protected_terms = protected_terms
            .into_iter()
            .map(|term| term.trim().to_string())
            .filter(|term| !term.is_empty())
            .collect();
        self.protected_terms.sort_by(|left, right| {
            right
                .chars()
                .count()
                .cmp(&left.chars().count())
                .then_with(|| left.cmp(right))
        });
        self.protected_terms.dedup();
        self
    }
}

impl Default for TranslationOptions {
    fn default() -> Self {
        Self::from_pair(LanguagePair::default())
    }
}
