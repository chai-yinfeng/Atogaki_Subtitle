use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::TranscriptSegment;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleStyle {
    pub font_family: String,
    pub font_size: f64,
    pub primary_color: String,
    pub outline_color: String,
    pub outline_width: f64,
    pub shadow: f64,
    pub background_color: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub alignment: u8,
    pub margin_left: u16,
    pub margin_right: u16,
    pub margin_vertical: u16,
    pub letter_spacing: f64,
}

impl SubtitleStyle {
    pub fn validate(&self) -> Result<()> {
        let family = self.font_family.trim();
        if family.is_empty() || family.contains([',', '\n', '\r']) {
            anyhow::bail!(
                "subtitle font family must be non-empty and cannot contain commas or newlines"
            );
        }
        validate_number("font size", self.font_size, 8.0, 200.0)?;
        validate_number("outline width", self.outline_width, 0.0, 20.0)?;
        validate_number("shadow", self.shadow, 0.0, 20.0)?;
        validate_number("letter spacing", self.letter_spacing, -20.0, 50.0)?;
        if !(1..=9).contains(&self.alignment) {
            anyhow::bail!("subtitle alignment must be between 1 and 9");
        }
        parse_color(&self.primary_color)?;
        parse_color(&self.outline_color)?;
        if let Some(color) = &self.background_color {
            parse_color(color)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleStyleSet {
    #[serde(default = "subtitle_style_schema_version")]
    pub schema_version: u32,
    pub source: SubtitleStyle,
    pub translation: SubtitleStyle,
}

impl SubtitleStyleSet {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != subtitle_style_schema_version() {
            anyhow::bail!(
                "unsupported subtitle style schema version {}",
                self.schema_version
            );
        }
        self.source
            .validate()
            .context("invalid source subtitle style")?;
        self.translation
            .validate()
            .context("invalid translation subtitle style")
    }
}

impl Default for SubtitleStyleSet {
    fn default() -> Self {
        Self {
            schema_version: subtitle_style_schema_version(),
            source: SubtitleStyle {
                font_family: "Hiragino Sans".to_string(),
                font_size: 42.0,
                primary_color: "#FFFFFF".to_string(),
                outline_color: "#0000007F".to_string(),
                outline_width: 2.0,
                shadow: 0.0,
                background_color: None,
                bold: false,
                italic: false,
                alignment: 2,
                margin_left: 80,
                margin_right: 80,
                margin_vertical: 120,
                letter_spacing: 0.0,
            },
            translation: SubtitleStyle {
                font_family: "Hiragino Sans GB".to_string(),
                font_size: 46.0,
                primary_color: "#FEFFD7".to_string(),
                outline_color: "#0000007F".to_string(),
                outline_width: 2.0,
                shadow: 0.0,
                background_color: None,
                bold: false,
                italic: false,
                alignment: 2,
                margin_left: 80,
                margin_right: 80,
                margin_vertical: 60,
                letter_spacing: 0.0,
            },
        }
    }
}

const fn subtitle_style_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubtitleTrack {
    #[serde(rename = "source", alias = "japanese")]
    Source,
    #[serde(rename = "translation", alias = "chinese")]
    Translation,
    #[serde(rename = "bilingual")]
    Bilingual,
}

pub fn write_srt(path: &Path, segments: &[TranscriptSegment], track: SubtitleTrack) -> Result<()> {
    let mut out = String::new();

    for (idx, seg) in segments.iter().enumerate() {
        let text = match track {
            SubtitleTrack::Source => seg.source_text.as_str(),
            SubtitleTrack::Translation => seg.translated_text.as_deref().unwrap_or(""),
            SubtitleTrack::Bilingual => {
                let translation = seg.translated_text.as_deref().unwrap_or("").trim();
                if translation.is_empty() {
                    seg.source_text.as_str()
                } else {
                    out.push_str(&(idx + 1).to_string());
                    out.push('\n');
                    out.push_str(&format!(
                        "{} --> {}\n",
                        format_srt_time(seg.start_ms),
                        format_srt_time(seg.end_ms)
                    ));
                    out.push_str(seg.source_text.trim());
                    out.push('\n');
                    out.push_str(translation);
                    out.push_str("\n\n");
                    continue;
                }
            }
        };
        if text.trim().is_empty() {
            continue;
        }

        out.push_str(&(idx + 1).to_string());
        out.push('\n');
        out.push_str(&format!(
            "{} --> {}\n",
            format_srt_time(seg.start_ms),
            format_srt_time(seg.end_ms)
        ));
        out.push_str(text.trim());
        out.push_str("\n\n");
    }

    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

pub fn write_ass(path: &Path, segments: &[TranscriptSegment]) -> Result<()> {
    write_ass_track(path, segments, SubtitleTrack::Bilingual)
}

pub fn write_ass_track(
    path: &Path,
    segments: &[TranscriptSegment],
    track: SubtitleTrack,
) -> Result<()> {
    write_ass_track_with_styles(path, segments, track, &SubtitleStyleSet::default())
}

pub fn write_ass_track_with_styles(
    path: &Path,
    segments: &[TranscriptSegment],
    track: SubtitleTrack,
    styles: &SubtitleStyleSet,
) -> Result<()> {
    let out = ass_text(segments, track, styles)?;
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}

pub fn ass_text(
    segments: &[TranscriptSegment],
    track: SubtitleTrack,
    styles: &SubtitleStyleSet,
) -> Result<String> {
    styles.validate()?;
    let mut out = String::from(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\
         WrapStyle: 2\n\
         ScaledBorderAndShadow: yes\n\n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n",
    );
    out.push_str(&ass_style_line("Source", &styles.source)?);
    out.push_str(&ass_style_line("Translation", &styles.translation)?);
    out.push_str(
        "\n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for seg in segments {
        if matches!(track, SubtitleTrack::Source | SubtitleTrack::Bilingual) {
            out.push_str(&format!(
                "Dialogue: 0,{},{},Source,,0,0,0,,{}\n",
                format_ass_time(seg.start_ms),
                format_ass_time(seg.end_ms),
                escape_ass(&seg.source_text)
            ));
        }
        if matches!(track, SubtitleTrack::Translation | SubtitleTrack::Bilingual)
            && let Some(translation) = seg
                .translated_text
                .as_deref()
                .filter(|s| !s.trim().is_empty())
        {
            out.push_str(&format!(
                "Dialogue: 1,{},{},Translation,,0,0,0,,{}\n",
                format_ass_time(seg.start_ms),
                format_ass_time(seg.end_ms),
                escape_ass(translation)
            ));
        }
    }

    Ok(out)
}

fn ass_style_line(name: &str, style: &SubtitleStyle) -> Result<String> {
    style.validate()?;
    let background = style.background_color.as_deref().unwrap_or("#0000007F");
    let border_style = if style.background_color.is_some() {
        3
    } else {
        1
    };
    Ok(format!(
        "Style: {name},{},{},{},&H000000FF,{},{},{},{},0,0,100,100,{},0,{border_style},{},{},{},{},{},{},1\n",
        style.font_family.trim(),
        trim_float(style.font_size),
        ass_color(&style.primary_color)?,
        ass_color(&style.outline_color)?,
        ass_color(background)?,
        ass_bool(style.bold),
        ass_bool(style.italic),
        trim_float(style.letter_spacing),
        trim_float(style.outline_width),
        trim_float(style.shadow),
        style.alignment,
        style.margin_left,
        style.margin_right,
        style.margin_vertical,
    ))
}

fn ass_bool(value: bool) -> i8 {
    if value { -1 } else { 0 }
}

fn trim_float(value: f64) -> String {
    let value = format!("{value:.2}");
    value
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn validate_number(label: &str, value: f64, minimum: f64, maximum: f64) -> Result<()> {
    if !value.is_finite() || value < minimum || value > maximum {
        anyhow::bail!("subtitle {label} must be between {minimum} and {maximum}");
    }
    Ok(())
}

fn parse_color(value: &str) -> Result<(u8, u8, u8, u8)> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| anyhow::anyhow!("subtitle color must start with #"))?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("subtitle color must use #RRGGBB or #RRGGBBAA");
    }
    let component = |start| u8::from_str_radix(&hex[start..start + 2], 16).unwrap();
    Ok((
        component(0),
        component(2),
        component(4),
        if hex.len() == 8 { component(6) } else { 255 },
    ))
}

fn ass_color(value: &str) -> Result<String> {
    let (red, green, blue, opacity) = parse_color(value)?;
    let alpha = 255_u8.saturating_sub(opacity);
    Ok(format!("&H{alpha:02X}{blue:02X}{green:02X}{red:02X}"))
}

fn format_srt_time(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn format_ass_time(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let centis = (ms % 1_000) / 10;
    format!("{hours}:{minutes:02}:{seconds:02}.{centis:02}")
}

fn escape_ass(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', "\\N")
}

#[cfg(test)]
mod tests {
    use super::{SubtitleStyleSet, SubtitleTrack, ass_text};
    use crate::domain::TranscriptSegment;

    #[test]
    fn legacy_language_specific_track_names_remain_readable() {
        assert_eq!(
            serde_json::from_str::<SubtitleTrack>("\"japanese\"").unwrap(),
            SubtitleTrack::Source
        );
        assert_eq!(
            serde_json::from_str::<SubtitleTrack>("\"chinese\"").unwrap(),
            SubtitleTrack::Translation
        );
        assert_eq!(
            serde_json::to_string(&SubtitleTrack::Translation).unwrap(),
            "\"translation\""
        );
    }

    #[test]
    fn custom_styles_serialize_to_ass_colors_and_metrics() {
        let mut styles = SubtitleStyleSet::default();
        styles.source.font_family = "Arial".to_string();
        styles.source.primary_color = "#12345680".to_string();
        styles.source.bold = true;
        styles.source.italic = true;
        styles.source.letter_spacing = 1.5;
        let ass = ass_text(
            &[TranscriptSegment::new(0, 1_000, "hello".to_string())],
            SubtitleTrack::Source,
            &styles,
        )
        .unwrap();
        assert!(ass.contains("Style: Source,Arial,42,&H7F563412"));
        assert!(ass.contains(",-1,-1,0,0,100,100,1.5,"));
        assert!(ass.contains("Dialogue: 0,0:00:00.00,0:00:01.00,Source"));
    }

    #[test]
    fn default_styles_preserve_the_existing_ass_projection() {
        let ass = ass_text(&[], SubtitleTrack::Bilingual, &SubtitleStyleSet::default()).unwrap();
        assert!(ass.contains(
            "Style: Source,Hiragino Sans,42,&H00FFFFFF,&H000000FF,&H80000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,80,80,120,1"
        ));
        assert!(ass.contains(
            "Style: Translation,Hiragino Sans GB,46,&H00D7FFFE,&H000000FF,&H80000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,80,80,60,1"
        ));
    }

    #[test]
    fn invalid_font_family_cannot_inject_an_ass_record() {
        let mut styles = SubtitleStyleSet::default();
        styles.source.font_family = "Arial,Injected".to_string();
        assert!(ass_text(&[], SubtitleTrack::Bilingual, &styles).is_err());
    }
}
