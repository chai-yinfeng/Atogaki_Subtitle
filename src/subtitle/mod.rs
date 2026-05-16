use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::segment::TranscriptSegment;

#[derive(Debug, Clone, Copy)]
pub enum SubtitleTrack {
    Japanese,
    Chinese,
}

pub fn write_srt(path: &Path, segments: &[TranscriptSegment], track: SubtitleTrack) -> Result<()> {
    let mut out = String::new();

    for (idx, seg) in segments.iter().enumerate() {
        let text = match track {
            SubtitleTrack::Japanese => seg.ja_text.as_str(),
            SubtitleTrack::Chinese => seg.zh_text.as_deref().unwrap_or(""),
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
    let mut out = String::from(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\
         WrapStyle: 2\n\
         ScaledBorderAndShadow: yes\n\n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Japanese,Arial,42,&H00FFFFFF,&H000000FF,&H80000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,80,80,120,1\n\
         Style: Chinese,Arial,46,&H00D7FFFE,&H000000FF,&H80000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,80,80,60,1\n\n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for seg in segments {
        out.push_str(&format!(
            "Dialogue: 0,{},{},Japanese,,0,0,0,,{}\n",
            format_ass_time(seg.start_ms),
            format_ass_time(seg.end_ms),
            escape_ass(&seg.ja_text)
        ));
        if let Some(zh) = seg.zh_text.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push_str(&format!(
                "Dialogue: 1,{},{},Chinese,,0,0,0,,{}\n",
                format_ass_time(seg.start_ms),
                format_ass_time(seg.end_ms),
                escape_ass(zh)
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
