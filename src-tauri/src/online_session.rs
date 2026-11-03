//! Desktop IPC for the session owner; caption windows only subscribe to events.
use std::path::PathBuf;
use std::sync::Arc;

use atogaki_subtitle::application::online_session::{
    OnlineModels, OnlineRuntime, OnlineSessionService, OnlineSessionStatus,
};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct DesktopOnlineSession {
    pub service: OnlineSessionService,
    pub data_dir: PathBuf,
}

#[tauri::command]
pub fn online_session_status(state: State<'_, DesktopOnlineSession>) -> OnlineSessionStatus {
    state.service.status()
}

#[tauri::command]
pub async fn start_online_session(
    app: AppHandle,
    state: State<'_, DesktopOnlineSession>,
    models: OnlineModels,
) -> Result<OnlineSessionStatus, String> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err("在线系统音频首版仅支持 macOS Apple Silicon 14.2+".into());
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSOperatingSystemVersion, NSProcessInfo};
        if !NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
            majorVersion: 14,
            minorVersion: 2,
            patchVersion: 0,
        }) {
            return Err("系统音频采集需要 macOS 14.2 或更高版本".into());
        }
    }
    let directory = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法定位应用目录")?
        .to_path_buf();
    let runtime = OnlineRuntime {
        python: state.data_dir.join("runtimes/online/bin/python3"),
        worker_root: app.path().resource_dir().map_err(|e| e.to_string())?,
        capture: directory.join("atogaki-audio-tap"),
    };
    state
        .service
        .start(
            runtime,
            models,
            Arc::new(move |event| {
                let _ = app.emit("online-session-event", event);
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
