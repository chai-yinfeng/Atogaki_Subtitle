use crate::domain::{LanguageCode, LanguagePair};

/// Options for a translation run, independent from a particular user interface.
#[derive(Debug, Clone)]
pub struct TranslationOptions {
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
}

impl TranslationOptions {
    pub fn new(source_language: LanguageCode, target_language: LanguageCode) -> Self {
        Self {
            source_language,
            target_language,
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
}

impl Default for TranslationOptions {
    fn default() -> Self {
        Self::from_pair(LanguagePair::default())
    }
}
