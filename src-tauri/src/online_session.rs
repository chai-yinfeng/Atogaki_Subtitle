//! Desktop controls and independent live captions; no transcript/audio persistence.
use atogaki_subtitle::{
    application::online_session::{
        OnlineModels, OnlineRuntime, OnlineSessionEvent, OnlineSessionService, OnlineSessionStatus,
    },
    infrastructure::{
        config::desktop_llama_server_path,
        local_llama::{LocalLlamaConfig, LocalLlamaTranslationProvider},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub struct DesktopOnlineSession {
    pub service: OnlineSessionService,
    pub data_dir: PathBuf,
    events: Arc<Mutex<Vec<CaptionEvent>>>,
    #[cfg(target_os = "macos")]
    panel: Arc<Mutex<Option<usize>>>,
}
impl DesktopOnlineSession {
    pub fn new(service: OnlineSessionService, data_dir: PathBuf) -> Self {
        Self {
            service,
            data_dir,
            events: Arc::default(),
            #[cfg(target_os = "macos")]
            panel: Arc::default(),
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Model {
    id: String,
    name: String,
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Assets {
    drafts: HashMap<String, Model>,
    finals: Vec<Model>,
    vad: PathBuf,
    hy_mt2: Option<PathBuf>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    id: String,
    name: String,
    ready: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    languages: Vec<Choice>,
    finals: Vec<Choice>,
    runtime_ready: bool,
    translation_ready: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionEvent {
    sequence: u64,
    #[serde(flatten)]
    data: OnlineSessionEvent,
}
#[derive(Serialize)]
pub struct Snapshot {
    status: OnlineSessionStatus,
    events: Vec<CaptionEvent>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    language: String,
    final_model: String,
    translate: bool,
    translation_drafts: bool,
}
fn assets(state: &DesktopOnlineSession) -> Result<Assets, String> {
    let bytes = std::fs::read(state.data_dir.join("models/online/manifest.json"))
        .map_err(|_| "在线模型尚未准备好，请先准备本机测试模型包。".to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn runtime(app: &AppHandle, state: &DesktopOnlineSession) -> Result<OnlineRuntime, String> {
    let directory = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法定位应用目录")?
        .to_path_buf();
    Ok(OnlineRuntime {
        python: state.data_dir.join("runtimes/online/bin/python3"),
        worker_root: app.path().resource_dir().map_err(|e| e.to_string())?,
        capture: directory.join("atogaki-audio-tap"),
        translator: None,
    })
}
#[tauri::command]
pub fn online_choices(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
) -> Result<Choices, String> {
    let a = assets(&state)?;
    let r = runtime(&app, &state)?;
    Ok(Choices {
        languages: [
            ("ja", "日语 · SenseVoice"),
            ("en", "英语 · Zipformer"),
            ("zh", "中文 · Zipformer"),
            ("ko", "韩语 · Zipformer"),
        ]
        .into_iter()
        .map(|(id, name)| Choice {
            id: id.into(),
            name: name.into(),
            ready: a.drafts.get(id).is_some_and(|m| m.path.is_dir()),
        })
        .collect(),
        finals: a
            .finals
            .into_iter()
            .map(|m| Choice {
                ready: m.path.is_dir(),
                id: m.id,
                name: m.name,
            })
            .collect(),
        runtime_ready: cfg!(all(target_os = "macos", target_arch = "aarch64"))
            && r.python.is_file()
            && r.capture.is_file(),
        translation_ready: a.hy_mt2.is_some_and(|p| p.is_file())
            && desktop_llama_server_path().is_file(),
    })
}
#[tauri::command]
pub fn online_session_status(state: State<'_, DesktopOnlineSession>) -> OnlineSessionStatus {
    state.service.status()
}
#[tauri::command]
pub fn online_snapshot(state: State<'_, DesktopOnlineSession>) -> Result<Snapshot, String> {
    Ok(Snapshot {
        status: state.service.status(),
        events: state.events.lock().map_err(|e| e.to_string())?.clone(),
    })
}
#[tauri::command]
pub async fn start_online_session(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
    options: Start,
) -> Result<OnlineSessionStatus, String> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err("在线首版仅支持 macOS Apple Silicon 14.2+".into());
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo};
        if !NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
            majorVersion: 14,
            minorVersion: 2,
            patchVersion: 0,
        }) {
            return Err("系统音频采集需要 macOS 14.2+".into());
        }
    }
    let a = assets(&state)?;
    let mut r = runtime(&app, &state)?;
    let draft = a
        .drafts
        .get(&options.language)
        .ok_or("没有该语言的草稿模型")?;
    let final_model = a
        .finals
        .iter()
        .find(|m| m.id == options.final_model)
        .ok_or("未安装所选定稿模型")?;
    if options.translate {
        let mut config = LocalLlamaConfig::hy_mt2(
            desktop_llama_server_path(),
            a.hy_mt2.ok_or("未安装本地翻译模型")?,
            String::new(),
        );
        config.parallel_slots = Some(1);
        r.translator = Some(LocalLlamaTranslationProvider::new(config).map_err(|e| e.to_string())?);
    }
    let models = OnlineModels {
        language: options.language,
        draft_model: draft.path.clone(),
        final_model: final_model.path.clone(),
        vad_model: a.vad,
        translation_url: None,
        translation_drafts: options.translation_drafts,
    };
    let events = state.events.clone();
    state
        .service
        .start(
            r,
            models,
            Arc::new(move |event| {
                if let Ok(mut history) = events.lock() {
                    if history
                        .last()
                        .is_some_and(|e| e.data.session_id != event.session_id)
                    {
                        history.clear();
                    }
                    let sequence = history.last().map_or(1, |e| e.sequence + 1);
                    let event = CaptionEvent {
                        sequence,
                        data: event,
                    };
                    history.push(event.clone());
                    // Preserve a few settled captions when a long draft fills the ring,
                    // so a newly opened window can recover the same reading history.
                    let recent = history.len().saturating_sub(256);
                    let mut source_finals = 0;
                    let mut translation_finals = 0;
                    let mut keep = vec![false; history.len()];
                    for (index, item) in history.iter().enumerate().rev() {
                        let settled = match item.data.event["kind"].as_str() {
                            Some("source_final") => {
                                source_finals += 1;
                                source_finals <= 3
                            }
                            Some("translation_final") => {
                                translation_finals += 1;
                                translation_finals <= 3
                            }
                            _ => false,
                        };
                        keep[index] = index >= recent || settled;
                    }
                    let mut index = 0;
                    history.retain(|_| {
                        let retained = keep[index];
                        index += 1;
                        retained
                    });
                    let _ = app.emit("online-session-event", event);
                }
            }),
        )
        .await
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub async fn stop_online_session(
    state: State<'_, DesktopOnlineSession>,
) -> Result<OnlineSessionStatus, String> {
    state.service.stop().await.map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub fn open_online_control(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("online-control") {
        w.show().map_err(|e| e.to_string())?;
        return w.set_focus().map_err(|e| e.to_string());
    }
    let w = WebviewWindowBuilder::new(
        &app,
        "online-control",
        WebviewUrl::App("online.html".into()),
    )
    .title("Atogaki · 在线字幕控制面板")
    .accept_first_mouse(true)
    .inner_size(780., 680.)
    .min_inner_size(650., 500.)
    .build()
    .map_err(|e| e.to_string())?;
    let window = w.clone();
    w.on_window_event(move |e| {
        if let WindowEvent::CloseRequested { api, .. } = e {
            api.prevent_close();
            let _ = window.hide();
        }
    });
    Ok(())
}
#[tauri::command]
pub fn open_online_captions(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
) -> Result<(), String> {
    let w = if let Some(w) = app.get_webview_window("online-captions") {
        w
    } else {
        WebviewWindowBuilder::new(
            &app,
            "online-captions",
            WebviewUrl::App("online-captions.html".into()),
        )
        .title("Atogaki · 在线字幕")
        .accept_first_mouse(true)
        .inner_size(760., 560.)
        .min_inner_size(420., 200.)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?
    };
    #[cfg(target_os = "macos")]
    {
        crate::show_caption_panel(&app, &w, state.panel.clone(), "Atogaki 在线字幕")?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        w.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn hide_online_captions(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(pointer) = *state.panel.lock().map_err(|e| e.to_string())? {
        if let Some(w) = app.get_webview_window("online-captions") {
            w.run_on_main_thread(move || {
                let panel: &objc2_app_kit::NSPanel =
                    unsafe { &*(pointer as *const objc2_app_kit::NSPanel) };
                panel.orderOut(None);
            })
            .map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("online-captions") {
        let _ = w.hide();
    }
    Ok(())
}
