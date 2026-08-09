use std::{fmt, str::FromStr};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LanguageCode {
    #[serde(rename = "ja")]
    #[default]
    Japanese,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-Hans")]
    SimplifiedChinese,
}

impl LanguageCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Japanese => "ja",
            Self::English => "en",
            Self::SimplifiedChinese => "zh-Hans",
        }
    }

    pub const fn display_name_zh(self) -> &'static str {
        match self {
            Self::Japanese => "日语",
            Self::English => "英语",
            Self::SimplifiedChinese => "简体中文",
        }
    }

    pub const fn whisper_code(self) -> &'static str {
        match self {
            Self::Japanese => "ja",
            Self::English => "en",
            Self::SimplifiedChinese => "zh",
        }
    }

    pub const fn deepl_source_code(self) -> &'static str {
        match self {
            Self::Japanese => "JA",
            Self::English => "EN",
            Self::SimplifiedChinese => "ZH",
        }
    }

    pub const fn deepl_target_code(self) -> &'static str {
        match self {
            Self::Japanese => "JA",
            Self::English => "EN",
            Self::SimplifiedChinese => "ZH-HANS",
        }
    }
}

impl fmt::Display for LanguageCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LanguageCode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "ja" | "ja-jp" | "japanese" => Ok(Self::Japanese),
            "en" | "en-us" | "en-gb" | "english" => Ok(Self::English),
            "zh" | "zh-cn" | "zh-hans" | "chinese" => Ok(Self::SimplifiedChinese),
            _ => Err(format!(
                "unsupported language '{value}'; expected ja, en, or zh-Hans"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePair {
    pub source: LanguageCode,
    pub target: LanguageCode,
}

impl LanguagePair {
    pub fn new(source: LanguageCode, target: LanguageCode) -> Result<Self> {
        if source == target {
            bail!("source and target languages must be different: {source}");
        }
        Ok(Self { source, target })
    }

    pub const fn japanese_to_simplified_chinese() -> Self {
        Self {
            source: LanguageCode::Japanese,
            target: LanguageCode::SimplifiedChinese,
        }
    }

    pub const fn english_to_simplified_chinese() -> Self {
        Self {
            source: LanguageCode::English,
            target: LanguageCode::SimplifiedChinese,
        }
    }
}

impl Default for LanguagePair {
    fn default() -> Self {
        Self::japanese_to_simplified_chinese()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{LanguageCode, LanguagePair};

    #[test]
    fn normalizes_supported_language_aliases() {
        assert_eq!(
            LanguageCode::from_str("JA-jp").unwrap(),
            LanguageCode::Japanese
        );
        assert_eq!(
            LanguageCode::from_str("en-US").unwrap(),
            LanguageCode::English
        );
        assert_eq!(
            LanguageCode::from_str("zh_CN").unwrap(),
            LanguageCode::SimplifiedChinese
        );
    }

    #[test]
    fn rejects_same_language_pairs() {
        assert!(LanguagePair::new(LanguageCode::English, LanguageCode::English).is_err());
    }
}
