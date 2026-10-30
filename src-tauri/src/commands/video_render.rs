//! Render IPC; the existing service owns the worker, frozen ASS and output replacement.
use crate::{DesktopState, reveal_in_file_manager};
use atogaki_subtitle::{
    application::{LocalRenderEstimate, LocalRenderQuality, LocalRenderRequest},
    domain::subtitle::SubtitleTrack,
    infrastructure::{local_db::LocalRenderJobRecord, media::MediaCapabilities},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoRenderRequest {
    source_job_id: String,
    output_path: String,
    subtitle_track: SubtitleTrack,
    quality: LocalRenderQuality,
    overwrite_existing: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoOutputSelection {
    path: String,
    already_exists: bool,
}

#[tauri::command]
pub(crate) async fn media_capabilities(
    state: State<'_, DesktopState>,
) -> Result<MediaCapabilities, String> {
    state
        .render_service
        .capabilities()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn submit_video_render(
    state: State<'_, DesktopState>,
    request: VideoRenderRequest,
) -> Result<LocalRenderJobRecord, String> {
    state
        .render_service
        .submit(LocalRenderRequest {
            source_job_id: request.source_job_id,
            output_path: PathBuf::from(request.output_path),
            subtitle_track: request.subtitle_track,
            quality: request.quality,
            overwrite_existing: request.overwrite_existing,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn video_render_estimates(
    state: State<'_, DesktopState>,
    source_job_id: String,
) -> Result<Vec<LocalRenderEstimate>, String> {
    state
        .render_service
        .estimates(&source_job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn list_video_renders(
    state: State<'_, DesktopState>,
    source_job_id: String,
) -> Result<Vec<LocalRenderJobRecord>, String> {
    state
        .render_service
        .list(&source_job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn cancel_video_render(
    state: State<'_, DesktopState>,
    render_id: String,
) -> Result<LocalRenderJobRecord, String> {
    state
        .render_service
        .cancel(&render_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn pick_video_output_file(
    app: AppHandle,
    window: tauri::WebviewWindow,
    initial_directory: Option<String>,
    suggested_name: String,
) -> Result<Option<VideoOutputSelection>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut picker = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("导出带字幕视频")
        .add_filter("MP4 视频", &["mp4"])
        .set_file_name(suggested_name);
    let initial_directory = initial_directory
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| app.path().download_dir().ok().filter(|path| path.is_dir()));
    if let Some(directory) = initial_directory {
        picker = picker.set_directory(directory);
    }
    picker.save_file(move |selection| {
        let _ = sender.send(selection);
    });

    receiver
        .await
        .map_err(|_| "视频保存面板意外关闭，请重试。".to_owned())?
        .map(|selection| {
            selection
                .into_path()
                .map(|path| VideoOutputSelection {
                    already_exists: path.exists(),
                    path: path.display().to_string(),
                })
                .map_err(|error| format!("无法读取视频输出路径：{error}"))
        })
        .transpose()
}

#[tauri::command]
pub(crate) async fn video_output_exists(path: String) -> Result<bool, String> {
    tokio::fs::try_exists(path)
        .await
        .map_err(|error| format!("无法检查视频输出路径：{error}"))
}

#[tauri::command]
pub(crate) fn reveal_rendered_video(path: String) -> Result<(), String> {
    reveal_in_file_manager(path, "烧录视频")
}
