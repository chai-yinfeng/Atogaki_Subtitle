//! Task style IPC; the existing service owns font validation and libass rendering.
use crate::{DesktopState, allow_asset_file};
use atogaki_subtitle::{
    application::{SubtitleFontFamily, SubtitleStylePreview, SubtitleStyleState},
    domain::subtitle::{SubtitleStyleSet, SubtitleTrack},
};
use serde::Deserialize;
use tauri::{AppHandle, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSubtitleStylesRequest {
    job_id: String,
    styles: SubtitleStyleSet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewSubtitleStylesRequest {
    job_id: String,
    styles: SubtitleStyleSet,
    subtitle_track: SubtitleTrack,
    timestamp_ms: u64,
}

#[tauri::command]
pub(crate) fn list_subtitle_fonts(state: State<'_, DesktopState>) -> Vec<SubtitleFontFamily> {
    state.subtitle_style_service.fonts().to_vec()
}

#[tauri::command]
pub(crate) async fn get_subtitle_style_state(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<SubtitleStyleState, String> {
    state
        .subtitle_style_service
        .get(&job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn save_subtitle_styles(
    state: State<'_, DesktopState>,
    request: SaveSubtitleStylesRequest,
) -> Result<SubtitleStyleState, String> {
    state
        .subtitle_style_service
        .save(&request.job_id, &request.styles)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn preview_subtitle_styles(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: PreviewSubtitleStylesRequest,
) -> Result<SubtitleStylePreview, String> {
    let preview = state
        .subtitle_style_service
        .preview(
            &request.job_id,
            &request.styles,
            request.subtitle_track,
            request.timestamp_ms,
        )
        .await
        .map_err(|error| error.to_string())?;
    allow_asset_file(&app, Some(&preview.output_path))?;
    Ok(preview)
}
