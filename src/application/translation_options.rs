/// Options for a translation run, independent from a particular user interface.
#[derive(Debug, Clone)]
pub struct TranslationOptions {
    pub source_language: String,
    pub target_language: String,
}

impl TranslationOptions {
    pub fn new(source_language: impl Into<String>, target_language: impl Into<String>) -> Self {
        Self {
            source_language: source_language.into(),
            target_language: target_language.into(),
        }
    }
}

impl Default for TranslationOptions {
    fn default() -> Self {
        Self::new("ja", "zh")
    }
}
