use crate::{DesktopState, model_download::model_catalog};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

pub const ALIGNER_ID: &str = "qwen-forced-aligner-0.6b";
pub fn aligner_path(models: &Path) -> Option<PathBuf> {
    model_catalog()
        .iter()
        .find(|m| m.id == ALIGNER_ID && m.installed(models))
        .map(|m| models.join(&m.file_name))
}
pub fn alignment_runtime_ready(models: &Path) -> bool {
    models.parent().is_some_and(|data| {
        data.join("runtimes/online/bin/python3").is_file()
            && data
                .join("runtimes/qwen-alignment/.atogaki-alignment.json")
                .is_file()
    })
}
pub fn qwen_ready(models: &Path) -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
        && alignment_runtime_ready(models)
        && aligner_path(models).is_some()
        && model_catalog()
            .iter()
            .any(|m| m.kind == "qwen-asr" && m.installed(models))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineModelChoice {
    id: String,
    name: String,
    path: String,
    provider: String,
    ready: bool,
    reason: String,
}
#[tauri::command]
pub async fn offline_recognition_choices(
    state: State<'_, DesktopState>,
) -> Result<Vec<OfflineModelChoice>, String> {
    let settings = state
        .settings_service
        .load()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let models = PathBuf::from(settings.models_directory);
    let runtime = alignment_runtime_ready(&models);
    let aligned = aligner_path(&models).is_some();
    let mut choices = vec![];
    if let Some(path) = settings
        .whisper_model_path
        .filter(|p| Path::new(p).is_file())
    {
        choices.push(OfflineModelChoice {
            id: "configured-whisper".into(),
            name: model_catalog()
                .iter()
                .find(|m| m.kind == "whisper" && models.join(&m.file_name) == Path::new(&path))
                .map(|m| m.name.clone())
                .unwrap_or_else(|| {
                    format!(
                        "Whisper · {}",
                        Path::new(&path)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                    )
                }),
            path,
            provider: "whisper.cpp".into(),
            ready: true,
            reason: String::new(),
        });
    }
    for model in state
        .custom_models
        .list()
        .await
        .map_err(|e| format!("{e:#}"))?
    {
        if model.kind == "vad" {
            continue;
        }
        let qwen = model.kind == "qwen-asr";
        let ready = crate::custom_models::validate(&model.kind, Path::new(&model.path)).is_ok()
            && (!qwen
                || cfg!(all(target_os = "macos", target_arch = "aarch64")) && runtime && aligned);
        let choice = OfflineModelChoice {
            id: model.id,
            name: model.name,
            path: model.path,
            provider: if qwen { "mlx-qwen3-asr" } else { "whisper.cpp" }.into(),
            ready,
            reason: if ready {
                String::new()
            } else {
                "模型、运行环境或对齐依赖未就绪".into()
            },
        };
        if let Some(existing) = choices.iter_mut().find(|m| m.path == choice.path) {
            *existing = choice;
        } else {
            choices.push(choice);
        }
    }
    for model in model_catalog()
        .iter()
        .filter(|m| matches!(m.kind.as_str(), "whisper" | "qwen-asr") && m.installed(&models))
    {
        let path = models.join(&model.file_name).display().to_string();
        if choices.iter().any(|m| m.path == path) {
            continue;
        }
        let qwen = model.kind == "qwen-asr";
        let ready =
            !qwen || cfg!(all(target_os = "macos", target_arch = "aarch64")) && runtime && aligned;
        let reason = if ready {
            ""
        } else if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "需要 Apple Silicon"
        } else if !runtime {
            "需准备本地运行环境"
        } else {
            "需下载字幕对齐模型"
        };
        choices.push(OfflineModelChoice {
            id: model.id.clone(),
            name: model.name.clone(),
            path,
            provider: if qwen { "mlx-qwen3-asr" } else { "whisper.cpp" }.into(),
            ready,
            reason: reason.into(),
        });
    }
    Ok(choices)
}
