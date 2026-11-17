use anyhow::{Result, bail};
use atogaki_subtitle::infrastructure::local_db::LocalDatabase;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::Mutex;

const KEY: &str = "recognition.registered_models";
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredModel {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    model: RegisteredModel,
    ready: bool,
}
pub struct CustomModels {
    database: LocalDatabase,
    mutation: Mutex<()>,
}
pub async fn read(database: &LocalDatabase) -> Result<Vec<RegisteredModel>> {
    database
        .get_setting(KEY)
        .await?
        .map(|value| serde_json::from_str(&value).map_err(Into::into))
        .unwrap_or_else(|| Ok(vec![]))
}
fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 64 || value.chars().any(char::is_control) {
        bail!("模型名称须为 1–64 字，不能包含控制字符");
    }
    Ok(value.into())
}
pub fn validate(kind: &str, path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("请使用完整的本机模型路径");
    }
    match kind {
        "whisper" | "vad" => {
            if path.extension().and_then(|s| s.to_str()) != Some("bin")
                || !path.is_file()
                || path.metadata()?.len() == 0
            {
                bail!("请选择非空的 Whisper／Silero GGML .bin 文件");
            }
        }
        "qwen-asr" => {
            let config = path.join("config.json");
            if !path.is_dir() || !config.is_file() || config.metadata()?.len() > 1_048_576 {
                bail!("请选择完整的 Qwen3-ASR 模型目录");
            }
            let value: serde_json::Value = serde_json::from_slice(&std::fs::read(config)?)?;
            if value["model_type"] != "qwen3_asr"
                || !path.join("preprocessor_config.json").is_file()
                || !path.join("tokenizer_config.json").is_file()
                || !(path.join("tokenizer.json").is_file()
                    || path.join("vocab.json").is_file() && path.join("merges.txt").is_file())
            {
                bail!("Qwen3-ASR 配置或 tokenizer 文件不完整");
            }
            if !std::fs::read_dir(path)?
                .filter_map(Result::ok)
                .any(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext == "safetensors")
                        && entry
                            .path()
                            .metadata()
                            .is_ok_and(|m| m.is_file() && m.len() > 0)
                })
            {
                bail!("Qwen3-ASR 目录缺少 safetensors 权重");
            }
        }
        _ => bail!("不支持该自定义模型类型"),
    }
    Ok(())
}
impl CustomModels {
    pub fn new(database: LocalDatabase) -> Self {
        Self {
            database,
            mutation: Mutex::new(()),
        }
    }
    pub async fn list(&self) -> Result<Vec<RegisteredModel>> {
        read(&self.database).await
    }
    async fn save(&self, models: &[RegisteredModel]) -> Result<()> {
        self.database
            .set_setting(KEY, &serde_json::to_string(models)?)
            .await
    }
    pub async fn register(
        &self,
        kind: &str,
        display_name: &str,
        path: &Path,
    ) -> Result<RegisteredModel> {
        let display_name = name(display_name)?;
        if !path.is_absolute() {
            bail!("请使用完整的本机模型路径");
        }
        let path = path.canonicalize()?;
        validate(kind, &path)?;
        let _guard = self.mutation.lock().await;
        let mut models = self.list().await?;
        if models
            .iter()
            .any(|model| model.path == path.to_string_lossy())
        {
            bail!("这个模型路径已经导入，可直接重命名");
        }
        if models.len() >= 100 {
            bail!("最多配置 100 个自定义模型");
        }
        let model = RegisteredModel {
            id: format!("custom-{}", uuid::Uuid::new_v4()),
            name: display_name,
            kind: kind.into(),
            path: path.display().to_string(),
        };
        models.push(model.clone());
        self.save(&models).await?;
        Ok(model)
    }
    pub async fn rename(&self, id: &str, display_name: &str) -> Result<()> {
        let display_name = name(display_name)?;
        let _guard = self.mutation.lock().await;
        let mut models = self.list().await?;
        let model = models
            .iter_mut()
            .find(|model| model.id == id)
            .ok_or_else(|| anyhow::anyhow!("自定义模型不存在"))?;
        model.name = display_name;
        self.save(&models).await
    }
    pub async fn forget(&self, id: &str) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let mut models = self.list().await?;
        models.retain(|model| model.id != id);
        self.save(&models).await
    }
}
#[tauri::command]
pub async fn registered_models(
    state: State<'_, crate::DesktopState>,
) -> Result<Vec<ModelStatus>, String> {
    Ok(state
        .custom_models
        .list()
        .await
        .map_err(|e| format!("{e:#}"))?
        .into_iter()
        .map(|model| ModelStatus {
            ready: validate(&model.kind, Path::new(&model.path)).is_ok(),
            model,
        })
        .collect())
}
#[tauri::command]
pub async fn register_model(
    state: State<'_, crate::DesktopState>,
    kind: String,
    name: String,
    path: String,
) -> Result<RegisteredModel, String> {
    state
        .custom_models
        .register(&kind, &name, Path::new(&path))
        .await
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub async fn rename_registered_model(
    state: State<'_, crate::DesktopState>,
    id: String,
    name: String,
) -> Result<(), String> {
    state
        .custom_models
        .rename(&id, &name)
        .await
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub async fn forget_registered_model(
    state: State<'_, crate::DesktopState>,
    id: String,
) -> Result<(), String> {
    let removed = state
        .custom_models
        .list()
        .await
        .map_err(|e| format!("{e:#}"))?
        .into_iter()
        .find(|m| m.id == id);
    state
        .custom_models
        .forget(&id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if let Some(model) = removed.filter(|m| matches!(m.kind.as_str(), "whisper" | "vad")) {
        state
            .settings_service
            .clear_downloaded_model(&model.kind, Path::new(&model.path))
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(())
}
#[tauri::command]
pub async fn pick_registered_model(app: AppHandle, kind: String) -> Result<Option<String>, String> {
    if matches!(kind.as_str(), "whisper" | "vad") {
        return crate::pick_local_file(&app, "导入本机模型", "GGML 模型", &["bin"], None).await;
    }
    if kind != "qwen-asr" {
        return Err("不支持该模型类型".into());
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("选择完整 Qwen3-ASR 模型目录")
        .pick_folder(move |selection| {
            let _ = sender.send(selection);
        });
    receiver
        .await
        .map_err(|e| e.to_string())?
        .map(|selection| {
            selection
                .into_path()
                .map(|p| p.display().to_string())
                .map_err(|e| e.to_string())
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn registry_persists_names_and_removal_never_deletes_models() {
        let root = std::env::temp_dir().join(format!("atogaki-custom-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("my-whisper.bin");
        std::fs::write(&path, b"local-model-fixture").unwrap();
        let database = LocalDatabase::open(root.join("db.sqlite")).await.unwrap();
        let service = CustomModels::new(database.clone());
        let model = service
            .register("whisper", "我的模型", &path)
            .await
            .unwrap();
        assert!(
            service
                .register("whisper", "重复路径", &path)
                .await
                .is_err()
        );
        assert!(service.register("whisper", "  ", &path).await.is_err());
        service.rename(&model.id, "改名后").await.unwrap();
        let reopened = CustomModels::new(database);
        assert_eq!(reopened.list().await.unwrap()[0].name, "改名后");
        assert_eq!(reopened.list().await.unwrap()[0].id, model.id);
        service.forget(&model.id).await.unwrap();
        assert!(reopened.list().await.unwrap().is_empty());
        assert!(path.is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unknown_or_incomplete_model_packages_are_rejected() {
        assert!(validate("anything", Path::new("/tmp")).is_err());
        assert!(validate("qwen-asr", Path::new("/tmp")).is_err());
        assert!(validate("vad", Path::new("/no/such/model.bin")).is_err());
    }
    #[test]
    fn qwen_registration_requires_the_processor_and_tokenizer_package() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-qwen-registration-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("config.json"), br#"{"model_type":"qwen3_asr"}"#).unwrap();
        for file in [
            "preprocessor_config.json",
            "tokenizer_config.json",
            "tokenizer.json",
            "model.safetensors",
        ] {
            std::fs::write(root.join(file), b"fixture").unwrap();
        }
        validate("qwen-asr", &root).unwrap();
        #[cfg(unix)]
        {
            std::fs::rename(root.join("model.safetensors"), root.join("weights.bin")).unwrap();
            std::os::unix::fs::symlink("weights.bin", root.join("model.safetensors")).unwrap();
            validate("qwen-asr", &root).unwrap();
        }
        std::fs::remove_file(root.join("preprocessor_config.json")).unwrap();
        assert!(validate("qwen-asr", &root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
