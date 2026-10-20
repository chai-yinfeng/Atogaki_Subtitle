use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
pub struct ThemeImage {
    pub name: String,
    pub display_name: String,
    pub path: String,
}

pub fn image_path(data_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let (hash, extension) = name.rsplit_once('.').ok_or("无效的主题图片名称")?;
    if hash.len() != 64
        || !hash.bytes().all(|c| c.is_ascii_hexdigit())
        || !["jpg", "png", "webp"].contains(&extension)
    {
        return Err("无效的主题图片名称".into());
    }
    Ok(data_dir.join("themes/imported").join(name))
}

pub fn import_image(data_dir: &Path, source: &Path) -> Result<ThemeImage, String> {
    const LIMIT: u64 = 25 * 1024 * 1024;
    let file = std::fs::File::open(source).map_err(|e| format!("无法打开图片：{e}"))?;
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取图片：{e}"))?;
    if bytes.len() as u64 > LIMIT {
        return Err("请选择不超过 25 MB 的图片".into());
    }
    let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        return Err("支持 PNG、JPEG、WebP 图片，请勿使用 SVG 或网页文件".into());
    };
    let name = format!("{:x}.{extension}", Sha256::digest(&bytes));
    let path = image_path(data_dir, &name)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    // Private staging prevents a failed import from exposing partial bytes.
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        if path.exists() {
            std::fs::remove_file(&temporary)?;
        } else {
            std::fs::rename(&temporary, &path)?;
        }
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("图片导入失败：{error}"));
    }
    Ok(ThemeImage {
        name,
        display_name: source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        path: path.display().to_string(),
    })
}

pub fn external_url(value: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "无效的网页地址")?;
    if !["https", "http"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("仅支持不含凭据的 HTTP / HTTPS 网页地址".into());
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_images_are_content_addressed_and_path_scoped() {
        let root = std::env::temp_dir().join(format!("atogaki-theme-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("original.png");
        let bytes = b"\x89PNG\r\n\x1a\nreference-image";
        std::fs::write(&source, bytes).unwrap();
        let image = import_image(&root, &source).unwrap();
        assert_eq!(std::fs::read(&image.path).unwrap(), bytes);
        assert_eq!(import_image(&root, &source).unwrap().name, image.name);
        assert!(image_path(&root, "../original.png").is_err());
        std::fs::write(&source, b"<svg></svg>").unwrap();
        assert!(import_image(&root, &source).is_err());
        assert_eq!(std::fs::read(&image.path).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn external_links_do_not_accept_files_commands_or_embedded_credentials() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://name:secret@example.com",
            "--args",
        ] {
            assert!(external_url(url).is_err());
        }
        assert_eq!(
            external_url("https://yorushika.com/").unwrap(),
            "https://yorushika.com/"
        );
    }
}
