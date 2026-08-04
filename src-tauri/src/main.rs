use std::path::PathBuf;

use atogaki_subtitle::{
    application::{
        LocalTaskService, LocalWorkspaceService, TranscriptionOptions, job_spec::TranscribeSpec,
    },
    infrastructure::{
        config::AppConfig,
        local_db::{LocalDatabase, LocalJobRecord, LocalSubtitleSegmentRecord},
    },
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

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
                    config,
                    data_dir.join("jobs"),
                    database.clone(),
                )
            })?;
            let workspace_service = LocalWorkspaceService::new(database);
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
            get_job_detail,
            list_jobs,
            submit_transcription,
            update_subtitle
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Atogaki desktop application");
}
