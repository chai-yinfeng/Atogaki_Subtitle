#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod credential_store;
mod desktop_settings;
mod dictionary_download;
mod dictionary_lookup;
mod model_download;

use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use atogaki_subtitle::{
    application::{
        LocalGlossaryApplyResult, LocalGlossaryPreview, LocalGlossaryPromptPreview,
        LocalGlossaryService, LocalGlossaryTermDraft, LocalLearningService, LocalRenderRequest,
        LocalRenderService, LocalSubtitleExport, LocalSubtitleExportArtifact,
        LocalSubtitleExportPlan, LocalTaskService, LocalTranslationStatus, LocalWorkspaceService,
        MutableTranslationProvider, TranscriptionOptions, UnconfiguredTranslationProvider,
        job_spec::TranscribeSpec,
    },
    domain::{LanguageCode, subtitle::SubtitleTrack},
    infrastructure::{
        config::{AppConfig, desktop_ffmpeg_path, desktop_whisper_cli_path},
        local_db::{
            LocalDatabase, LocalGlossaryDetail, LocalGlossaryRecord, LocalJobRecord,
            LocalJobTranslationStats, LocalLearningItemDetail, LocalRenderJobRecord,
            LocalSubtitleSegmentRecord, LocalTranslationRunRecord, NewLocalLearningSelection,
        },
        media::MediaCapabilities,
        waveform::WaveformWindow,
    },
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;

use crate::{
    desktop_settings::{
        DesktopSettings, DesktopSettingsService, DictionaryCredentialStatus,
        SaveDesktopSettingsRequest, SaveDictionaryCredentialRequest, TranslationCredentialCheck,
    },
    dictionary_download::{
        DictionaryCatalogItem, DictionaryDownloadService, DictionaryDownloadState,
    },
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
    learning_service: LocalLearningService,
    render_service: LocalRenderService,
    settings_service: DesktopSettingsService,
    model_download_service: ModelDownloadService,
    dictionary_download_service: DictionaryDownloadService,
    dictionary_lookup_service: dictionary_lookup::DictionaryLookupService,
}

const SUBTITLE_OVERLAY_LABEL: &str = "subtitle-overlay";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleOverlayPayload {
    source_text: String,
    translated_text: Option<String>,
    source_language: LanguageCode,
    target_language: LanguageCode,
    playing: bool,
    playback_rate: f64,
}

#[derive(Default)]
struct SubtitleOverlayState {
    current: Mutex<Option<SubtitleOverlayPayload>>,
    #[cfg(target_os = "macos")]
    native_panel: Mutex<Option<usize>>,
}

#[tauri::command]
fn open_subtitle_overlay(
    app: AppHandle,
    state: State<'_, SubtitleOverlayState>,
    payload: SubtitleOverlayPayload,
) -> Result<(), String> {
    set_subtitle_overlay_payload(&app, &state, payload)?;

    if let Some(window) = app.get_webview_window(SUBTITLE_OVERLAY_LABEL) {
        #[cfg(target_os = "macos")]
        show_subtitle_overlay_macos(&app, &window)?;
        #[cfg(not(target_os = "macos"))]
        {
            window.show().map_err(|error| error.to_string())?;
            window
                .set_always_on_top(true)
                .map_err(|error| error.to_string())?;
            window
                .set_visible_on_all_workspaces(true)
                .map_err(|error| error.to_string())?;
            window.set_focus().map_err(|error| error.to_string())?;
        }
    } else {
        let overlay_app = app.clone();
        let overlay_window = WebviewWindowBuilder::new(
            &app,
            SUBTITLE_OVERLAY_LABEL,
            WebviewUrl::App("overlay.html".into()),
        )
        .title("Atogaki 悬浮字幕")
        .inner_size(680.0, 170.0)
        .min_inner_size(360.0, 120.0)
        .resizable(true)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        show_subtitle_overlay_macos(&app, &overlay_window)?;
        #[cfg(not(target_os = "macos"))]
        overlay_window.show().map_err(|error| error.to_string())?;
        overlay_window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Ok(mut current) = overlay_app.state::<SubtitleOverlayState>().current.lock()
                {
                    *current = None;
                }
                let _ = overlay_app
                    .get_webview_window(SUBTITLE_OVERLAY_LABEL)
                    .map(|window| window.hide());
                let _ = overlay_app.emit_to("main", "subtitle-overlay-visibility", false);
            }
        });
    }

    let _ = app.emit_to("main", "subtitle-overlay-visibility", true);
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_subtitle_overlay_macos<R: tauri::Runtime>(
    app: &AppHandle<R>,
    window: &tauri::WebviewWindow<R>,
) -> Result<(), String> {
    use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
    use objc2_app_kit::{
        NSBackingStoreType, NSPanel, NSScreenSaverWindowLevel, NSWindow, NSWindowButton,
        NSWindowCollectionBehavior, NSWindowStyleMask,
    };
    use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo, NSSize, NSString};

    let app = app.clone();
    let window = window.clone();
    window
        .clone()
        .run_on_main_thread(move || {
            if let Some(panel_pointer) = app
                .state::<SubtitleOverlayState>()
                .native_panel
                .lock()
                .ok()
                .and_then(|panel| *panel)
            {
                let panel: &NSPanel = unsafe { &*(panel_pointer as *const NSPanel) };
                panel.orderFrontRegardless();
                return;
            }
            let Ok(native_window) = window.ns_window() else {
                return;
            };
            let native_window: &NSWindow = unsafe { &*native_window.cast() };
            let Some(content_view) = native_window.contentView() else {
                return;
            };
            let Some(main_thread) = MainThreadMarker::new() else {
                return;
            };
            let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
                NSPanel::alloc(main_thread),
                native_window.frame(),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Resizable
                    | NSWindowStyleMask::UtilityWindow
                    | NSWindowStyleMask::NonactivatingPanel,
                NSBackingStoreType::Buffered,
                false,
            );
            let mut behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary;
            let process_info = NSProcessInfo::processInfo();
            if process_info.isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
                majorVersion: 13,
                minorVersion: 0,
                patchVersion: 0,
            }) {
                behavior |= NSWindowCollectionBehavior::CanJoinAllApplications;
            }
            panel.setTitle(&NSString::from_str("Atogaki 悬浮字幕"));
            panel.setContentView(Some(&content_view));
            // The WebView was created for Tauri's hidden NSWindow, but its
            // window delegate must not be reused by this separately allocated
            // NSPanel. Tao's delegate assumes resize notifications come from
            // the original window and aborts when it receives this panel.
            for button in [
                NSWindowButton::CloseButton,
                NSWindowButton::MiniaturizeButton,
                NSWindowButton::ZoomButton,
            ] {
                if let Some(button) = panel.standardWindowButton(button) {
                    button.setHidden(true);
                }
            }
            panel.setMinSize(NSSize::new(360.0, 120.0));
            panel.setFloatingPanel(true);
            // Keep the panel non-activating when it is shown, but allow an
            // intentional click inside it to make its WebView key so the
            // playback shortcuts work without returning to the main window.
            panel.setBecomesKeyOnlyIfNeeded(false);
            panel.setHidesOnDeactivate(false);
            panel.setCollectionBehavior(behavior);
            panel.setLevel(NSScreenSaverWindowLevel);
            unsafe { panel.setReleasedWhenClosed(false) };
            native_window.orderOut(None);
            panel.orderFrontRegardless();

            let panel_pointer = Retained::into_raw(panel) as usize;
            if let Ok(mut stored) = app.state::<SubtitleOverlayState>().native_panel.lock() {
                *stored = Some(panel_pointer);
            }
        })
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn hide_subtitle_overlay_macos(app: &AppHandle) -> Result<(), String> {
    use objc2_app_kit::NSPanel;

    let panel_pointer = app
        .state::<SubtitleOverlayState>()
        .native_panel
        .lock()
        .map_err(|_| "subtitle overlay panel state is unavailable".to_string())?
        .to_owned();
    if let (Some(panel_pointer), Some(window)) = (
        panel_pointer,
        app.get_webview_window(SUBTITLE_OVERLAY_LABEL),
    ) {
        window
            .run_on_main_thread(move || {
                let panel: &NSPanel = unsafe { &*(panel_pointer as *const NSPanel) };
                panel.orderOut(None);
            })
            .map_err(|error| error.to_string())?;
    }
    app.set_activation_policy(tauri::ActivationPolicy::Regular)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn update_subtitle_overlay(
    app: AppHandle,
    state: State<'_, SubtitleOverlayState>,
    payload: SubtitleOverlayPayload,
) -> Result<(), String> {
    set_subtitle_overlay_payload(&app, &state, payload)
}

#[tauri::command]
fn current_subtitle_overlay(
    state: State<'_, SubtitleOverlayState>,
) -> Result<Option<SubtitleOverlayPayload>, String> {
    state
        .current
        .lock()
        .map(|current| current.clone())
        .map_err(|_| "subtitle overlay state is unavailable".to_string())
}

#[tauri::command]
fn hide_subtitle_overlay(
    app: AppHandle,
    state: State<'_, SubtitleOverlayState>,
) -> Result<(), String> {
    *state
        .current
        .lock()
        .map_err(|_| "subtitle overlay state is unavailable".to_string())? = None;
    #[cfg(target_os = "macos")]
    hide_subtitle_overlay_macos(&app)?;
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window(SUBTITLE_OVERLAY_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    let _ = app.emit_to("main", "subtitle-overlay-visibility", false);
    Ok(())
}

fn set_subtitle_overlay_payload(
    app: &AppHandle,
    state: &State<'_, SubtitleOverlayState>,
    payload: SubtitleOverlayPayload,
) -> Result<(), String> {
    *state
        .current
        .lock()
        .map_err(|_| "subtitle overlay state is unavailable".to_string())? = Some(payload.clone());
    if let Some(window) = app.get_webview_window(SUBTITLE_OVERLAY_LABEL) {
        window
            .emit("subtitle-overlay-update", payload)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
    start_ms: i64,
    end_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SplitSubtitleRequest {
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
struct MergeSubtitlesRequest {
    job_id: String,
    left_segment_id: String,
    right_segment_id: String,
    source_text: String,
    translated_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreSubtitleStructureRequest {
    job_id: String,
    before_segments: Vec<LocalSubtitleSegmentRecord>,
    after_segments: Vec<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSubtitleTimingRequest {
    job_id: String,
    before_segments: Vec<LocalSubtitleSegmentRecord>,
    after_segments: Vec<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WaveformWindowRequest {
    job_id: String,
    start_ms: i64,
    end_ms: i64,
    point_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleExportRequest {
    job_id: String,
    output_directory: String,
    overwrite_existing: bool,
    artifacts: Vec<LocalSubtitleExportArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleExportPlanRequest {
    job_id: String,
    output_directory: String,
    artifacts: Vec<LocalSubtitleExportArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoRenderRequest {
    source_job_id: String,
    output_path: String,
    subtitle_track: SubtitleTrack,
    overwrite_existing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavePlaybackPositionRequest {
    job_id: String,
    position_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveLearningSelectionRequest {
    job_id: String,
    segment_id: String,
    item_type: String,
    selection_start_utf16: i64,
    selection_end_utf16: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateLearningMeaningRequest {
    item_id: String,
    meaning_text: Option<String>,
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
    translation_runs: Vec<LocalTranslationRunRecord>,
    playback_path: Option<String>,
    audio_fallback_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct DesktopJobSummary {
    #[serde(flatten)]
    job: LocalJobRecord,
    translation_status: &'static str,
    segment_count: i64,
    translated_segment_count: i64,
    stale_translation_count: i64,
}

impl DesktopJobSummary {
    fn new(job: LocalJobRecord, stats: Option<LocalJobTranslationStats>) -> Self {
        let stats = stats.unwrap_or(LocalJobTranslationStats {
            job_id: job.job_id.clone(),
            segment_count: 0,
            translated_segment_count: 0,
            stale_translation_count: 0,
        });
        let translation_status = if stats.segment_count == 0 {
            "not_ready"
        } else if stats.stale_translation_count > 0 {
            "stale"
        } else if stats.translated_segment_count == 0 {
            "untranslated"
        } else if stats.translated_segment_count < stats.segment_count {
            "partial"
        } else {
            "translated"
        };
        Self {
            job,
            translation_status,
            segment_count: stats.segment_count,
            translated_segment_count: stats.translated_segment_count,
            stale_translation_count: stats.stale_translation_count,
        }
    }
}

#[tauri::command]
async fn list_jobs(state: State<'_, DesktopState>) -> Result<Vec<DesktopJobSummary>, String> {
    let jobs = state
        .task_service
        .list_persisted_jobs()
        .await
        .map_err(|error| error.to_string())?;
    let stats = state
        .task_service
        .list_persisted_job_translation_stats()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|stats| (stats.job_id.clone(), stats))
        .collect::<HashMap<_, _>>();
    Ok(jobs
        .into_iter()
        .map(|job| {
            let job_stats = stats.get(&job.job_id).cloned();
            DesktopJobSummary::new(job, job_stats)
        })
        .collect())
}

#[tauri::command]
async fn list_learning_items(
    state: State<'_, DesktopState>,
) -> Result<Vec<LocalLearningItemDetail>, String> {
    state
        .learning_service
        .list_items()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_learning_selection(
    state: State<'_, DesktopState>,
    request: SaveLearningSelectionRequest,
) -> Result<LocalLearningItemDetail, String> {
    state
        .learning_service
        .save_selection(NewLocalLearningSelection {
            job_id: request.job_id,
            segment_id: request.segment_id,
            item_type: request.item_type,
            selection_start_utf16: request.selection_start_utf16,
            selection_end_utf16: request.selection_end_utf16,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn update_learning_item_meaning(
    state: State<'_, DesktopState>,
    request: UpdateLearningMeaningRequest,
) -> Result<LocalLearningItemDetail, String> {
    state
        .learning_service
        .update_meaning(&request.item_id, request.meaning_text)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn delete_learning_item(
    state: State<'_, DesktopState>,
    item_id: String,
) -> Result<(), String> {
    state
        .learning_service
        .delete_item(&item_id)
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
async fn relink_job_media(
    app: AppHandle,
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Option<LocalJobRecord>, String> {
    let current = state
        .workspace_service
        .get_job(&job_id)
        .await
        .map_err(|error| error.to_string())?;
    let initial_directory = current
        .job
        .input_path
        .as_deref()
        .and_then(|path| Path::new(path).parent())
        .filter(|path| path.is_dir())
        .map(Path::to_path_buf);
    let selection = pick_local_file(
        &app,
        "重新定位原媒体",
        "媒体",
        &["mp3", "m4a", "wav", "mp4", "mkv", "webm", "mov"],
        initial_directory,
    )
    .await?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    state
        .task_service
        .relink_persisted_job_input(&job_id, selection)
        .await
        .map(Some)
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
async fn get_job_glossary_snapshot(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Option<String>, String> {
    let workspace = state
        .workspace_service
        .get_job(&job_id)
        .await
        .map_err(|error| error.to_string())?;
    let Some(path) = workspace.job.glossary_snapshot_path else {
        return Ok(None);
    };
    tokio::fs::read_to_string(&path)
        .await
        .map(Some)
        .map_err(|error| format!("failed to read task glossary snapshot {path}: {error}"))
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
        translation_runs: detail.translation_runs,
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
async fn restore_subtitle(
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
async fn split_subtitle(
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
async fn merge_subtitles(
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
async fn restore_subtitle_structure(
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
async fn save_subtitle_timing(
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
        .map_err(|error| format!("{error:#}"))
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
        .map_err(|error| format!("{error:#}"))
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
            &request.artifacts,
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
        .subtitle_export_plan(
            &request.job_id,
            &PathBuf::from(request.output_directory),
            &request.artifacts,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_playback_position(
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
async fn save_playback_position(
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
async fn get_waveform_window(
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
    reveal_in_file_manager(path, "导出文件")
}

#[tauri::command]
fn reveal_rendered_video(path: String) -> Result<(), String> {
    reveal_in_file_manager(path, "烧录视频")
}

fn reveal_in_file_manager(path: String, label: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err(format!("{label}不存在：{}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        let path = path
            .canonicalize()
            .map_err(|error| format!("无法定位{label}：{error}"))?;
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

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|error| format!("无法打开 Explorer：{error}"))?;
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err("当前平台尚不支持在文件管理器中定位导出文件。".to_string())
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
async fn check_translation_api_key(
    state: State<'_, DesktopState>,
    provider_id: String,
) -> Result<TranslationCredentialCheck, String> {
    state
        .settings_service
        .check_translation_api_key(&provider_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn dictionary_credential_statuses(
    state: State<'_, DesktopState>,
) -> Result<Vec<DictionaryCredentialStatus>, String> {
    state
        .settings_service
        .dictionary_credential_statuses()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn save_dictionary_credential(
    state: State<'_, DesktopState>,
    request: SaveDictionaryCredentialRequest,
) -> Result<DictionaryCredentialStatus, String> {
    state
        .settings_service
        .save_dictionary_credential(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn check_dictionary_credential(
    state: State<'_, DesktopState>,
    provider_id: String,
) -> Result<DictionaryCredentialStatus, String> {
    state
        .settings_service
        .check_dictionary_credential(&provider_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn lookup_learning_dictionary(
    state: State<'_, DesktopState>,
    item_id: String,
    provider_id: String,
) -> Result<LocalLearningItemDetail, String> {
    state
        .dictionary_lookup_service
        .lookup(&item_id, &provider_id)
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
fn dictionary_catalog(state: State<'_, DesktopState>) -> Vec<DictionaryCatalogItem> {
    state.dictionary_download_service.catalog()
}

#[tauri::command]
fn dictionary_directory(state: State<'_, DesktopState>) -> String {
    state.dictionary_download_service.directory()
}

#[tauri::command]
async fn start_dictionary_download(
    state: State<'_, DesktopState>,
    dictionary_id: String,
) -> Result<DictionaryDownloadState, String> {
    state
        .dictionary_download_service
        .start(&dictionary_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn dictionary_download_states(
    state: State<'_, DesktopState>,
) -> Result<Vec<DictionaryDownloadState>, String> {
    Ok(state.dictionary_download_service.states().await)
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
    let mut options =
        TranscriptionOptions::new(request.model_path.trim().into(), request.source_language);
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
                std::env::var("DEEPSEEK_API_KEY").ok(),
                configured_file("ATOGAKI_WHISPER_MODEL"),
                configured_file("ATOGAKI_VAD_MODEL"),
            );
            tauri::async_runtime::block_on(settings_service.initialize())?;
            let glossary_service = LocalGlossaryService::new(database.clone());
            tauri::async_runtime::block_on(glossary_service.ensure_builtins())?;
            let learning_service = LocalLearningService::new(database.clone());
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
            let dictionary_download_service = DictionaryDownloadService::new(
                data_dir.join("dictionaries"),
                settings_service.clone(),
            )?;
            let dictionary_lookup_service = dictionary_lookup::DictionaryLookupService::new(
                data_dir.join("dictionaries"),
                learning_service.clone(),
                settings_service.clone(),
            );
            app.manage(DesktopState {
                data_dir,
                task_service,
                workspace_service,
                glossary_service,
                learning_service,
                render_service,
                settings_service,
                model_download_service,
                dictionary_download_service,
                dictionary_lookup_service,
            });
            app.manage(SubtitleOverlayState::default());
            if let Some(main_window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                main_window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(overlay) = app_handle.get_webview_window(SUBTITLE_OVERLAY_LABEL)
                        {
                            let _ = overlay.destroy();
                        }
                        app_handle.exit(0);
                    }
                    #[cfg(target_os = "macos")]
                    WindowEvent::Focused(focused) => {
                        let overlay_visible = app_handle
                            .state::<SubtitleOverlayState>()
                            .current
                            .lock()
                            .is_ok_and(|current| current.is_some());
                        if overlay_visible {
                            let policy = if *focused {
                                tauri::ActivationPolicy::Regular
                            } else {
                                tauri::ActivationPolicy::Accessory
                            };
                            let _ = app_handle.set_activation_policy(policy);
                        }
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            check_dictionary_credential,
            check_translation_api_key,
            data_directory,
            desktop_settings,
            dictionary_catalog,
            dictionary_credential_statuses,
            dictionary_directory,
            dictionary_download_states,
            delete_job,
            delete_learning_item,
            delete_glossary,
            export_workspace_subtitles,
            get_glossary,
            get_job_glossary_snapshot,
            get_job_detail,
            get_playback_position,
            get_waveform_window,
            list_glossaries,
            list_jobs,
            list_learning_items,
            lookup_learning_dictionary,
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
            current_subtitle_overlay,
            hide_subtitle_overlay,
            preview_glossary_application,
            preview_glossary_prompt,
            preview_workspace_subtitle_export,
            recognition_defaults,
            reveal_exported_subtitle,
            reveal_rendered_video,
            relink_job_media,
            rename_job,
            retry_job,
            save_download_network_settings,
            save_dictionary_credential,
            save_desktop_settings,
            save_glossary,
            save_learning_selection,
            save_playback_position,
            submit_transcription,
            start_model_download,
            start_dictionary_download,
            test_network_connection,
            submit_video_render,
            open_subtitle_overlay,
            cancel_video_render,
            translate_all_subtitles,
            translate_subtitle,
            translation_status,
            update_subtitle_overlay,
            update_learning_item_meaning,
            update_subtitle,
            restore_subtitle,
            split_subtitle,
            merge_subtitles,
            restore_subtitle_structure,
            save_subtitle_timing
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
        DesktopJobSummary, SubmitTranscriptionRequest, desktop_transcription_options,
        validated_data_dir_override,
    };
    use atogaki_subtitle::{
        domain::LanguageCode,
        infrastructure::local_db::{LocalJobRecord, LocalJobTranslationStats},
    };

    fn test_job() -> LocalJobRecord {
        LocalJobRecord {
            job_id: "job-1".to_string(),
            display_name: None,
            storage_dir: "/tmp/job-1".to_string(),
            input_path: None,
            render_output_path: None,
            status: "done".to_string(),
            message: "done".to_string(),
            error_message: None,
            glossary_id: None,
            glossary_name: None,
            glossary_snapshot_path: None,
            source_language: "ja".to_string(),
            target_language: "zh-Hans".to_string(),
            created_at_unix: 1,
            started_at_unix: Some(1),
            completed_at_unix: Some(1),
            updated_at_unix: 1,
        }
    }

    fn translation_stats(
        segment_count: i64,
        translated_segment_count: i64,
        stale_translation_count: i64,
    ) -> LocalJobTranslationStats {
        LocalJobTranslationStats {
            job_id: "job-1".to_string(),
            segment_count,
            translated_segment_count,
            stale_translation_count,
        }
    }

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

    #[test]
    fn desktop_job_summary_distinguishes_translation_coverage() {
        assert_eq!(
            DesktopJobSummary::new(test_job(), Some(translation_stats(0, 0, 0))).translation_status,
            "not_ready"
        );
        assert_eq!(
            DesktopJobSummary::new(test_job(), Some(translation_stats(3, 0, 0))).translation_status,
            "untranslated"
        );
        assert_eq!(
            DesktopJobSummary::new(test_job(), Some(translation_stats(3, 2, 0))).translation_status,
            "partial"
        );
        assert_eq!(
            DesktopJobSummary::new(test_job(), Some(translation_stats(3, 3, 0))).translation_status,
            "translated"
        );
        assert_eq!(
            DesktopJobSummary::new(test_job(), Some(translation_stats(3, 3, 1))).translation_status,
            "stale"
        );
    }
}
