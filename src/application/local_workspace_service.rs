use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    application::{
        TranslationContextSegment, TranslationOptions, TranslationProvider, TranslationRequest,
        TranslationTargetSegment, UnconfiguredTranslationProvider,
    },
    domain::{
        LanguageCode, TranscriptSegment,
        subtitle::{self, SubtitleStyleSet},
    },
    infrastructure::{
        job_store::Job,
        local_db::{
            LocalDatabase, LocalJobRecord, LocalMachineTranslation, LocalSubtitleSegmentRecord,
            LocalTranslationRunRecord, NewLocalTranslationRun,
        },
        waveform::{self, WaveformWindow},
    },
};

const TRANSLATION_BATCH_SIZE: usize = 12;
const TRANSLATION_CONTEXT_WINDOW_MS: i64 = 30_000;
const TRANSLATION_CONTEXT_MAX_CHARS: usize = 2_000;
const PLAYBACK_POSITION_PREFIX: &str = "listening.playback_position_ms.";

#[derive(Debug, Clone, Serialize)]
pub struct LocalWorkspaceJob {
    pub job: LocalJobRecord,
    pub segments: Vec<LocalSubtitleSegmentRecord>,
    pub translation_runs: Vec<LocalTranslationRunRecord>,
    pub subtitle_styles: SubtitleStyleSet,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalTranslationStatus {
    pub provider_id: String,
    pub provider: String,
    pub configured: bool,
    pub model: Option<String>,
    pub endpoint_kind: String,
    pub configuration_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalSubtitleExport {
    pub source_srt: String,
    pub translated_srt: String,
    pub bilingual_srt: String,
    pub bilingual_ass: String,
    pub missing_translation_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalSubtitleExportPlan {
    pub output_directory: String,
    pub base_name: String,
    pub source_srt: String,
    pub translated_srt: String,
    pub bilingual_srt: String,
    pub bilingual_ass: String,
    pub existing_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSubtitleExportArtifact {
    SourceSrt,
    TranslatedSrt,
    BilingualSrt,
    BilingualAss,
}

/// Application boundary for browsing, translating, editing and exporting the
/// durable desktop workspace.
///
/// Processing artifacts remain in each job directory, while subtitle changes
/// are read from and written to SQLite. Generated files are projections of the
/// current database state; they never become the source for desktop edits.
#[derive(Debug, Clone)]
pub struct LocalWorkspaceService {
    database: LocalDatabase,
    translation_provider: Arc<dyn TranslationProvider>,
    translation_lock: Arc<Mutex<()>>,
}

impl LocalWorkspaceService {
    pub fn new(database: LocalDatabase) -> Self {
        Self::with_provider(database, Arc::new(UnconfiguredTranslationProvider))
    }

    pub fn with_provider(
        database: LocalDatabase,
        translation_provider: Arc<dyn TranslationProvider>,
    ) -> Self {
        Self {
            database,
            translation_provider,
            translation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn playback_position(&self, job_id: &str) -> Result<i64> {
        self.database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let key = format!("{PLAYBACK_POSITION_PREFIX}{job_id}");
        Ok(self
            .database
            .get_setting(&key)
            .await?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0))
    }

    pub async fn save_playback_position(&self, job_id: &str, position_ms: i64) -> Result<()> {
        if position_ms < 0 {
            return Err(anyhow!("playback position cannot be negative"));
        }
        self.database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        self.database
            .set_setting(
                &format!("{PLAYBACK_POSITION_PREFIX}{job_id}"),
                &position_ms.to_string(),
            )
            .await
    }

    pub async fn waveform_window(
        &self,
        job_id: &str,
        start_ms: i64,
        end_ms: i64,
        point_count: usize,
    ) -> Result<WaveformWindow> {
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let task_directory = PathBuf::from(job.storage_dir);
        let audio_path = task_directory.join("audio.wav");
        let cache_path = task_directory.join("waveform-v1.bin");
        tokio::task::spawn_blocking(move || {
            waveform::load_waveform_window(&audio_path, &cache_path, start_ms, end_ms, point_count)
        })
        .await
        .context("waveform worker stopped unexpectedly")?
    }

    pub fn translation_status(&self) -> LocalTranslationStatus {
        let provider = self.translation_provider.status();
        LocalTranslationStatus {
            provider_id: provider.id,
            provider: provider.name,
            configured: provider.configured,
            model: provider.model,
            endpoint_kind: provider.endpoint_kind,
            configuration_hint: provider.configuration_hint,
        }
    }

    pub async fn get_job(&self, job_id: &str) -> Result<LocalWorkspaceJob> {
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let segments = self.database.list_segments(job_id).await?;
        let translation_runs = self.database.list_translation_runs(job_id).await?;
        let subtitle_styles = self.database.get_subtitle_styles(job_id).await?;
        Ok(LocalWorkspaceJob {
            job,
            segments,
            translation_runs,
            subtitle_styles,
        })
    }

    pub async fn save_subtitle_styles(
        &self,
        job_id: &str,
        styles: &SubtitleStyleSet,
    ) -> Result<SubtitleStyleSet> {
        self.database.save_subtitle_styles(job_id, styles).await
    }

    pub async fn update_subtitle(
        &self,
        job_id: &str,
        segment_id: &str,
        source_text: String,
        translated_text: Option<String>,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<LocalSubtitleSegmentRecord> {
        self.database
            .update_segment(
                job_id,
                segment_id,
                source_text,
                translated_text,
                start_ms,
                end_ms,
            )
            .await
    }

    pub async fn restore_subtitle(
        &self,
        snapshot: &LocalSubtitleSegmentRecord,
    ) -> Result<LocalSubtitleSegmentRecord> {
        self.database.restore_segment(snapshot).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn split_subtitle(
        &self,
        job_id: &str,
        segment_id: &str,
        boundary_ms: i64,
        left_source_text: String,
        right_source_text: String,
        left_translated_text: Option<String>,
        right_translated_text: Option<String>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.database
            .split_segment(
                job_id,
                segment_id,
                boundary_ms,
                left_source_text,
                right_source_text,
                left_translated_text,
                right_translated_text,
            )
            .await
    }

    pub async fn merge_subtitles(
        &self,
        job_id: &str,
        left_segment_id: &str,
        right_segment_id: &str,
        source_text: String,
        translated_text: Option<String>,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.database
            .merge_adjacent_segments(
                job_id,
                left_segment_id,
                right_segment_id,
                source_text,
                translated_text,
            )
            .await
    }

    pub async fn restore_subtitle_structure(
        &self,
        job_id: &str,
        before_segments: &[LocalSubtitleSegmentRecord],
        after_segments: &[LocalSubtitleSegmentRecord],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.database
            .restore_segment_structure(job_id, before_segments, after_segments)
            .await
    }

    pub async fn save_subtitle_timing(
        &self,
        job_id: &str,
        before_segments: &[LocalSubtitleSegmentRecord],
        after_segments: &[LocalSubtitleSegmentRecord],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.database
            .save_segment_timing(job_id, before_segments, after_segments)
            .await
    }

    pub async fn translate_segment(
        &self,
        job_id: &str,
        segment_id: &str,
    ) -> Result<LocalSubtitleSegmentRecord> {
        let _guard = self.translation_lock.lock().await;
        let context_segments = self.database.list_segments(job_id).await?;
        let segment = context_segments
            .iter()
            .find(|segment| segment.id == segment_id)
            .cloned()
            .ok_or_else(|| anyhow!("subtitle segment not found: {segment_id}"))?;
        let translated = self
            .translate_records(job_id, &context_segments, &[segment])
            .await?;
        translated
            .into_iter()
            .find(|segment| segment.id == segment_id)
            .ok_or_else(|| anyhow!("translated subtitle segment disappeared: {segment_id}"))
    }

    pub async fn translate_all(&self, job_id: &str) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let _guard = self.translation_lock.lock().await;
        let segments = self.database.list_segments(job_id).await?;
        if segments.is_empty() {
            return Err(anyhow!(
                "cannot translate a workspace without subtitle segments"
            ));
        }
        self.translate_records(job_id, &segments, &segments).await
    }

    pub async fn export_subtitles(&self, job_id: &str) -> Result<LocalSubtitleExport> {
        let workspace = self.get_job(job_id).await?;
        let stale_count = workspace
            .segments
            .iter()
            .filter(|segment| segment.translation_stale)
            .count();
        if stale_count > 0 {
            return Err(anyhow!(
                "{stale_count} subtitle translation(s) are stale; retranslate them before export"
            ));
        }
        if workspace.segments.is_empty() {
            return Err(anyhow!(
                "cannot export a workspace without subtitle segments"
            ));
        }

        let segments = workspace_segments(&workspace.segments)?;
        let missing_translation_count = segments
            .iter()
            .filter(|segment| {
                segment
                    .translated_text
                    .as_deref()
                    .is_none_or(|text| text.trim().is_empty())
            })
            .count();
        let job = Job::open(PathBuf::from(&workspace.job.storage_dir))?;
        subtitle::write_srt(&job.source_srt, &segments, subtitle::SubtitleTrack::Source)?;
        subtitle::write_srt(
            &job.translated_srt,
            &segments,
            subtitle::SubtitleTrack::Translation,
        )?;
        subtitle::write_srt(
            &job.bilingual_srt,
            &segments,
            subtitle::SubtitleTrack::Bilingual,
        )?;
        subtitle::write_ass_track_with_styles(
            &job.bilingual_ass,
            &segments,
            subtitle::SubtitleTrack::Bilingual,
            &workspace.subtitle_styles,
        )?;

        Ok(LocalSubtitleExport {
            source_srt: job.source_srt.display().to_string(),
            translated_srt: job.translated_srt.display().to_string(),
            bilingual_srt: job.bilingual_srt.display().to_string(),
            bilingual_ass: job.bilingual_ass.display().to_string(),
            missing_translation_count,
        })
    }

    /// Freeze the current SQLite subtitle text into an ASS file for a video
    /// render. The snapshot is immutable for that render even if the workspace
    /// is edited while it is queued or running.
    pub async fn export_ass_snapshot(
        &self,
        job_id: &str,
        output: &Path,
        track: subtitle::SubtitleTrack,
    ) -> Result<usize> {
        let workspace = self.get_job(job_id).await?;
        let stale_count = workspace
            .segments
            .iter()
            .filter(|segment| segment.translation_stale)
            .count();
        if stale_count > 0 {
            return Err(anyhow!(
                "{stale_count} subtitle translation(s) are stale; retranslate them before rendering"
            ));
        }
        if workspace.segments.is_empty() {
            return Err(anyhow!(
                "cannot render a workspace without subtitle segments"
            ));
        }
        let segments = workspace_segments(&workspace.segments)?;
        let missing_translation_count = segments
            .iter()
            .filter(|segment| {
                segment
                    .translated_text
                    .as_deref()
                    .is_none_or(|text| text.trim().is_empty())
            })
            .count();
        subtitle::write_ass_track_with_styles(
            output,
            &segments,
            track,
            &workspace.subtitle_styles,
        )?;
        Ok(missing_translation_count)
    }

    pub async fn subtitle_export_plan(
        &self,
        job_id: &str,
        output_directory: &Path,
        artifacts: &[LocalSubtitleExportArtifact],
    ) -> Result<LocalSubtitleExportPlan> {
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        planned_subtitle_export(&job, output_directory, artifacts)
    }

    pub async fn export_subtitles_to(
        &self,
        job_id: &str,
        output_directory: &Path,
        overwrite_existing: bool,
        artifacts: &[LocalSubtitleExportArtifact],
    ) -> Result<LocalSubtitleExport> {
        if artifacts.is_empty() {
            return Err(anyhow!("select at least one subtitle export format"));
        }
        let plan = self
            .subtitle_export_plan(job_id, output_directory, artifacts)
            .await?;
        if !overwrite_existing && !plan.existing_files.is_empty() {
            return Err(anyhow!(
                "subtitle export would overwrite {} existing file(s)",
                plan.existing_files.len()
            ));
        }

        let workspace = self.get_job(job_id).await?;
        if workspace.segments.is_empty() {
            return Err(anyhow!(
                "cannot export a workspace without subtitle segments"
            ));
        }
        let selected = artifacts.iter().copied().collect::<HashSet<_>>();
        let requires_translation = selected
            .iter()
            .any(|artifact| !matches!(artifact, LocalSubtitleExportArtifact::SourceSrt));
        let stale_count = workspace
            .segments
            .iter()
            .filter(|segment| segment.translation_stale)
            .count();
        let missing_translation_count = workspace
            .segments
            .iter()
            .filter(|segment| segment.translated_text.as_deref().is_none_or(str::is_empty))
            .count();
        if requires_translation && stale_count > 0 {
            return Err(anyhow!(
                "{stale_count} subtitle translation(s) are stale; retranslate them before exporting translated subtitles"
            ));
        }
        if requires_translation && missing_translation_count > 0 {
            return Err(anyhow!(
                "{missing_translation_count} subtitle translation(s) are missing; finish translation or export only the source SRT"
            ));
        }
        let segments = workspace_segments(&workspace.segments)?;
        let task_job = Job::open(PathBuf::from(&workspace.job.storage_dir))?;
        for artifact in &selected {
            let (generated, destination) = match artifact {
                LocalSubtitleExportArtifact::SourceSrt => {
                    subtitle::write_srt(
                        &task_job.source_srt,
                        &segments,
                        subtitle::SubtitleTrack::Source,
                    )?;
                    (&task_job.source_srt, &plan.source_srt)
                }
                LocalSubtitleExportArtifact::TranslatedSrt => {
                    subtitle::write_srt(
                        &task_job.translated_srt,
                        &segments,
                        subtitle::SubtitleTrack::Translation,
                    )?;
                    (&task_job.translated_srt, &plan.translated_srt)
                }
                LocalSubtitleExportArtifact::BilingualSrt => {
                    subtitle::write_srt(
                        &task_job.bilingual_srt,
                        &segments,
                        subtitle::SubtitleTrack::Bilingual,
                    )?;
                    (&task_job.bilingual_srt, &plan.bilingual_srt)
                }
                LocalSubtitleExportArtifact::BilingualAss => {
                    subtitle::write_ass_track_with_styles(
                        &task_job.bilingual_ass,
                        &segments,
                        subtitle::SubtitleTrack::Bilingual,
                        &workspace.subtitle_styles,
                    )?;
                    (&task_job.bilingual_ass, &plan.bilingual_ass)
                }
            };
            copy_export_file(generated, destination, overwrite_existing)?;
        }

        Ok(LocalSubtitleExport {
            source_srt: plan.source_srt,
            translated_srt: plan.translated_srt,
            bilingual_srt: plan.bilingual_srt,
            bilingual_ass: plan.bilingual_ass,
            missing_translation_count,
        })
    }

    async fn translate_records(
        &self,
        job_id: &str,
        context_segments: &[LocalSubtitleSegmentRecord],
        segments_to_translate: &[LocalSubtitleSegmentRecord],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let provider = self.translation_provider.status();
        if !provider.configured {
            return Err(anyhow!(
                "{} is not configured. {}",
                provider.name,
                provider
                    .configuration_hint
                    .as_deref()
                    .unwrap_or("Configure the translation provider and restart Atogaki")
            ));
        }
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        let protected_terms = job
            .glossary_snapshot_path
            .as_deref()
            .map(Path::new)
            .map(|path| {
                crate::domain::glossary::load(path)
                    .with_context(|| {
                        format!("failed to load task glossary snapshot {}", path.display())
                    })
                    .map(|glossary| glossary.translation_protected_terms())
            })
            .transpose()?
            .unwrap_or_default();
        let options = TranslationOptions::new(
            LanguageCode::from_str(&job.source_language).map_err(anyhow::Error::msg)?,
            LanguageCode::from_str(&job.target_language).map_err(anyhow::Error::msg)?,
        )
        .with_protected_terms(protected_terms);
        let mut updates = Vec::with_capacity(segments_to_translate.len());
        for batch in segments_to_translate.chunks(TRANSLATION_BATCH_SIZE) {
            let targets = batch
                .iter()
                .map(|segment| TranslationTargetSegment {
                    segment_id: segment.id.clone(),
                    source_text: segment.source_text.clone(),
                })
                .collect::<Vec<_>>();
            let (before_context, after_context) = translation_context(context_segments, batch);
            let response = self
                .translation_provider
                .translate(TranslationRequest {
                    options: options.clone(),
                    before_context,
                    targets,
                    after_context,
                    style_instruction: None,
                })
                .await
                .with_context(|| {
                    format!(
                        "failed to translate SQLite subtitle workspace with {}",
                        provider.name
                    )
                })?;
            if response.translations.len() != batch.len() {
                return Err(anyhow!(
                    "{} returned {} translations for {} subtitle segments",
                    provider.name,
                    response.translations.len(),
                    batch.len()
                ));
            }
            let mut translated_by_id = response
                .translations
                .into_iter()
                .map(|translation| (translation.segment_id.clone(), translation))
                .collect::<std::collections::HashMap<_, _>>();
            if translated_by_id.len() != batch.len() {
                return Err(anyhow!("{} returned duplicate subtitle IDs", provider.name));
            }
            for segment in batch {
                let translation = translated_by_id.remove(&segment.id).ok_or_else(|| {
                    anyhow!(
                        "{} did not return translation for subtitle {}",
                        provider.name,
                        segment.id
                    )
                })?;
                if translation.translated_text.trim().is_empty() {
                    return Err(anyhow!(
                        "{} returned an empty translation for subtitle {}",
                        provider.name,
                        segment.id
                    ));
                }
                updates.push(LocalMachineTranslation {
                    segment_id: segment.id.clone(),
                    source_text: segment.source_text.clone(),
                    translated_text: translation.translated_text,
                });
            }
            self.database
                .record_translation_run(&NewLocalTranslationRun {
                    id: uuid::Uuid::new_v4().to_string(),
                    job_id: job_id.to_string(),
                    provider_id: provider.id.clone(),
                    provider_name: provider.name.clone(),
                    model: response.model.or_else(|| provider.model.clone()),
                    endpoint_kind: provider.endpoint_kind.clone(),
                    segment_count: i64::try_from(batch.len())
                        .context("translation batch size exceeds SQLite i64")?,
                    input_tokens: response.usage.input_tokens,
                    output_tokens: response.usage.output_tokens,
                })
                .await?;
        }
        self.database
            .apply_machine_translations(job_id, &updates)
            .await
    }
}

fn planned_subtitle_export(
    job: &LocalJobRecord,
    output_directory: &Path,
    artifacts: &[LocalSubtitleExportArtifact],
) -> Result<LocalSubtitleExportPlan> {
    if !output_directory.is_dir() {
        return Err(anyhow!(
            "subtitle export directory does not exist: {}",
            output_directory.display()
        ));
    }
    let base_name = subtitle_export_base_name(job);
    let source_language =
        LanguageCode::from_str(&job.source_language).map_err(anyhow::Error::msg)?;
    let target_language =
        LanguageCode::from_str(&job.target_language).map_err(anyhow::Error::msg)?;
    let source_srt = output_directory.join(format!("{base_name}.{source_language}.srt"));
    let translated_srt = output_directory.join(format!("{base_name}.{target_language}.srt"));
    let bilingual_srt = output_directory.join(format!("{base_name}.bilingual.srt"));
    let bilingual_ass = output_directory.join(format!("{base_name}.bilingual.ass"));
    let selected = artifacts.iter().copied().collect::<HashSet<_>>();
    let paths = [
        (LocalSubtitleExportArtifact::SourceSrt, &source_srt),
        (LocalSubtitleExportArtifact::TranslatedSrt, &translated_srt),
        (LocalSubtitleExportArtifact::BilingualSrt, &bilingual_srt),
        (LocalSubtitleExportArtifact::BilingualAss, &bilingual_ass),
    ];
    let existing_files = paths
        .iter()
        .filter(|(artifact, path)| selected.contains(artifact) && path.exists())
        .map(|(_, path)| path.display().to_string())
        .collect();

    Ok(LocalSubtitleExportPlan {
        output_directory: output_directory.display().to_string(),
        base_name,
        source_srt: source_srt.display().to_string(),
        translated_srt: translated_srt.display().to_string(),
        bilingual_srt: bilingual_srt.display().to_string(),
        bilingual_ass: bilingual_ass.display().to_string(),
        existing_files,
    })
}

fn subtitle_export_base_name(job: &LocalJobRecord) -> String {
    let fallback = || format!("Atogaki-{}", job.job_id.chars().take(8).collect::<String>());
    let raw = job
        .display_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            job.input_path.as_deref().and_then(|path| {
                Path::new(path)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(fallback);
    let sanitized = raw
        .chars()
        .take(100)
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim().trim_matches('.').trim().to_string();
    if sanitized.is_empty() {
        fallback()
    } else {
        sanitized
    }
}

fn copy_export_file(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    overwrite_existing: bool,
) -> Result<()> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    if overwrite_existing {
        fs::copy(source, destination).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        return Ok(());
    }

    let mut input = fs::File::open(source)
        .with_context(|| format!("failed to open generated subtitle {}", source.display()))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .with_context(|| {
            format!(
                "subtitle export file already exists or cannot be created: {}",
                destination.display()
            )
        })?;
    io::copy(&mut input, &mut output).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn translation_context(
    all_segments: &[LocalSubtitleSegmentRecord],
    batch: &[LocalSubtitleSegmentRecord],
) -> (
    Vec<TranslationContextSegment>,
    Vec<TranslationContextSegment>,
) {
    let Some(first) = batch.first() else {
        return (Vec::new(), Vec::new());
    };
    let Some(last) = batch.last() else {
        return (Vec::new(), Vec::new());
    };
    let window_start = first.start_ms.saturating_sub(TRANSLATION_CONTEXT_WINDOW_MS);
    let window_end = last.end_ms.saturating_add(TRANSLATION_CONTEXT_WINDOW_MS);
    let target_ids = batch
        .iter()
        .map(|segment| segment.id.as_str())
        .collect::<HashSet<_>>();

    let candidates = all_segments
        .iter()
        .filter(|segment| segment.end_ms >= window_start && segment.start_ms <= window_end)
        .filter(|segment| !target_ids.contains(segment.id.as_str()))
        .collect::<Vec<_>>();
    let mut before = Vec::new();
    let mut used = 0usize;
    for segment in candidates
        .iter()
        .copied()
        .filter(|segment| segment.segment_index < first.segment_index)
        .rev()
    {
        let separator = usize::from(used > 0);
        let available = (TRANSLATION_CONTEXT_MAX_CHARS / 2).saturating_sub(used + separator);
        if available == 0 {
            break;
        }
        let length = segment.source_text.chars().count();
        let truncated = length > available;
        let source_text = if truncated {
            segment
                .source_text
                .chars()
                .skip(length - available)
                .collect()
        } else {
            segment.source_text.clone()
        };
        used += separator + source_text.chars().count();
        before.push(TranslationContextSegment {
            segment_id: segment.id.clone(),
            source_text,
        });
        if truncated {
            break;
        }
    }
    before.reverse();
    let mut after = Vec::new();
    for segment in candidates
        .into_iter()
        .filter(|segment| segment.segment_index > last.segment_index)
    {
        let separator = usize::from(used > 0);
        let available = TRANSLATION_CONTEXT_MAX_CHARS.saturating_sub(used + separator);
        if available == 0 {
            break;
        }
        let length = segment.source_text.chars().count();
        let truncated = length > available;
        let source_text = if truncated {
            segment.source_text.chars().take(available).collect()
        } else {
            segment.source_text.clone()
        };
        used += separator + source_text.chars().count();
        after.push(TranslationContextSegment {
            segment_id: segment.id.clone(),
            source_text,
        });
        if truncated {
            break;
        }
    }
    (before, after)
}

pub(crate) fn workspace_segments(
    records: &[LocalSubtitleSegmentRecord],
) -> Result<Vec<TranscriptSegment>> {
    records
        .iter()
        .map(|record| {
            Ok(TranscriptSegment {
                id: record.id.clone(),
                start_ms: u64::try_from(record.start_ms)
                    .context("SQLite subtitle start time is negative")?,
                end_ms: u64::try_from(record.end_ms)
                    .context("SQLite subtitle end time is negative")?,
                source_text: record.source_text.clone(),
                translated_text: record.translated_text.clone(),
                source_edited: record.source_edited,
                translation_stale: record.translation_stale,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        sync::{Arc, Mutex as StdMutex},
    };

    use super::{
        LocalSubtitleExportArtifact, LocalWorkspaceService, TRANSLATION_CONTEXT_MAX_CHARS,
        translation_context,
    };
    use crate::{
        application::{
            TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
            TranslationResponse, TranslationResult, TranslationUsage, job_manifest::JobManifest,
            job_snapshot::JobSnapshot, job_status::JobStatus,
        },
        domain::{LanguageCode, LanguagePair, TranscriptSegment},
        infrastructure::{
            job_store::Job,
            local_db::{LocalDatabase, LocalGlossaryTermInput, LocalSubtitleSegmentRecord},
        },
    };

    #[derive(Debug, Clone)]
    struct FakeTranslationProvider {
        requests: Arc<StdMutex<Vec<TranslationRequest>>>,
        omit_last_result: bool,
    }

    impl FakeTranslationProvider {
        fn new(omit_last_result: bool) -> Self {
            Self {
                requests: Arc::new(StdMutex::new(Vec::new())),
                omit_last_result,
            }
        }
    }

    impl TranslationProvider for FakeTranslationProvider {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: "fake".to_string(),
                name: "Fake Translate".to_string(),
                configured: true,
                model: Some("deterministic-v1".to_string()),
                endpoint_kind: "test".to_string(),
                configuration_hint: None,
            }
        }

        fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
            self.requests.lock().unwrap().push(request.clone());
            let omit_last_result = self.omit_last_result;
            Box::pin(async move {
                let mut translations = request
                    .targets
                    .into_iter()
                    .map(|target| TranslationResult {
                        segment_id: target.segment_id,
                        translated_text: format!("译：{}", target.source_text),
                    })
                    .collect::<Vec<_>>();
                if omit_last_result {
                    translations.pop();
                }
                Ok(TranslationResponse {
                    translations,
                    model: Some("deterministic-v1".to_string()),
                    usage: TranslationUsage {
                        input_tokens: Some(42),
                        output_tokens: Some(12),
                    },
                })
            })
        }
    }

    fn subtitle_record(
        index: i64,
        start_ms: i64,
        end_ms: i64,
        text: impl Into<String>,
    ) -> LocalSubtitleSegmentRecord {
        LocalSubtitleSegmentRecord {
            id: format!("segment-{index}"),
            job_id: "job".to_string(),
            segment_index: index,
            start_ms,
            end_ms,
            source_text: text.into(),
            translated_text: None,
            source_edited: false,
            translation_edited: false,
            translation_stale: false,
            timing_edited: false,
        }
    }

    #[test]
    fn translation_context_uses_nearby_sqlite_segments() {
        let segments = vec![
            subtitle_record(0, 0, 1_000, "远处的旧内容"),
            subtitle_record(1, 15_000, 16_000, "刚才提到的名字"),
            subtitle_record(2, 40_000, 41_000, "当前 SQLite 日文"),
            subtitle_record(3, 65_000, 66_000, "接下来的话题"),
            subtitle_record(4, 80_000, 81_000, "远处的新内容"),
        ];

        let (before, after) = translation_context(&segments, &segments[2..3]);

        assert_eq!(before[0].source_text, "刚才提到的名字");
        assert_eq!(after[0].source_text, "接下来的话题");
    }

    #[test]
    fn translation_context_is_capped_around_the_current_batch() {
        let segments = vec![
            subtitle_record(0, 0, 1_000, "前".repeat(1_500)),
            subtitle_record(1, 1_000, 2_000, "当前中心"),
            subtitle_record(2, 2_000, 3_000, "后".repeat(1_500)),
        ];

        let (before, after) = translation_context(&segments, &segments[1..2]);
        let context_chars = before
            .iter()
            .chain(after.iter())
            .map(|segment| segment.source_text.chars().count())
            .sum::<usize>()
            + before.len().saturating_add(after.len()).saturating_sub(1);

        assert!(!before.is_empty());
        assert!(!after.is_empty());
        assert!(context_chars <= TRANSLATION_CONTEXT_MAX_CHARS);
    }

    #[tokio::test]
    async fn injected_translation_provider_receives_ordered_text_and_context() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-provider-injection-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let first = TranscriptSegment::new(0, 1_000, "The first line".to_string());
        let second = TranscriptSegment::new(1_000, 2_000, "The second line".to_string());
        let manifest = JobManifest::new(
            &job,
            None,
            None,
            LanguagePair::english_to_simplified_chinese(),
        );
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second.clone()],
            })
            .await
            .unwrap();
        let provider = Arc::new(FakeTranslationProvider::new(false));
        let service = LocalWorkspaceService::with_provider(database.clone(), provider.clone());

        let status = service.translation_status();
        assert_eq!(status.provider_id, "fake");
        assert_eq!(status.provider, "Fake Translate");
        assert_eq!(status.model.as_deref(), Some("deterministic-v1"));

        let translated = service.translate_all(&manifest.job_id).await.unwrap();
        assert_eq!(translated[0].id, first.id);
        assert_eq!(
            translated[0].translated_text.as_deref(),
            Some("译：The first line")
        );
        assert_eq!(translated[1].id, second.id);
        assert_eq!(
            translated[1].translated_text.as_deref(),
            Some("译：The second line")
        );

        {
            let requests = provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].options.source_language, LanguageCode::English);
            assert_eq!(
                requests[0].options.target_language,
                LanguageCode::SimplifiedChinese
            );
            assert_eq!(
                requests[0]
                    .targets
                    .iter()
                    .map(|target| target.source_text.as_str())
                    .collect::<Vec<_>>(),
                ["The first line", "The second line"]
            );
            assert!(requests[0].before_context.is_empty());
            assert!(requests[0].after_context.is_empty());
            assert!(requests[0].options.protected_terms.is_empty());
        }

        let runs = database
            .list_translation_runs(&manifest.job_id)
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].provider_id, "fake");
        assert_eq!(runs[0].provider_name, "Fake Translate");
        assert_eq!(runs[0].model.as_deref(), Some("deterministic-v1"));
        assert_eq!(runs[0].endpoint_kind, "test");
        assert_eq!(runs[0].segment_count, 2);
        assert_eq!(runs[0].input_tokens, Some(42));
        assert_eq!(runs[0].output_tokens, Some(12));

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn structurally_edited_segments_translate_and_export_in_new_order() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-structure-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let first = TranscriptSegment::new(0, 1_000, "Good morning everyone".to_string());
        let second = TranscriptSegment::new(1_000, 2_000, "Welcome back".to_string());
        let mut manifest = JobManifest::new(
            &job,
            None,
            None,
            LanguagePair::english_to_simplified_chinese(),
        );
        manifest.mark(JobStatus::Done);
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second],
            })
            .await
            .unwrap();
        let provider = Arc::new(FakeTranslationProvider::new(false));
        let service = LocalWorkspaceService::with_provider(database.clone(), provider);

        let split = service
            .split_subtitle(
                &manifest.job_id,
                &first.id,
                500,
                "Good morning".to_string(),
                "everyone".to_string(),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(split.len(), 3);
        let translated = service.translate_all(&manifest.job_id).await.unwrap();
        assert_eq!(translated.len(), 3);
        assert_eq!(
            translated[0].translated_text.as_deref(),
            Some("译：Good morning")
        );
        assert_eq!(
            translated[1].translated_text.as_deref(),
            Some("译：everyone")
        );

        let exported = service.export_subtitles(&manifest.job_id).await.unwrap();
        let source = fs::read_to_string(exported.source_srt).unwrap();
        let translated_srt = fs::read_to_string(exported.translated_srt).unwrap();
        assert!(source.contains("Good morning"));
        assert!(source.contains("everyone"));
        assert!(translated_srt.contains("译：Good morning"));
        assert!(translated_srt.contains("译：everyone"));

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn task_glossary_snapshot_becomes_translation_protection() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-translation-glossary-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let manifest = JobManifest::new(&job, None, None, LanguagePair::default());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![TranscriptSegment::new(0, 1_000, "盗作を聴く".to_string())],
            })
            .await
            .unwrap();
        let snapshot_path = root.join("recognition-glossary.txt");
        fs::write(&snapshot_path, "盗作\nナブナ => n-buna\n").unwrap();
        let glossary = database
            .save_glossary(
                None,
                "测试词表".to_string(),
                "ja",
                vec![LocalGlossaryTermInput {
                    source_text: "盗作".to_string(),
                    target_text: None,
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        database
            .assign_job_glossary(
                &manifest.job_id,
                &glossary.glossary.id,
                &glossary.glossary.name,
                &snapshot_path,
            )
            .await
            .unwrap();
        let provider = Arc::new(FakeTranslationProvider::new(false));
        let service = LocalWorkspaceService::with_provider(database.clone(), provider.clone());

        service.translate_all(&manifest.job_id).await.unwrap();

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].options.protected_terms,
            ["n-buna", "ナブナ", "盗作"]
        );
        drop(requests);
        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn provider_result_count_mismatch_does_not_partially_update_sqlite() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-provider-count-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let first = TranscriptSegment::new(0, 1_000, "第一段".to_string());
        let second = TranscriptSegment::new(1_000, 2_000, "第二段".to_string());
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first, second],
            })
            .await
            .unwrap();
        let service = LocalWorkspaceService::with_provider(
            database.clone(),
            Arc::new(FakeTranslationProvider::new(true)),
        );

        let error = service.translate_all(&manifest.job_id).await.unwrap_err();
        assert!(error.to_string().contains("returned 1 translations for 2"));
        assert!(
            database
                .list_segments(&manifest.job_id)
                .await
                .unwrap()
                .iter()
                .all(|segment| segment.translated_text.is_none())
        );

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn exports_named_sqlite_subtitles_without_silent_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-workspace-named-export-test-{}",
            uuid::Uuid::new_v4()
        ));
        let export_directory = root.join("exports");
        fs::create_dir_all(&export_directory).unwrap();
        let job = Job::create_in(&root).unwrap();
        let mut segment = TranscriptSegment::new(0, 1_000, "Current source text".to_string());
        segment.set_translation(Some("当前译文".to_string()));
        let manifest = JobManifest::new(
            &job,
            Some("/media/radio.mp4".into()),
            None,
            LanguagePair::english_to_simplified_chinese(),
        );
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![segment],
            })
            .await
            .unwrap();
        database
            .rename_job(&manifest.job_id, Some("深夜电台: 第/12回".to_string()))
            .await
            .unwrap();
        let service = LocalWorkspaceService::new(database.clone());
        let artifacts = [
            LocalSubtitleExportArtifact::SourceSrt,
            LocalSubtitleExportArtifact::TranslatedSrt,
            LocalSubtitleExportArtifact::BilingualSrt,
            LocalSubtitleExportArtifact::BilingualAss,
        ];

        let plan = service
            .subtitle_export_plan(&manifest.job_id, &export_directory, &artifacts)
            .await
            .unwrap();
        assert_eq!(plan.base_name, "深夜电台_ 第_12回");
        assert!(plan.existing_files.is_empty());
        assert!(plan.source_srt.ends_with("深夜电台_ 第_12回.en.srt"));
        assert!(
            plan.translated_srt
                .ends_with("深夜电台_ 第_12回.zh-Hans.srt")
        );

        let exported = service
            .export_subtitles_to(&manifest.job_id, &export_directory, false, &artifacts)
            .await
            .unwrap();
        assert!(
            fs::read_to_string(&exported.source_srt)
                .unwrap()
                .contains("Current source text")
        );
        assert!(
            fs::read_to_string(&exported.bilingual_ass)
                .unwrap()
                .contains("当前译文")
        );

        let conflicting = service
            .subtitle_export_plan(&manifest.job_id, &export_directory, &artifacts)
            .await
            .unwrap();
        assert_eq!(conflicting.existing_files.len(), 4);
        let error = service
            .export_subtitles_to(&manifest.job_id, &export_directory, false, &artifacts)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("would overwrite 4"));
        service
            .export_subtitles_to(&manifest.job_id, &export_directory, true, &artifacts)
            .await
            .unwrap();
        assert_eq!(
            service.playback_position(&manifest.job_id).await.unwrap(),
            0
        );
        service
            .save_playback_position(&manifest.job_id, 42_500)
            .await
            .unwrap();
        assert_eq!(
            service.playback_position(&manifest.job_id).await.unwrap(),
            42_500
        );
        assert!(
            service
                .save_playback_position(&manifest.job_id, -1)
                .await
                .is_err()
        );

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn exports_the_current_sqlite_workspace_instead_of_generated_json() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-workspace-export-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let first = TranscriptSegment::new(0, 1_000, "生成された原文".to_string());
        let second = TranscriptSegment::new(1_000, 2_000, "二番目".to_string());
        job.write_segments(&[first.clone(), second.clone()])
            .unwrap();
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![first.clone(), second],
            })
            .await
            .unwrap();
        database
            .update_segment(
                &manifest.job_id,
                &first.id,
                "SQLiteで直した原文".to_string(),
                Some("SQLite 中修正的译文".to_string()),
                250,
                1_250,
            )
            .await
            .unwrap();

        let service = LocalWorkspaceService::new(database.clone());
        let exported = service.export_subtitles(&manifest.job_id).await.unwrap();
        assert_eq!(exported.missing_translation_count, 1);
        let japanese = fs::read_to_string(&exported.source_srt).unwrap();
        let bilingual = fs::read_to_string(&exported.bilingual_srt).unwrap();
        let ass = fs::read_to_string(&exported.bilingual_ass).unwrap();
        assert!(japanese.contains("SQLiteで直した原文"));
        assert!(japanese.contains("00:00:00,250 --> 00:00:01,250"));
        assert!(!japanese.contains("生成された原文"));
        assert!(bilingual.contains("SQLite 中修正的译文"));
        assert!(ass.contains("SQLite 中修正的译文"));

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rejects_export_when_a_translation_is_stale() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-workspace-stale-export-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job = Job::create_in(&root).unwrap();
        let mut segment = TranscriptSegment::new(0, 1_000, "原文".to_string());
        segment.set_translation(Some("译文".to_string()));
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![segment.clone()],
            })
            .await
            .unwrap();
        database
            .update_segment(
                &manifest.job_id,
                &segment.id,
                "修正した原文".to_string(),
                segment.translated_text,
                0,
                1_000,
            )
            .await
            .unwrap();

        let service = LocalWorkspaceService::new(database.clone());
        let error = service
            .export_subtitles(&manifest.job_id)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stale"));
        assert!(!job.bilingual_srt.exists());

        let export_directory = root.join("selected-export");
        fs::create_dir(&export_directory).unwrap();
        let source_only = [LocalSubtitleExportArtifact::SourceSrt];
        let exported = service
            .export_subtitles_to(&manifest.job_id, &export_directory, false, &source_only)
            .await
            .unwrap();
        assert!(Path::new(&exported.source_srt).exists());
        assert!(!Path::new(&exported.bilingual_srt).exists());

        let translated = [LocalSubtitleExportArtifact::TranslatedSrt];
        let error = service
            .export_subtitles_to(&manifest.job_id, &export_directory, false, &translated)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stale"));

        drop(service);
        database.close().await;
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
