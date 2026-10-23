//! Subtitle workspace IPC boundary. Application services still own validation,
//! persistence, revisions and transactions; this module owns wire DTOs only.
use std::path::PathBuf;

use atogaki_subtitle::infrastructure::{
    local_db::{LocalSubtitleSegmentRecord, LocalTranslationRunRecord},
    waveform::WaveformWindow,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{DesktopState, DesktopTimedJob, media_server::PlaybackMediaRegistry};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateSubtitleRequest {
    job_id: String,
    segment_id: String,
    source_text: String,
    translated_text: Option<String>,
    start_ms: i64,
    end_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditSubtitleBlockRequest {
    job_id: String,
    before_segments: Vec<LocalSubtitleSegmentRecord>,
    remove_id: Option<String>,
    insertion: Option<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SplitSubtitleRequest {
    job_id: String,
    segment_id: String,
    boundary_ms: i64,
    left_source_text: String,
    right_source_text: String,
    left_translated_text: Option<String>,
    right_translated_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeSubtitlesRequest {
    job_id: String,
    left_segment_id: String,
    right_segment_id: String,
    source_text: String,
    translated_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreSubtitleStructureRequest {
    job_id: String,
    before_segments: Vec<LocalSubtitleSegmentRecord>,
    after_segments: Vec<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSubtitleTimingRequest {
    job_id: String,
    before_segments: Vec<LocalSubtitleSegmentRecord>,
    after_segments: Vec<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WaveformWindowRequest {
    job_id: String,
    start_ms: i64,
    end_ms: i64,
    point_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavePlaybackPositionRequest {
    job_id: String,
    position_ms: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopJobDetail {
    translation_context_stale_ids: Vec<String>,
    job: DesktopTimedJob,
    segments: Vec<LocalSubtitleSegmentRecord>,
    translation_runs: Vec<LocalTranslationRunRecord>,
    playback_path: Option<String>,
    audio_fallback_path: Option<String>,
}

#[tauri::command]
pub(crate) async fn get_job_detail(
    state: State<'_, DesktopState>,
    media_registry: State<'_, PlaybackMediaRegistry>,
    job_id: String,
) -> Result<DesktopJobDetail, String> {
    // Reconcile only newer output for the task being opened.
    state
        .task_service
        .refresh_persisted_job(&job_id)
        .await
        .map_err(|error| error.to_string())?;
    let detail = state
        .workspace_service
        .get_job(&job_id)
        .await
        .map_err(|error| error.to_string())?;
    let playback_path = register_playback_file(&media_registry, detail.job.input_path.as_deref())?;
    let fallback = PathBuf::from(&detail.job.storage_dir).join("audio.wav");
    let audio_fallback_path = register_playback_file(&media_registry, fallback.to_str())?;

    Ok(DesktopJobDetail {
        translation_context_stale_ids: detail
            .cue_provenance
            .iter()
            .filter(|cue| cue.translation_dependency_stale)
            .map(|cue| cue.segment_id.clone())
            .collect(),
        job: DesktopTimedJob::new(detail.job),
        segments: detail.segments,
        translation_runs: detail.translation_runs,
        playback_path,
        audio_fallback_path,
    })
}

#[tauri::command]
pub(crate) async fn update_subtitle(
    state: State<'_, DesktopState>,
    request: UpdateSubtitleRequest,
) -> Result<LocalSubtitleSegmentRecord, String> {
    state
        .workspace_service
        .update_subtitle(
            &request.job_id,
            &request.segment_id,
            request.source_text,
            request.translated_text,
            request.start_ms,
            request.end_ms,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn restore_subtitle(
    state: State<'_, DesktopState>,
    snapshot: LocalSubtitleSegmentRecord,
) -> Result<LocalSubtitleSegmentRecord, String> {
    state
        .workspace_service
        .restore_subtitle(&snapshot)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn edit_subtitle_block(
    state: State<'_, DesktopState>,
    request: EditSubtitleBlockRequest,
) -> Result<Vec<LocalSubtitleSegmentRecord>, String> {
    state
        .workspace_service
        .edit_subtitle_block(
            &request.job_id,
            &request.before_segments,
            request.remove_id.as_deref(),
            request.insertion,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn split_subtitle(
    state: State<'_, DesktopState>,
    request: SplitSubtitleRequest,
) -> Result<Vec<LocalSubtitleSegmentRecord>, String> {
    state
        .workspace_service
        .split_subtitle(
            &request.job_id,
            &request.segment_id,
            request.boundary_ms,
            request.left_source_text,
            request.right_source_text,
            request.left_translated_text,
            request.right_translated_text,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn merge_subtitles(
    state: State<'_, DesktopState>,
    request: MergeSubtitlesRequest,
) -> Result<Vec<LocalSubtitleSegmentRecord>, String> {
    state
        .workspace_service
        .merge_subtitles(
            &request.job_id,
            &request.left_segment_id,
            &request.right_segment_id,
            request.source_text,
            request.translated_text,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn restore_subtitle_structure(
    state: State<'_, DesktopState>,
    request: RestoreSubtitleStructureRequest,
) -> Result<Vec<LocalSubtitleSegmentRecord>, String> {
    state
        .workspace_service
        .restore_subtitle_structure(
            &request.job_id,
            &request.before_segments,
            &request.after_segments,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn save_subtitle_timing(
    state: State<'_, DesktopState>,
    request: SaveSubtitleTimingRequest,
) -> Result<Vec<LocalSubtitleSegmentRecord>, String> {
    state
        .workspace_service
        .save_subtitle_timing(
            &request.job_id,
            &request.before_segments,
            &request.after_segments,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn get_playback_position(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<i64, String> {
    state
        .workspace_service
        .playback_position(&job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn save_playback_position(
    state: State<'_, DesktopState>,
    request: SavePlaybackPositionRequest,
) -> Result<(), String> {
    state
        .workspace_service
        .save_playback_position(&request.job_id, request.position_ms)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn get_waveform_window(
    state: State<'_, DesktopState>,
    request: WaveformWindowRequest,
) -> Result<WaveformWindow, String> {
    state
        .workspace_service
        .waveform_window(
            &request.job_id,
            request.start_ms,
            request.end_ms,
            request.point_count,
        )
        .await
        .map_err(|error| format!("{error:#}"))
}

fn register_playback_file(
    registry: &PlaybackMediaRegistry,
    path: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(path) = path.map(PathBuf::from).filter(|path| path.is_file()) else {
        return Ok(None);
    };
    registry.register(&path).map(Some)
}
