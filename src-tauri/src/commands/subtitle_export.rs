//! Subtitle file export IPC; application services retain file and workspace validation.
use crate::{DesktopState, reveal_in_file_manager};
use atogaki_subtitle::application::{
    LocalSubtitleExport, LocalSubtitleExportArtifact, LocalSubtitleExportPlan,
};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubtitleExportRequest {
    job_id: String,
    output_directory: String,
    overwrite_existing: bool,
    artifacts: Vec<LocalSubtitleExportArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubtitleExportPlanRequest {
    job_id: String,
    output_directory: String,
    artifacts: Vec<LocalSubtitleExportArtifact>,
}

#[tauri::command]
pub(crate) async fn export_workspace_subtitles(
    state: State<'_, DesktopState>,
    request: SubtitleExportRequest,
) -> Result<LocalSubtitleExport, String> {
    state
        .workspace_service
        .export_subtitles_to(
            &request.job_id,
            &PathBuf::from(request.output_directory),
            request.overwrite_existing,
            &request.artifacts,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn preview_workspace_subtitle_export(
    state: State<'_, DesktopState>,
    request: SubtitleExportPlanRequest,
) -> Result<LocalSubtitleExportPlan, String> {
    state
        .workspace_service
        .subtitle_export_plan(
            &request.job_id,
            &PathBuf::from(request.output_directory),
            &request.artifacts,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn pick_subtitle_export_directory(
    app: AppHandle,
    window: tauri::WebviewWindow,
    initial_directory: Option<String>,
) -> Result<Option<String>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut picker = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("选择字幕导出目录");
    let initial_directory = initial_directory
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| app.path().download_dir().ok().filter(|path| path.is_dir()));
    if let Some(directory) = initial_directory {
        picker = picker.set_directory(directory);
    }
    picker.pick_folder(move |selection| {
        let _ = sender.send(selection);
    });

    receiver
        .await
        .map_err(|_| "目录选择器意外关闭，请重试。".to_owned())?
        .map(|selection| {
            selection
                .into_path()
                .map(|path| path.display().to_string())
                .map_err(|error| format!("无法读取所选目录：{error}"))
        })
        .transpose()
}

#[tauri::command]
pub(crate) fn reveal_exported_subtitle(path: String) -> Result<(), String> {
    reveal_in_file_manager(path, "导出文件")
}
