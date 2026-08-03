use std::path::PathBuf;

use crate::{
    application::{TranscriptionOptions, TranslationOptions},
    domain::render::RenderOptions,
};

#[derive(Debug, Clone)]
pub struct TranscribeSpec {
    pub input: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub transcription: TranscriptionOptions,
}

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub input: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub render_output: Option<PathBuf>,
    pub deepl_auth_key: Option<String>,
    pub transcription: TranscriptionOptions,
    pub translation: TranslationOptions,
    pub render: RenderOptions,
}

#[derive(Debug, Clone)]
pub struct TranslateSpec {
    pub job_dir: PathBuf,
    pub deepl_auth_key: Option<String>,
    pub translation: TranslationOptions,
}

#[derive(Debug, Clone)]
pub struct ApplyGlossarySpec {
    pub job_dir: PathBuf,
    pub glossary: PathBuf,
    pub keep_translations: bool,
}

#[derive(Debug, Clone)]
pub struct ExportSpec {
    pub job_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RenderSpec {
    pub input: PathBuf,
    pub job_dir: PathBuf,
    pub output: PathBuf,
    pub render: RenderOptions,
}

#[derive(Debug, Clone)]
pub struct RerenderSpec {
    pub job_dir: PathBuf,
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub render: RenderOptions,
}
