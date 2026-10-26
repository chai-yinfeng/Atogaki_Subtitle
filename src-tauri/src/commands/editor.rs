//! Subtitle workspace IPC boundary. Application services still own validation,
//! persistence, revisions and transactions; this module owns wire DTOs only.
use std::path::PathBuf;

use atogaki_subtitle::application::{
    structure_edit_session::{StructureEditResult, StructureEditState},
    structure_undo_history::StructureHistoryUndo,
};
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecordedSplitSubtitleRequest {
    job_id: String,
    segment_id: String,
    boundary_ms: i64,
    left_source_text: String,
    right_source_text: String,
    left_translated_text: Option<String>,
    right_translated_text: Option<String>,
    expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecordedMergeSubtitlesRequest {
    job_id: String,
    left_segment_id: String,
    right_segment_id: String,
    source_text: String,
    translated_text: Option<String>,
    expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UndoStructureRequest {
    job_id: String,
    token: String,
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

// All editor structure operations use the same backend session. No IPC accepts
// client snapshots for restoring provenance or translation dependencies.
#[tauri::command]
pub(crate) async fn get_structure_edit_state(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<StructureEditState, String> {
    state
        .workspace_service
        .structure_edits()
        .state(&job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn edit_subtitle_block_recorded(
    state: State<'_, DesktopState>,
    request: EditSubtitleBlockRequest,
) -> Result<StructureEditResult, String> {
    state
        .workspace_service
        .structure_edits()
        .edit_block(
            &request.job_id,
            &request.before_segments,
            request.remove_id.as_deref(),
            request.insertion,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn split_subtitle_recorded(
    state: State<'_, DesktopState>,
    request: RecordedSplitSubtitleRequest,
) -> Result<StructureEditResult, String> {
    state
        .workspace_service
        .structure_edits()
        .split(
            &request.job_id,
            request.expected_revision,
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
pub(crate) async fn merge_subtitles_recorded(
    state: State<'_, DesktopState>,
    request: RecordedMergeSubtitlesRequest,
) -> Result<StructureEditResult, String> {
    state
        .workspace_service
        .structure_edits()
        .merge(
            &request.job_id,
            request.expected_revision,
            &request.left_segment_id,
            &request.right_segment_id,
            request.source_text,
            request.translated_text,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn save_subtitle_timing_recorded(
    state: State<'_, DesktopState>,
    request: SaveSubtitleTimingRequest,
) -> Result<StructureEditResult, String> {
    state
        .workspace_service
        .structure_edits()
        .save_timing(
            &request.job_id,
            &request.before_segments,
            &request.after_segments,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn undo_subtitle_structure(
    state: State<'_, DesktopState>,
    request: UndoStructureRequest,
) -> Result<StructureHistoryUndo, String> {
    state
        .workspace_service
        .structure_edits()
        .undo(&request.job_id, &request.token)
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

#[cfg(test)]
mod structure_wire_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recorded_cut_and_join_require_explicit_workspace_revision() {
        let mut cut = json!({"jobId":"job","segmentId":"cue","boundaryMs":500,
            "leftSourceText":"left","rightSourceText":"right",
            "leftTranslatedText":null,"rightTranslatedText":null});
        assert!(serde_json::from_value::<RecordedSplitSubtitleRequest>(cut.clone()).is_err());
        cut["expectedRevision"] = json!(42);
        let request: RecordedSplitSubtitleRequest = serde_json::from_value(cut.clone()).unwrap();
        assert_eq!(request.expected_revision, 42);
        assert_eq!(request.job_id, "job");
        cut["sourceRunId"] = json!("forged");
        assert!(serde_json::from_value::<RecordedSplitSubtitleRequest>(cut).is_err());
        let mut join = json!({"jobId":"job","leftSegmentId":"left","rightSegmentId":"right",
            "sourceText":"joined","translatedText":null});
        assert!(serde_json::from_value::<RecordedMergeSubtitlesRequest>(join.clone()).is_err());
        join["expectedRevision"] = json!(43);
        assert_eq!(
            serde_json::from_value::<RecordedMergeSubtitlesRequest>(join)
                .unwrap()
                .expected_revision,
            43
        );
    }

    #[test]
    fn undo_accepts_only_task_and_opaque_token_not_client_snapshots() {
        let request: UndoStructureRequest =
            serde_json::from_value(json!({"jobId":"job","token":"opaque"})).unwrap();
        assert_eq!(request.token, "opaque");
        assert!(
            serde_json::from_value::<UndoStructureRequest>(
                json!({"jobId":"job","beforeSegments":[],"afterSegments":[]})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<UndoStructureRequest>(
                json!({"jobId":"job","token":"opaque","provenance":[]})
            )
            .is_err()
        );
    }
}
