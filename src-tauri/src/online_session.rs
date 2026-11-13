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
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub struct DesktopOnlineSession {
    pub service: OnlineSessionService,
    pub data_dir: PathBuf,
    events: Arc<Mutex<Vec<CaptionEvent>>>,
    pub caption_locked: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    panel: Arc<Mutex<Option<usize>>>,
}
impl DesktopOnlineSession {
    pub fn new(service: OnlineSessionService, data_dir: PathBuf) -> Self {
        Self {
            service,
            data_dir,
            events: Arc::default(),
            caption_locked: Arc::default(),
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
    translators: Vec<Choice>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionEvent {
    sequence: u64,
    #[serde(flatten)]
    data: OnlineSessionEvent,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    status: OnlineSessionStatus,
    events: Vec<CaptionEvent>,
    caption_locked: bool,
    caption_visible: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    language: String,
    final_model: String,
    translate: bool,
    translation_drafts: bool,
    #[serde(default = "default_translation_backend")]
    translation_backend: String,
    #[serde(default)]
    scene: String,
    glossary_id: Option<String>,
}
fn default_translation_backend() -> String {
    "hy-mt2".into()
}

async fn index_model(
    desktop: &crate::DesktopState,
    state: &DesktopOnlineSession,
) -> Result<PathBuf, String> {
    Ok(desktop
        .settings_service
        .downloaded_model_path("index-translate")
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| state.data_dir.join("models/Index-Translate-2B.Q4_K_M.gguf")))
}
fn assets(state: &DesktopOnlineSession) -> Result<Assets, String> {
    let models = state.data_dir.join("models");
    let manifest = models.join("online/manifest.json");
    let mut assets = match std::fs::read(&manifest) {
        Ok(bytes) => serde_json::from_slice::<Assets>(&bytes).map_err(|e| e.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Assets {
            drafts: HashMap::new(),
            finals: vec![],
            vad: models.join("online/silero_vad.onnx"),
            hy_mt2: None,
        },
        Err(error) => return Err(error.to_string()),
    };
    // Managed packages augment the local preview manifest. Partial downloads are never marked ready.
    for item in crate::model_download::model_catalog() {
        let path = models.join(&item.file_name);
        let model = Model {
            id: item.id.clone(),
            name: item.name.clone(),
            path,
        };
        if item.kind == "qwen-asr" {
            if let Some(existing) = assets.finals.iter_mut().find(|m| m.id == item.id) {
                if item.installed(&models) {
                    *existing = model;
                }
            } else {
                assets.finals.push(model);
            }
        } else if item.category == "draft" {
            let lang = item.id.trim_start_matches("draft-");
            if item.installed(&models) || !assets.drafts.contains_key(lang) {
                assets.drafts.insert(
                    lang.into(),
                    Model {
                        id: lang.into(),
                        ..model
                    },
                );
            }
        }
    }
    Ok(assets)
}
fn model_ready(model: &Model, catalog_id: &str) -> bool {
    crate::model_download::model_catalog()
        .iter()
        .find(|m| m.id == catalog_id)
        .map_or_else(
            || model.path.is_dir(),
            |item| item.installed_at(&model.path),
        )
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
pub async fn online_choices(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
    desktop: State<'_, crate::DesktopState>,
) -> Result<Choices, String> {
    let mut a = assets(&state)?;
    a.hy_mt2 = desktop
        .settings_service
        .downloaded_model_path("hy-mt2")
        .await
        .map_err(|e| e.to_string())?
        .or(a.hy_mt2);
    let r = runtime(&app, &state)?;
    let translators = vec![
        Choice {
            id: "hy-mt2".into(),
            name: "Hy-MT2 1.8B".into(),
            ready: a.hy_mt2.as_ref().is_some_and(|p| p.is_file())
                && desktop_llama_server_path().is_file(),
        },
        Choice {
            id: "index-translate".into(),
            name: "Index-Translate 2B Q4".into(),
            ready: index_model(&desktop, &state).await?.is_file()
                && desktop_llama_server_path().is_file(),
        },
    ];
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
            ready: a
                .drafts
                .get(id)
                .is_some_and(|m| model_ready(m, &format!("draft-{id}"))),
        })
        .collect(),
        finals: a
            .finals
            .into_iter()
            .map(|m| Choice {
                ready: model_ready(&m, &m.id),
                id: m.id,
                name: m.name,
            })
            .collect(),
        runtime_ready: cfg!(all(target_os = "macos", target_arch = "aarch64"))
            && r.python.is_file()
            && r.capture.is_file(),
        translation_ready: translators.iter().any(|m| m.ready),
        translators,
    })
}
#[tauri::command]
pub fn online_session_status(state: State<'_, DesktopOnlineSession>) -> OnlineSessionStatus {
    state.service.status()
}
#[tauri::command]
pub async fn online_snapshot(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
) -> Result<Snapshot, String> {
    #[cfg(target_os = "macos")]
    let caption_visible = {
        let pointer = *state.panel.lock().map_err(|e| e.to_string())?;
        if let Some(pointer) = pointer {
            let (send, receive) = tokio::sync::oneshot::channel();
            app.run_on_main_thread(move || {
                let panel: &objc2_app_kit::NSPanel =
                    unsafe { &*(pointer as *const objc2_app_kit::NSPanel) };
                let _ = send.send(panel.isVisible());
            })
            .map_err(|e| e.to_string())?;
            receive.await.map_err(|e| e.to_string())?
        } else {
            false
        }
    };
    #[cfg(not(target_os = "macos"))]
    let caption_visible = app
        .get_webview_window("online-captions")
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    Ok(Snapshot {
        status: state.service.status(),
        events: state.events.lock().map_err(|e| e.to_string())?.clone(),
        caption_locked: state.caption_locked.load(Ordering::Relaxed),
        caption_visible,
    })
}
#[tauri::command]
pub async fn start_online_session(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
    desktop: State<'_, crate::DesktopState>,
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
    let mut a = assets(&state)?;
    a.hy_mt2 = desktop
        .settings_service
        .downloaded_model_path("hy-mt2")
        .await
        .map_err(|e| e.to_string())?
        .or(a.hy_mt2);
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
    if !model_ready(draft, &format!("draft-{}", options.language))
        || !model_ready(final_model, &final_model.id)
    {
        return Err("所选识别模型包不完整，请先在模型与存储中准备模型。".into());
    }
    if options.scene.chars().count() > 1000 {
        return Err("场景提示最多1000字符".into());
    }
    let terms = if options.translate
        && let Some(id) = options.glossary_id.as_deref()
    {
        desktop
            .glossary_service
            .online_translation_terms(id, &options.language)
            .await
            .map_err(|e| e.to_string())?
    } else {
        vec![]
    };
    let mut translation_context =
        serde_json::json!({"scene":options.scene.trim(), "protected_terms":terms});
    if options.translate {
        let mut config = match options.translation_backend.as_str() {
            "hy-mt2" => LocalLlamaConfig::hy_mt2(
                desktop_llama_server_path(),
                a.hy_mt2.ok_or("未安装Hy-MT2翻译模型")?,
                String::new(),
            ),
            "index-translate" => LocalLlamaConfig::index_translate(
                desktop_llama_server_path(),
                index_model(&desktop, &state).await?,
                String::new(),
            ),
            _ => return Err("未知本地翻译后端".into()),
        };
        translation_context["backend"] = serde_json::json!(options.translation_backend);
        translation_context["model"] = serde_json::json!(config.model_id);
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
        translation_context,
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
                    // Restore paired originals and latest MT revisions when a
                    // new caption window opens, including an older MT draft.
                    // These are memory caps, not visible-history item counts.
                    let recent = history.len().saturating_sub(256);
                    let mut source_finals = 0;
                    let mut translation_groups = HashSet::new();
                    let mut keep = vec![false; history.len()];
                    for (index, item) in history.iter().enumerate().rev() {
                        let settled = match item.data.event["kind"].as_str() {
                            Some("source_final") => {
                                source_finals += 1;
                                source_finals <= 64
                            }
                            Some(
                                "translation_final" | "translation_draft" | "translation_failed",
                            ) => {
                                let id = item.data.event["group_id"].as_str().unwrap_or("");
                                translation_groups.len() < 64
                                    && translation_groups.insert(id.to_string())
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
    focus: Option<bool>,
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
        .inner_size(920., 280.)
        .transparent(true)
        .background_color(tauri::window::Color(0, 0, 0, 0))
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
        crate::show_caption_panel(
            &app,
            &w,
            state.panel.clone(),
            "Atogaki 在线字幕",
            focus.unwrap_or(false),
        )?;
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

#[tauri::command]
pub fn toggle_online_captions(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(pointer) = *state.panel.lock().map_err(|e| e.to_string())? {
        let locked = state.caption_locked.clone();
        return app
            .run_on_main_thread(move || {
                let panel: &objc2_app_kit::NSPanel =
                    unsafe { &*(pointer as *const objc2_app_kit::NSPanel) };
                if panel.isVisible() {
                    panel.orderOut(None);
                } else if locked.load(Ordering::Relaxed) {
                    panel.orderFrontRegardless();
                } else {
                    panel.makeKeyAndOrderFront(None);
                }
            })
            .map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "macos"))]
    if app
        .get_webview_window("online-captions")
        .is_some_and(|w| w.is_visible().unwrap_or(false))
    {
        return hide_online_captions(app, state);
    }
    open_online_captions(app, state, Some(true))
}

#[tauri::command]
pub fn set_online_caption_locked(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
    locked: bool,
) -> Result<(), String> {
    state.caption_locked.store(locked, Ordering::Relaxed);
    #[cfg(target_os = "macos")]
    if let Some(window) = app.get_webview_window("online-captions") {
        let panel = state.panel.clone();
        let current = state.caption_locked.clone();
        window
            .run_on_main_thread(move || {
                if let Some(pointer) = panel.lock().ok().and_then(|p| *p) {
                    let panel: &objc2_app_kit::NSPanel =
                        unsafe { &*(pointer as *const objc2_app_kit::NSPanel) };
                    panel.setIgnoresMouseEvents(current.load(Ordering::Relaxed));
                }
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use atogaki_subtitle::application::stage_scheduler::StageScheduler;

    #[test]
    fn model_choices_exist_without_a_private_manifest_and_reject_incomplete_packages() {
        let root =
            std::env::temp_dir().join(format!("atogaki-online-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let state = DesktopOnlineSession::new(
            OnlineSessionService::new(StageScheduler::new(2)),
            root.clone(),
        );
        let available = assets(&state).unwrap();
        assert_eq!(available.drafts.len(), 4);
        assert_eq!(available.finals.len(), 4);
        assert!(available.finals.iter().all(|m| !model_ready(m, &m.id)));
        let candidate = available
            .finals
            .iter()
            .find(|m| m.id == "qwen-0.6b-4bit")
            .unwrap();
        std::fs::create_dir_all(&candidate.path).unwrap();
        std::fs::write(candidate.path.join("config.json"), b"partial config").unwrap();
        assert!(!model_ready(candidate, &candidate.id));
        // A valid legacy manifest still supplies its paths; catalog variants augment it once.
        let legacy = serde_json::json!({"drafts": {}, "finals": [{"id":"qwen-0.6b", "name":"Existing", "path":root.join("legacy-qwen")}], "vad":root.join("legacy-vad.onnx"), "hyMt2": null});
        std::fs::write(root.join("models/online/manifest.json"), legacy.to_string()).unwrap();
        let merged = assets(&state).unwrap();
        assert_eq!(merged.finals.len(), 4);
        assert_eq!(
            merged
                .finals
                .iter()
                .find(|m| m.id == "qwen-0.6b")
                .unwrap()
                .path,
            root.join("legacy-qwen")
        );
        assert_eq!(merged.vad, root.join("legacy-vad.onnx"));
        std::fs::write(root.join("models/online/manifest.json"), b"invalid JSON").unwrap();
        assert!(assets(&state).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
