use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::{
    application::{TranslationOptions, TranslationProvider, UnconfiguredTranslationProvider},
    domain::{LanguageCode, TranscriptSegment, subtitle},
    infrastructure::{
        job_store::Job,
        local_db::{
            LocalDatabase, LocalJobRecord, LocalMachineTranslation, LocalSubtitleSegmentRecord,
        },
    },
};

const TRANSLATION_BATCH_SIZE: usize = 12;
const TRANSLATION_CONTEXT_WINDOW_MS: i64 = 30_000;
const TRANSLATION_CONTEXT_MAX_CHARS: usize = 2_000;

#[derive(Debug, Clone, Serialize)]
pub struct LocalWorkspaceJob {
    pub job: LocalJobRecord,
    pub segments: Vec<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalTranslationStatus {
    pub provider_id: String,
    pub provider: String,
    pub configured: bool,
    pub model: Option<String>,
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

    pub fn translation_status(&self) -> LocalTranslationStatus {
        let provider = self.translation_provider.status();
        LocalTranslationStatus {
            provider_id: provider.id,
            provider: provider.name,
            configured: provider.configured,
            model: provider.model,
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
        Ok(LocalWorkspaceJob { job, segments })
    }

    pub async fn update_subtitle_text(
        &self,
        job_id: &str,
        segment_id: &str,
        source_text: String,
        translated_text: Option<String>,
    ) -> Result<LocalSubtitleSegmentRecord> {
        self.database
            .update_segment_text(job_id, segment_id, source_text, translated_text)
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
        subtitle::write_ass(&job.bilingual_ass, &segments)?;

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
        subtitle::write_ass_track(output, &segments, track)?;
        Ok(missing_translation_count)
    }

    pub async fn subtitle_export_plan(
        &self,
        job_id: &str,
        output_directory: &Path,
    ) -> Result<LocalSubtitleExportPlan> {
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        planned_subtitle_export(&job, output_directory)
    }

    pub async fn export_subtitles_to(
        &self,
        job_id: &str,
        output_directory: &Path,
        overwrite_existing: bool,
    ) -> Result<LocalSubtitleExport> {
        let plan = self.subtitle_export_plan(job_id, output_directory).await?;
        if !overwrite_existing && !plan.existing_files.is_empty() {
            return Err(anyhow!(
                "subtitle export would overwrite {} existing file(s)",
                plan.existing_files.len()
            ));
        }

        // Always refresh the task-local projections first. The selected export
        // is a copy of the same current SQLite state used by future rendering.
        let generated = self.export_subtitles(job_id).await?;
        copy_export_file(&generated.source_srt, &plan.source_srt, overwrite_existing)?;
        copy_export_file(
            &generated.translated_srt,
            &plan.translated_srt,
            overwrite_existing,
        )?;
        copy_export_file(
            &generated.bilingual_srt,
            &plan.bilingual_srt,
            overwrite_existing,
        )?;
        copy_export_file(
            &generated.bilingual_ass,
            &plan.bilingual_ass,
            overwrite_existing,
        )?;

        Ok(LocalSubtitleExport {
            source_srt: plan.source_srt,
            translated_srt: plan.translated_srt,
            bilingual_srt: plan.bilingual_srt,
            bilingual_ass: plan.bilingual_ass,
            missing_translation_count: generated.missing_translation_count,
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
        let options = TranslationOptions::new(
            LanguageCode::from_str(&job.source_language).map_err(anyhow::Error::msg)?,
            LanguageCode::from_str(&job.target_language).map_err(anyhow::Error::msg)?,
        );
        let mut updates = Vec::with_capacity(segments_to_translate.len());
        for batch in segments_to_translate.chunks(TRANSLATION_BATCH_SIZE) {
            let source_texts = batch
                .iter()
                .map(|segment| segment.source_text.clone())
                .collect::<Vec<_>>();
            let context = translation_context(context_segments, batch);
            let translated = self
                .translation_provider
                .translate(&options, &source_texts, context.as_deref())
                .await
                .with_context(|| {
                    format!(
                        "failed to translate SQLite subtitle workspace with {}",
                        provider.name
                    )
                })?;
            if translated.len() != batch.len() {
                return Err(anyhow!(
                    "{} returned {} translations for {} subtitle segments",
                    provider.name,
                    translated.len(),
                    batch.len()
                ));
            }
            updates.extend(
                batch
                    .iter()
                    .zip(translated)
                    .map(|(segment, translated_text)| LocalMachineTranslation {
                        segment_id: segment.id.clone(),
                        source_text: segment.source_text.clone(),
                        translated_text,
                    }),
            );
        }
        self.database
            .apply_machine_translations(job_id, &updates)
            .await
    }
}

fn planned_subtitle_export(
    job: &LocalJobRecord,
    output_directory: &Path,
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
    let paths = [&source_srt, &translated_srt, &bilingual_srt, &bilingual_ass];
    let existing_files = paths
        .iter()
        .filter(|path| path.exists())
        .map(|path| path.display().to_string())
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

fn copy_export_file(source: &str, destination: &str, overwrite_existing: bool) -> Result<()> {
    let source = Path::new(source);
    let destination = Path::new(destination);
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
) -> Option<String> {
    let first = batch.first()?;
    let last = batch.last()?;
    let window_start = first.start_ms.saturating_sub(TRANSLATION_CONTEXT_WINDOW_MS);
    let window_end = last.end_ms.saturating_add(TRANSLATION_CONTEXT_WINDOW_MS);
    let target_ids = batch
        .iter()
        .map(|segment| segment.id.as_str())
        .collect::<HashSet<_>>();

    let mut context = String::new();
    let mut context_chars = 0;
    let mut target_start = None;
    let mut target_end = 0;
    for segment in all_segments
        .iter()
        .filter(|segment| segment.end_ms >= window_start && segment.start_ms <= window_end)
    {
        if !context.is_empty() {
            context.push('\n');
            context_chars += 1;
        }
        let line_start = context_chars;
        context.push_str(&segment.source_text);
        context_chars += segment.source_text.chars().count();
        if target_ids.contains(segment.id.as_str()) {
            target_start.get_or_insert(line_start);
            target_end = context_chars;
        }
    }

    if context.trim().is_empty() {
        return None;
    }
    if context_chars <= TRANSLATION_CONTEXT_MAX_CHARS {
        return Some(context);
    }

    let chars = context.chars().collect::<Vec<_>>();
    let target_start = target_start.unwrap_or(0);
    let target_length = target_end.saturating_sub(target_start);
    let surrounding_budget = TRANSLATION_CONTEXT_MAX_CHARS.saturating_sub(target_length);
    let desired_start = target_start.saturating_sub(surrounding_budget / 2);
    let latest_start = chars.len().saturating_sub(TRANSLATION_CONTEXT_MAX_CHARS);
    let start = desired_start.min(latest_start);
    let end = (start + TRANSLATION_CONTEXT_MAX_CHARS).min(chars.len());
    Some(chars[start..end].iter().collect())
}

fn workspace_segments(records: &[LocalSubtitleSegmentRecord]) -> Result<Vec<TranscriptSegment>> {
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
        sync::{Arc, Mutex as StdMutex},
    };

    use super::{LocalWorkspaceService, TRANSLATION_CONTEXT_MAX_CHARS, translation_context};
    use crate::{
        application::{
            TranslationFuture, TranslationOptions, TranslationProvider, TranslationProviderStatus,
            job_manifest::JobManifest, job_snapshot::JobSnapshot,
        },
        domain::{LanguageCode, LanguagePair, TranscriptSegment},
        infrastructure::{
            job_store::Job,
            local_db::{LocalDatabase, LocalSubtitleSegmentRecord},
        },
    };

    type CapturedTranslationRequest = (LanguageCode, LanguageCode, Vec<String>, Option<String>);

    #[derive(Debug, Clone)]
    struct FakeTranslationProvider {
        requests: Arc<StdMutex<Vec<CapturedTranslationRequest>>>,
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
                configuration_hint: None,
            }
        }

        fn translate<'a>(
            &'a self,
            options: &'a TranslationOptions,
            texts: &'a [String],
            context: Option<&'a str>,
        ) -> TranslationFuture<'a> {
            let source_texts = texts.to_vec();
            self.requests.lock().unwrap().push((
                options.source_language,
                options.target_language,
                source_texts.clone(),
                context.map(str::to_string),
            ));
            let omit_last_result = self.omit_last_result;
            Box::pin(async move {
                let mut translated = source_texts
                    .into_iter()
                    .map(|text| format!("译：{text}"))
                    .collect::<Vec<_>>();
                if omit_last_result {
                    translated.pop();
                }
                Ok(translated)
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

        let context = translation_context(&segments, &segments[2..3]).unwrap();

        assert!(context.contains("刚才提到的名字"));
        assert!(context.contains("当前 SQLite 日文"));
        assert!(context.contains("接下来的话题"));
        assert!(!context.contains("远处的旧内容"));
        assert!(!context.contains("远处的新内容"));
    }

    #[test]
    fn translation_context_is_capped_around_the_current_batch() {
        let segments = vec![
            subtitle_record(0, 0, 1_000, "前".repeat(1_500)),
            subtitle_record(1, 1_000, 2_000, "当前中心"),
            subtitle_record(2, 2_000, 3_000, "后".repeat(1_500)),
        ];

        let context = translation_context(&segments, &segments[1..2]).unwrap();

        assert_eq!(context.chars().count(), TRANSLATION_CONTEXT_MAX_CHARS);
        assert!(context.contains("当前中心"));
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

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, LanguageCode::English);
        assert_eq!(requests[0].1, LanguageCode::SimplifiedChinese);
        assert_eq!(requests[0].2, ["The first line", "The second line"]);
        let context = requests[0].3.as_deref().unwrap();
        assert!(context.contains("The first line"));
        assert!(context.contains("The second line"));
        drop(requests);

        drop(service);
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

        let plan = service
            .subtitle_export_plan(&manifest.job_id, &export_directory)
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
            .export_subtitles_to(&manifest.job_id, &export_directory, false)
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
            .subtitle_export_plan(&manifest.job_id, &export_directory)
            .await
            .unwrap();
        assert_eq!(conflicting.existing_files.len(), 4);
        let error = service
            .export_subtitles_to(&manifest.job_id, &export_directory, false)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("would overwrite 4"));
        service
            .export_subtitles_to(&manifest.job_id, &export_directory, true)
            .await
            .unwrap();

        drop(service);
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
            .update_segment_text(
                &manifest.job_id,
                &first.id,
                "SQLiteで直した原文".to_string(),
                Some("SQLite 中修正的译文".to_string()),
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
        assert!(!japanese.contains("生成された原文"));
        assert!(bilingual.contains("SQLite 中修正的译文"));
        assert!(ass.contains("SQLite 中修正的译文"));

        drop(service);
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
            .update_segment_text(
                &manifest.job_id,
                &segment.id,
                "修正した原文".to_string(),
                segment.translated_text,
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

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
