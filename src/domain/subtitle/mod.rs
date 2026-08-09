use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::TranscriptSegment;

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
    let mut out = String::from(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\
         WrapStyle: 2\n\
         ScaledBorderAndShadow: yes\n\n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Source,Hiragino Sans,42,&H00FFFFFF,&H000000FF,&H80000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,80,80,120,1\n\
         Style: Translation,Hiragino Sans GB,46,&H00D7FFFE,&H000000FF,&H80000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,80,80,60,1\n\n\
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

    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
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
    use super::SubtitleTrack;

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
}
