use std::path::PathBuf;

use atogaki_subtitle::{
    application::{LocalTaskService, TranscriptionOptions, job_spec::TranscribeSpec},
    infrastructure::{config::AppConfig, local_db::LocalJobRecord},
};
use serde::Deserialize;
use tauri::{Manager, State};

struct DesktopState {
    data_dir: PathBuf,
    task_service: LocalTaskService,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitTranscriptionRequest {
    input_path: String,
    model_path: String,
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
fn data_directory(state: State<'_, DesktopState>) -> String {
    state.data_dir.display().to_string()
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
            let task_service = tauri::async_runtime::block_on(LocalTaskService::start_persistent(
                config,
                data_dir.join("jobs"),
                data_dir.join("atogaki.sqlite"),
            ))?;
            app.manage(DesktopState {
                data_dir,
                task_service,
            });
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            data_directory,
            list_jobs,
            submit_transcription
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Atogaki desktop application");
}
