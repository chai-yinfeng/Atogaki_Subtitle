use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASE: &str = "https://github.com/chai-yinfeng/Atogaki_Subtitle/releases";
const PUBLIC_KEY: Option<&str> = option_env!("ATOGAKI_UPDATER_PUBLIC_KEY");

#[derive(Default)]
pub struct DesktopUpdate(tokio::sync::Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    available_version: Option<String>,
    enabled: bool,
}

fn public_key() -> Option<&'static str> {
    PUBLIC_KEY.map(str::trim).filter(|key| !key.is_empty())
}

fn require_main(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("请在主窗口检查或安装更新。".into());
    }
    Ok(())
}

fn release_asset(url: &str, version: &str) -> bool {
    url == format!("{RELEASE}/download/v{version}/Atogaki-v{version}-macos-arm64.app.tar.gz")
}

#[tauri::command]
pub async fn check_desktop_update(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopUpdate>,
) -> Result<UpdateInfo, String> {
    require_main(&window)?;
    let mut pending = state.0.try_lock().map_err(|_| "正在处理更新。")?;
    *pending = None;
    let mut info = UpdateInfo {
        current_version: app.package_info().version.to_string(),
        available_version: None,
        enabled: cfg!(target_arch = "aarch64") && public_key().is_some(),
    };
    let Some(key) = public_key().filter(|_| info.enabled) else {
        return Ok(info);
    };
    let endpoint = format!("{RELEASE}/latest/download/latest.json")
        .parse()
        .map_err(|_| "更新地址无效。")?;
    let update = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?
        .version_comparator(|current, remote| {
            remote.version.pre.is_empty() && remote.version > current
        })
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    if let Some(update) = update {
        if !release_asset(update.download_url.as_str(), &update.version) {
            return Err("更新清单没有指向 Atogaki 的对应正式发行包。".into());
        }
        info.available_version = Some(update.version.clone());
        *pending = Some(update);
    }
    Ok(info)
}

async fn require_idle(app: &AppHandle) -> Result<(), String> {
    let online = app.state::<crate::online_session::DesktopOnlineSession>();
    if matches!(
        online.service.status().phase.as_str(),
        "loading" | "requesting_audio" | "running" | "stopping" | "draining"
    ) {
        return Err("请先停止在线字幕。".into());
    }
    let desktop = app.state::<crate::DesktopState>();
    let jobs = desktop
        .task_service
        .list_persisted_jobs()
        .await
        .map_err(|error| error.to_string())?;
    for job in jobs {
        if !matches!(job.status.as_str(), "done" | "failed") {
            return Err("请等待离线任务停止后再安装更新。".into());
        }
        let renders = desktop
            .render_service
            .list(&job.job_id)
            .await
            .map_err(|error| error.to_string())?;
        if renders
            .iter()
            .any(|render| matches!(render.status.as_str(), "queued" | "running"))
        {
            return Err("请等待视频导出结束后再安装更新。".into());
        }
    }
    let models = desktop
        .model_download_service
        .states()
        .await
        .map_err(|error| error.to_string())?;
    let dictionaries = desktop.dictionary_download_service.states().await;
    if models.iter().any(|model| model.active)
        || dictionaries.iter().any(|dictionary| {
            matches!(
                dictionary.status.as_str(),
                "queued" | "resolving" | "downloading"
            )
        })
    {
        return Err("请等待模型和词典下载结束后再安装更新。".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn install_desktop_update(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, DesktopUpdate>,
    version: String,
    on_progress: Channel<u64>,
) -> Result<(), String> {
    require_main(&window)?;
    let mut pending = state.0.try_lock().map_err(|_| "正在处理更新。")?;
    let mut update = pending
        .as_ref()
        .filter(|update| update.version == version)
        .cloned()
        .ok_or("请重新检查更新。")?;
    require_idle(&app).await?;
    update.timeout = Some(Duration::from_secs(600));
    let mut reported_mib = 0;
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            |chunk, _| {
                downloaded += chunk as u64;
                let mib = downloaded / (1024 * 1024);
                if mib > reported_mib {
                    reported_mib = mib;
                    let _ = on_progress.send(mib);
                }
            },
            || {},
        )
        .await
        .map_err(|error| error.to_string())?;
    require_idle(&app).await?;
    tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    *pending = None;
    app.request_restart();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_matching_project_release_archive() {
        let url = format!("{RELEASE}/download/v0.2.1/Atogaki-v0.2.1-macos-arm64.app.tar.gz");
        assert!(release_asset(&url, "0.2.1"));
        assert!(!release_asset(&url, "0.2.2"));
        assert!(!release_asset(
            &url.replace("github.com", "example.com"),
            "0.2.1"
        ));
        assert!(!release_asset(&url.replace(".app.tar.gz", ".dmg"), "0.2.1"));
    }
}
