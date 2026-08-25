use std::{fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    application::{
        LocalWorkspaceService,
        local_workspace_service::workspace_segments,
        subtitle_font_service::{SubtitleFontFamily, SubtitleFontReport, SubtitleFontService},
    },
    domain::{
        TranscriptSegment,
        subtitle::{self, SubtitleStyleSet, SubtitleTrack},
    },
    infrastructure::media::{self, SubtitlePreviewRender},
};

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleStyleState {
    pub styles: SubtitleStyleSet,
    pub font_report: SubtitleFontReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleStylePreview {
    pub output_path: String,
    pub timestamp_ms: u64,
    pub font_report: SubtitleFontReport,
    pub font_events: Vec<String>,
}

#[derive(Clone)]
pub struct SubtitleStyleService {
    ffmpeg: PathBuf,
    workspace: LocalWorkspaceService,
    fonts: SubtitleFontService,
}

impl SubtitleStyleService {
    pub fn new(
        ffmpeg: PathBuf,
        workspace: LocalWorkspaceService,
        fonts: SubtitleFontService,
    ) -> Self {
        Self {
            ffmpeg,
            workspace,
            fonts,
        }
    }

    pub fn fonts(&self) -> &[SubtitleFontFamily] {
        self.fonts.families()
    }

    pub async fn get(&self, job_id: &str) -> Result<SubtitleStyleState> {
        let workspace = self.workspace.get_job(job_id).await?;
        Ok(style_state(
            &self.fonts,
            workspace.subtitle_styles,
            &workspace.segments,
        ))
    }

    pub async fn save(
        &self,
        job_id: &str,
        styles: &SubtitleStyleSet,
    ) -> Result<SubtitleStyleState> {
        let styles = self.workspace.save_subtitle_styles(job_id, styles).await?;
        let workspace = self.workspace.get_job(job_id).await?;
        Ok(style_state(&self.fonts, styles, &workspace.segments))
    }

    pub async fn preview(
        &self,
        job_id: &str,
        styles: &SubtitleStyleSet,
        track: SubtitleTrack,
        requested_timestamp_ms: u64,
    ) -> Result<SubtitleStylePreview> {
        styles.validate()?;
        let workspace = self.workspace.get_job(job_id).await?;
        if workspace.segments.is_empty() {
            return Err(anyhow!("cannot preview a task without subtitle segments"));
        }
        let mut segments = workspace_segments(&workspace.segments)?;
        let input = workspace.job.input_path.as_deref().map(PathBuf::from);
        let has_video = if let Some(input) = input.as_deref().filter(|path| path.is_file()) {
            media::probe_media(&self.ffmpeg, input).await?.has_video
        } else {
            false
        };
        let (preview_input, timestamp_ms) = if has_video {
            let selected = segment_at_or_near(&segments, requested_timestamp_ms);
            let timestamp_ms = requested_timestamp_ms
                .max(selected.start_ms)
                .min(selected.end_ms.saturating_sub(1));
            (input.as_deref(), timestamp_ms)
        } else {
            let selected = segment_at_or_near(&segments, requested_timestamp_ms).clone();
            segments = vec![TranscriptSegment {
                start_ms: 0,
                end_ms: 2_000,
                ..selected
            }];
            (None, 1_000)
        };
        let preview_directory = PathBuf::from(&workspace.job.storage_dir).join("previews");
        fs::create_dir_all(&preview_directory).with_context(|| {
            format!(
                "failed to create subtitle preview directory {}",
                preview_directory.display()
            )
        })?;
        let preview_id = Uuid::new_v4();
        let ass_path = preview_directory.join(format!("subtitle-style-{preview_id}.ass"));
        let image_path = preview_directory.join(format!("subtitle-style-{preview_id}.png"));
        subtitle::write_ass_track_with_styles(&ass_path, &segments, track, styles)?;
        let rendered = media::render_ass_preview_frame(
            &self.ffmpeg,
            preview_input,
            &ass_path,
            &image_path,
            timestamp_ms,
        )
        .await;
        let _ = fs::remove_file(&ass_path);
        let SubtitlePreviewRender {
            output_path,
            font_events,
        } = rendered?;
        remove_old_preview_images(&preview_directory, &image_path);
        let font_report = font_report(&self.fonts, styles, &workspace.segments);
        Ok(SubtitleStylePreview {
            output_path,
            timestamp_ms,
            font_report,
            font_events,
        })
    }
}

fn style_state(
    fonts: &SubtitleFontService,
    styles: SubtitleStyleSet,
    segments: &[crate::infrastructure::local_db::LocalSubtitleSegmentRecord],
) -> SubtitleStyleState {
    let font_report = font_report(fonts, &styles, segments);
    SubtitleStyleState {
        styles,
        font_report,
    }
}

fn font_report(
    fonts: &SubtitleFontService,
    styles: &SubtitleStyleSet,
    segments: &[crate::infrastructure::local_db::LocalSubtitleSegmentRecord],
) -> SubtitleFontReport {
    let source = segments
        .iter()
        .map(|segment| segment.source_text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let translation = segments
        .iter()
        .filter_map(|segment| segment.translated_text.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    fonts.check_styles(styles, &source, &translation)
}

fn segment_at_or_near(segments: &[TranscriptSegment], timestamp_ms: u64) -> &TranscriptSegment {
    segments
        .iter()
        .find(|segment| timestamp_ms >= segment.start_ms && timestamp_ms < segment.end_ms)
        .or_else(|| {
            segments.iter().min_by_key(|segment| {
                if timestamp_ms < segment.start_ms {
                    segment.start_ms - timestamp_ms
                } else {
                    timestamp_ms.saturating_sub(segment.end_ms)
                }
            })
        })
        .expect("preview requires at least one segment")
}

fn remove_old_preview_images(directory: &std::path::Path, current: &std::path::Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()).filter(|path| {
        path != current
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("subtitle-style-") && name.ends_with(".png"))
    }) {
        let _ = fs::remove_file(path);
    }
}
