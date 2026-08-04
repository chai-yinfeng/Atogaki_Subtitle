use std::path::PathBuf;

use atogaki_subtitle::{
    application::{
        LocalSubtitleExport, LocalTaskService, LocalTranslationStatus, LocalWorkspaceService,
        TranscriptionOptions, job_spec::TranscribeSpec,
    },
    infrastructure::{
        config::AppConfig,
        local_db::{LocalDatabase, LocalJobRecord, LocalSubtitleSegmentRecord},
    },
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

struct DesktopState {
    data_dir: PathBuf,
    task_service: LocalTaskService,
    workspace_service: LocalWorkspaceService,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitTranscriptionRequest {
    input_path: String,
    model_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSubtitleRequest {
    job_id: String,
    segment_id: String,
    ja_text: String,
    zh_text: Option<String>,
}

#[derive(Debug, Serialize)]
struct DesktopJobDetail {
    job: LocalJobRecord,
    segments: Vec<LocalSubtitleSegmentRecord>,
    playback_path: Option<String>,
    audio_fallback_path: Option<String>,
}

#[tauri::command]
async fn list_jobs(state: State<'_, DesktopState>) -> Result<Vec<LocalJobRecord>, String> {
    state
        .task_service
        .list_persisted_jobs()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn submit_transcription(
    state: State<'_, DesktopState>,
    request: SubmitTranscriptionRequest,
) -> Result<String, String> {
    let snapshot = state
        .task_service
        .submit_transcription(TranscribeSpec {
            input: request.input_path.into(),
            output_dir: None,
            transcription: TranscriptionOptions::japanese(request.model_path.into()),
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(snapshot.manifest.job_id)
}

#[tauri::command]
async fn get_job_detail(
    app: AppHandle,
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<DesktopJobDetail, String> {
    // Pull the latest generated snapshot into SQLite before opening the editor.
    state
        .task_service
        .list_persisted_jobs()
        .await
        .map_err(|error| error.to_string())?;
    let detail = state
        .workspace_service
        .get_job(&job_id)
        .await
        .map_err(|error| error.to_string())?;
    let playback_path = allow_playback_file(&app, detail.job.input_path.as_deref())?;
    let fallback = PathBuf::from(&detail.job.storage_dir).join("audio.wav");
    let audio_fallback_path = allow_playback_file(&app, fallback.to_str())?;

    Ok(DesktopJobDetail {
        job: detail.job,
        segments: detail.segments,
        playback_path,
        audio_fallback_path,
    })
}

#[tauri::command]
async fn update_subtitle(
    state: State<'_, DesktopState>,
    request: UpdateSubtitleRequest,
) -> Result<LocalSubtitleSegmentRecord, String> {
    state
        .workspace_service
        .update_subtitle_text(
            &request.job_id,
            &request.segment_id,
            request.ja_text,
            request.zh_text,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn translation_status(state: State<'_, DesktopState>) -> LocalTranslationStatus {
    state.workspace_service.translation_status()
}

#[tauri::command]
async fn translate_subtitle(
    state: State<'_, DesktopState>,
    job_id: String,
    segment_id: String,
) -> Result<LocalSubtitleSegmentRecord, String> {
    state
        .workspace_service
        .translate_segment(&job_id, &segment_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn translate_all_subtitles(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Vec<LocalSubtitleSegmentRecord>, String> {
    state
        .workspace_service
        .translate_all(&job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn export_workspace_subtitles(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<LocalSubtitleExport, String> {
    state
        .workspace_service
        .export_subtitles(&job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn pick_media_file(app: AppHandle) -> Result<Option<String>, String> {
    pick_local_file(
        &app,
        "选择音频或视频文件",
        "媒体",
        &["mp3", "m4a", "wav", "mp4", "mkv", "webm", "mov"],
        None,
    )
    .await
}

#[tauri::command]
async fn pick_model_file(app: AppHandle) -> Result<Option<String>, String> {
    pick_local_file(
        &app,
        "选择 Whisper 模型",
        "Whisper 模型",
        &["bin"],
        model_picker_directory(&app),
    )
    .await
}

#[tauri::command]
fn data_directory(state: State<'_, DesktopState>) -> String {
    state.data_dir.display().to_string()
}

fn allow_playback_file(app: &AppHandle, path: Option<&str>) -> Result<Option<String>, String> {
    let Some(path) = path.map(PathBuf::from).filter(|path| path.is_file()) else {
        return Ok(None);
    };
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|error| format!("failed to allow local media playback: {error}"))?;
    Ok(Some(path.display().to_string()))
}

async fn pick_local_file(
    app: &AppHandle,
    title: &str,
    filter_name: &str,
    extensions: &[&str],
    initial_directory: Option<PathBuf>,
) -> Result<Option<String>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut picker = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, extensions);
    if let Some(directory) = initial_directory {
        picker = picker.set_directory(directory);
    }

    // The JavaScript dialog command uses blocking_pick_file. On macOS the native
    // panel must stay attached to the main event loop, so use the callback API here.
    picker.pick_file(move |selection| {
        let _ = sender.send(selection);
    });

    receiver
        .await
        .map_err(|_| "文件选择器意外关闭，请直接粘贴文件路径后重试。".to_owned())?
        .map(|selection| {
            selection
                .into_path()
                .map(|path| path.display().to_string())
                .map_err(|error| format!("无法读取所选文件路径：{error}"))
        })
        .transpose()
}

fn model_picker_directory(app: &AppHandle) -> Option<PathBuf> {
    std::env::var_os("ATOGAKI_WHISPER_MODEL")
        .map(PathBuf::from)
        .and_then(|path| path.parent().map(PathBuf::from))
        .filter(|path| path.is_dir())
        .or_else(|| {
            app.path()
                .home_dir()
                .ok()
                .map(|home| home.join("Models"))
                .filter(|path| path.is_dir())
        })
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let config = AppConfig {
                ffmpeg: std::env::var_os("ATOGAKI_FFMPEG")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("ffmpeg")),
                whisper_cli: std::env::var_os("ATOGAKI_WHISPER_CLI")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("whisper-cli")),
                deepl_auth_key: std::env::var("DEEPL_AUTH_KEY").ok(),
            };
            let database = tauri::async_runtime::block_on(LocalDatabase::open(
                data_dir.join("atogaki.sqlite"),
            ))?;
            let task_service = tauri::async_runtime::block_on(async {
                LocalTaskService::start_with_database(
                    config.clone(),
                    data_dir.join("jobs"),
                    database.clone(),
                )
            })?;
            let workspace_service =
                LocalWorkspaceService::with_deepl(database, config.deepl_auth_key);
            app.manage(DesktopState {
                data_dir,
                task_service,
                workspace_service,
            });
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            data_directory,
            export_workspace_subtitles,
            get_job_detail,
            list_jobs,
            pick_media_file,
            pick_model_file,
            submit_transcription,
            translate_all_subtitles,
            translate_subtitle,
            translation_status,
            update_subtitle
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Atogaki desktop application");
}
