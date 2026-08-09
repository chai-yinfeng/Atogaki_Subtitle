mod credential_store;
mod desktop_settings;
mod model_download;

use std::{ffi::OsString, path::PathBuf, sync::Arc};

use atogaki_subtitle::{
    application::{
        LocalGlossaryApplyResult, LocalGlossaryPreview, LocalGlossaryPromptPreview,
        LocalGlossaryService, LocalGlossaryTermDraft, LocalRenderRequest, LocalRenderService,
        LocalSubtitleExport, LocalSubtitleExportPlan, LocalTaskService, LocalTranslationStatus,
        LocalWorkspaceService, MutableTranslationProvider, TranscriptionOptions,
        UnconfiguredTranslationProvider, job_spec::TranscribeSpec,
    },
    domain::{LanguageCode, subtitle::SubtitleTrack},
    infrastructure::{
        config::{AppConfig, desktop_ffmpeg_path, desktop_whisper_cli_path},
        local_db::{
            LocalDatabase, LocalGlossaryDetail, LocalGlossaryRecord, LocalJobRecord,
            LocalRenderJobRecord, LocalSubtitleSegmentRecord,
        },
        media::MediaCapabilities,
    },
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    desktop_settings::{DesktopSettings, DesktopSettingsService, SaveDesktopSettingsRequest},
    model_download::{
        ModelCatalogItem, ModelDownloadService, ModelDownloadState, NetworkSourceCheck,
        test_download_network,
    },
};

struct DesktopState {
    data_dir: PathBuf,
    task_service: LocalTaskService,
    workspace_service: LocalWorkspaceService,
    glossary_service: LocalGlossaryService,
    render_service: LocalRenderService,
    settings_service: DesktopSettingsService,
    model_download_service: ModelDownloadService,
}

#[tauri::command]
async fn media_capabilities(state: State<'_, DesktopState>) -> Result<MediaCapabilities, String> {
    state
        .render_service
        .capabilities()
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitTranscriptionRequest {
    input_path: String,
    model_path: String,
    source_language: LanguageCode,
    vad_model_path: Option<String>,
    glossary_id: Option<String>,
    #[serde(default)]
    selected_content_groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptPreviewRequest {
    glossary_id: String,
    #[serde(default)]
    selected_content_groups: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopRecognitionDefaults {
    whisper_model_path: Option<String>,
    vad_model_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestDownloadNetworkRequest {
    proxy_mode: String,
    proxy_url: Option<String>,
    model_mirror_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveDownloadNetworkSettingsRequest {
    proxy_mode: String,
    proxy_url: Option<String>,
    model_mirror_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveGlossaryRequest {
    glossary_id: Option<String>,
    name: String,
    source_language: LanguageCode,
    terms: Vec<LocalGlossaryTermDraft>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSubtitleRequest {
    job_id: String,
    segment_id: String,
    source_text: String,
    translated_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleExportRequest {
    job_id: String,
    output_directory: String,
    overwrite_existing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleExportPlanRequest {
    job_id: String,
    output_directory: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoRenderRequest {
    source_job_id: String,
    output_path: String,
    subtitle_track: SubtitleTrack,
    overwrite_existing: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoOutputSelection {
    path: String,
    already_exists: bool,
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
async fn retry_job(state: State<'_, DesktopState>, job_id: String) -> Result<String, String> {
    let settings = state
        .settings_service
        .load()
        .await
        .map_err(|error| error.to_string())?;
    state
        .task_service
        .retry_persisted_job(
            &job_id,
            settings.whisper_model_path.map(PathBuf::from),
            settings.vad_model_path.map(PathBuf::from),
        )
        .await
        .map(|snapshot| snapshot.manifest.job_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn rename_job(
    state: State<'_, DesktopState>,
    job_id: String,
    display_name: Option<String>,
) -> Result<LocalJobRecord, String> {
    state
        .task_service
        .rename_persisted_job(&job_id, display_name)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn delete_job(state: State<'_, DesktopState>, job_id: String) -> Result<(), String> {
    state
        .task_service
        .delete_persisted_job(&job_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn submit_transcription(
    state: State<'_, DesktopState>,
    request: SubmitTranscriptionRequest,
) -> Result<String, String> {
    let transcription = desktop_transcription_options(&request)?;
    let snapshot = state
        .task_service
        .submit_transcription_with_glossary(
            TranscribeSpec {
                input: request.input_path.into(),
                output_dir: None,
                transcription,
            },
            request.glossary_id.as_deref(),
            &request.selected_content_groups,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(snapshot.manifest.job_id)
}

#[tauri::command]
async fn list_glossaries(
    state: State<'_, DesktopState>,
) -> Result<Vec<LocalGlossaryRecord>, String> {
    state
        .glossary_service
        .list()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_glossary(
    state: State<'_, DesktopState>,
    glossary_id: String,
) -> Result<LocalGlossaryDetail, String> {
    state
        .glossary_service
        .get(&glossary_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_glossary_prompt(
    state: State<'_, DesktopState>,
    request: PromptPreviewRequest,
) -> Result<LocalGlossaryPromptPreview, String> {
    state
        .glossary_service
        .prompt_preview(&request.glossary_id, &request.selected_content_groups)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_glossary(
    state: State<'_, DesktopState>,
    request: SaveGlossaryRequest,
) -> Result<LocalGlossaryDetail, String> {
    state
        .glossary_service
        .save(
            request.glossary_id.as_deref(),
            request.name,
            request.source_language,
            request.terms,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn delete_glossary(
    state: State<'_, DesktopState>,
    glossary_id: String,
) -> Result<(), String> {
    state
        .glossary_service
        .delete(&glossary_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_glossary_application(
    state: State<'_, DesktopState>,
    job_id: String,
    glossary_id: String,
) -> Result<LocalGlossaryPreview, String> {
    state
        .glossary_service
        .preview_apply(&job_id, &glossary_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn apply_glossary_to_workspace(
    state: State<'_, DesktopState>,
    job_id: String,
    glossary_id: String,
) -> Result<LocalGlossaryApplyResult, String> {
    state
        .glossary_service
        .apply(&job_id, &glossary_id)
        .await
        .map_err(|error| error.to_string())
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
            request.source_text,
            request.translated_text,
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
    request: SubtitleExportRequest,
) -> Result<LocalSubtitleExport, String> {
    state
        .workspace_service
        .export_subtitles_to(
            &request.job_id,
            &PathBuf::from(request.output_directory),
            request.overwrite_existing,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_workspace_subtitle_export(
    state: State<'_, DesktopState>,
    request: SubtitleExportPlanRequest,
) -> Result<LocalSubtitleExportPlan, String> {
    state
        .workspace_service
        .subtitle_export_plan(&request.job_id, &PathBuf::from(request.output_directory))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn submit_video_render(
    state: State<'_, DesktopState>,
    request: VideoRenderRequest,
) -> Result<LocalRenderJobRecord, String> {
    state
        .render_service
        .submit(LocalRenderRequest {
            source_job_id: request.source_job_id,
            output_path: PathBuf::from(request.output_path),
            subtitle_track: request.subtitle_track,
            overwrite_existing: request.overwrite_existing,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_video_renders(
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
async fn cancel_video_render(
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
async fn pick_vad_model_file(app: AppHandle) -> Result<Option<String>, String> {
    pick_local_file(
        &app,
        "选择 Silero VAD 模型",
        "VAD 模型",
        &["bin"],
        vad_model_picker_directory(&app),
    )
    .await
}

#[tauri::command]
async fn pick_subtitle_export_directory(
    app: AppHandle,
    initial_directory: Option<String>,
) -> Result<Option<String>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut picker = app.dialog().file().set_title("选择字幕导出目录");
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
async fn pick_video_output_file(
    app: AppHandle,
    initial_directory: Option<String>,
    suggested_name: String,
) -> Result<Option<VideoOutputSelection>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut picker = app
        .dialog()
        .file()
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
fn reveal_exported_subtitle(path: String) -> Result<(), String> {
    reveal_in_finder(path, "导出文件")
}

#[tauri::command]
fn reveal_rendered_video(path: String) -> Result<(), String> {
    reveal_in_finder(path, "烧录视频")
}

fn reveal_in_finder(path: String, label: &str) -> Result<(), String> {
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("无法定位{label}：{error}"))?;
    if !path.is_file() {
        return Err(format!("{label}不存在：{}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .status()
            .map_err(|error| format!("无法打开 Finder：{error}"))?;
        if !status.success() {
            return Err(format!("Finder 定位失败：{status}"));
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    Err("当前只支持在 macOS Finder 中定位导出文件。".to_string())
}

#[tauri::command]
async fn recognition_defaults(
    state: State<'_, DesktopState>,
) -> Result<DesktopRecognitionDefaults, String> {
    state
        .settings_service
        .load()
        .await
        .map(|settings| DesktopRecognitionDefaults {
            whisper_model_path: settings.whisper_model_path,
            vad_model_path: settings.vad_model_path,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn desktop_settings(state: State<'_, DesktopState>) -> Result<DesktopSettings, String> {
    state
        .settings_service
        .load()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_desktop_settings(
    state: State<'_, DesktopState>,
    request: SaveDesktopSettingsRequest,
) -> Result<DesktopSettings, String> {
    state
        .settings_service
        .save(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_download_network_settings(
    state: State<'_, DesktopState>,
    request: SaveDownloadNetworkSettingsRequest,
) -> Result<(), String> {
    state
        .settings_service
        .save_download_network_settings(
            &request.proxy_mode,
            request.proxy_url,
            request.model_mirror_url,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn model_catalog(state: State<'_, DesktopState>) -> Vec<ModelCatalogItem> {
    state.model_download_service.catalog()
}

#[tauri::command]
async fn start_model_download(
    state: State<'_, DesktopState>,
    model_id: String,
) -> Result<ModelDownloadState, String> {
    state
        .model_download_service
        .start(&model_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn model_download_states(
    state: State<'_, DesktopState>,
) -> Result<Vec<ModelDownloadState>, String> {
    Ok(state.model_download_service.states().await)
}

#[tauri::command]
async fn test_network_connection(
    request: TestDownloadNetworkRequest,
) -> Result<Vec<NetworkSourceCheck>, String> {
    test_download_network(
        &request.proxy_mode,
        request.proxy_url,
        request.model_mirror_url,
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

fn vad_model_picker_directory(app: &AppHandle) -> Option<PathBuf> {
    configured_file("ATOGAKI_VAD_MODEL")
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| model_picker_directory(app))
}

fn configured_file(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn desktop_transcription_options(
    request: &SubmitTranscriptionRequest,
) -> Result<TranscriptionOptions, String> {
    let mut options = TranscriptionOptions::new(
        request.model_path.trim().into(),
        request.source_language,
    );
    options.vad_model = request
        .vad_model_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    if let Some(path) = options.vad_model.as_deref()
        && !path.is_file()
    {
        return Err(format!("VAD 模型不存在：{}", path.display()));
    }
    Ok(options)
}

fn validated_data_dir_override(value: Option<OsString>) -> Result<Option<PathBuf>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Err("ATOGAKI_DATA_DIR must not be empty".to_string());
    }

    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!(
            "ATOGAKI_DATA_DIR must be an absolute path: {}",
            path.display()
        ));
    }
    std::fs::create_dir_all(&path).map_err(|error| {
        format!(
            "failed to create ATOGAKI_DATA_DIR {}: {error}",
            path.display()
        )
    })?;
    path.canonicalize().map(Some).map_err(|error| {
        format!(
            "failed to resolve ATOGAKI_DATA_DIR {}: {error}",
            path.display()
        )
    })
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = validated_data_dir_override(std::env::var_os("ATOGAKI_DATA_DIR"))?
                .unwrap_or(app.path().app_data_dir()?);
            let config = AppConfig {
                ffmpeg: desktop_ffmpeg_path(),
                whisper_cli: desktop_whisper_cli_path(),
                deepl_auth_key: std::env::var("DEEPL_AUTH_KEY").ok(),
            };
            let database = tauri::async_runtime::block_on(LocalDatabase::open(
                data_dir.join("atogaki.sqlite"),
            ))?;
            let translation_provider =
                MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider));
            let models_directory = data_dir.join("models");
            let settings_service = DesktopSettingsService::new(
                database.clone(),
                translation_provider.clone(),
                models_directory.clone(),
                config.deepl_auth_key.clone(),
                configured_file("ATOGAKI_WHISPER_MODEL"),
                configured_file("ATOGAKI_VAD_MODEL"),
            );
            tauri::async_runtime::block_on(settings_service.initialize())?;
            let glossary_service = LocalGlossaryService::new(database.clone());
            tauri::async_runtime::block_on(glossary_service.ensure_builtins())?;
            let task_service = tauri::async_runtime::block_on(async {
                LocalTaskService::start_with_database(
                    config.clone(),
                    data_dir.join("jobs"),
                    database.clone(),
                )
            })?;
            tauri::async_runtime::block_on(task_service.recover_interrupted_jobs())?;
            let workspace_service = LocalWorkspaceService::with_provider(
                database.clone(),
                Arc::new(translation_provider),
            );
            let render_service = tauri::async_runtime::block_on(LocalRenderService::start(
                config.ffmpeg.clone(),
                database.clone(),
                workspace_service.clone(),
            ))?;
            let model_download_service =
                ModelDownloadService::new(models_directory, settings_service.clone())?;
            app.manage(DesktopState {
                data_dir,
                task_service,
                workspace_service,
                glossary_service,
                render_service,
                settings_service,
                model_download_service,
            });
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            data_directory,
            desktop_settings,
            delete_job,
            delete_glossary,
            export_workspace_subtitles,
            get_glossary,
            get_job_detail,
            list_glossaries,
            list_jobs,
            list_video_renders,
            model_catalog,
            model_download_states,
            media_capabilities,
            pick_media_file,
            pick_model_file,
            pick_subtitle_export_directory,
            pick_video_output_file,
            pick_vad_model_file,
            apply_glossary_to_workspace,
            preview_glossary_application,
            preview_glossary_prompt,
            preview_workspace_subtitle_export,
            recognition_defaults,
            reveal_exported_subtitle,
            reveal_rendered_video,
            rename_job,
            retry_job,
            save_download_network_settings,
            save_desktop_settings,
            save_glossary,
            submit_transcription,
            start_model_download,
            test_network_connection,
            submit_video_render,
            cancel_video_render,
            translate_all_subtitles,
            translate_subtitle,
            translation_status,
            update_subtitle
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Atogaki desktop application");
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        SubmitTranscriptionRequest, desktop_transcription_options, validated_data_dir_override,
    };
    use atogaki_subtitle::domain::LanguageCode;

    #[test]
    fn desktop_data_directory_override_requires_an_absolute_path() {
        let error = validated_data_dir_override(Some("relative/test-data".into())).unwrap_err();

        assert!(error.contains("must be an absolute path"));
    }

    #[test]
    fn desktop_data_directory_override_creates_an_isolated_directory() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-desktop-data-test-{nonce}"));

        let resolved = validated_data_dir_override(Some(root.clone().into_os_string()))
            .unwrap()
            .unwrap();

        assert!(resolved.is_dir());
        assert_eq!(resolved, root.canonicalize().unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_request_keeps_vad_disabled_when_no_model_is_selected() {
        let request = SubmitTranscriptionRequest {
            input_path: "audio.wav".to_string(),
            model_path: "whisper.bin".to_string(),
            source_language: LanguageCode::Japanese,
            vad_model_path: None,
            glossary_id: None,
            selected_content_groups: Vec::new(),
        };

        let options = desktop_transcription_options(&request).unwrap();

        assert!(options.vad_model.is_none());
    }

    #[test]
    fn desktop_request_enables_the_selected_vad_model() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-vad-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let vad_model = root.join("ggml-silero.bin");
        fs::write(&vad_model, b"test model placeholder").unwrap();
        let request = SubmitTranscriptionRequest {
            input_path: "audio.wav".to_string(),
            model_path: "whisper.bin".to_string(),
            source_language: LanguageCode::English,
            vad_model_path: Some(vad_model.display().to_string()),
            glossary_id: None,
            selected_content_groups: Vec::new(),
        };

        let options = desktop_transcription_options(&request).unwrap();

        assert_eq!(options.vad_model.as_deref(), Some(vad_model.as_path()));
        assert_eq!(options.source_language, LanguageCode::English);
        fs::remove_dir_all(root).unwrap();
    }
}
