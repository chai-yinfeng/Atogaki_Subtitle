use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::{
    application::TranslationOptions,
    domain::{TranscriptSegment, subtitle},
    infrastructure::{
        deepl,
        job_store::Job,
        local_db::{
            LocalDatabase, LocalJobRecord, LocalMachineTranslation, LocalSubtitleSegmentRecord,
        },
    },
};

#[derive(Debug, Clone, Serialize)]
pub struct LocalWorkspaceJob {
    pub job: LocalJobRecord,
    pub segments: Vec<LocalSubtitleSegmentRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalTranslationStatus {
    pub provider: &'static str,
    pub configured: bool,
    pub source_language: String,
    pub target_language: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalSubtitleExport {
    pub ja_srt: String,
    pub zh_srt: String,
    pub bilingual_srt: String,
    pub bilingual_ass: String,
    pub missing_translation_count: usize,
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
    deepl_auth_key: Option<String>,
    translation_lock: Arc<Mutex<()>>,
}

impl LocalWorkspaceService {
    pub fn new(database: LocalDatabase) -> Self {
        Self::with_deepl(database, None)
    }

    pub fn with_deepl(database: LocalDatabase, deepl_auth_key: Option<String>) -> Self {
        Self {
            database,
            deepl_auth_key: deepl_auth_key.filter(|key| !key.trim().is_empty()),
            translation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn translation_status(&self) -> LocalTranslationStatus {
        let options = TranslationOptions::default();
        LocalTranslationStatus {
            provider: "DeepL",
            configured: self.deepl_auth_key.is_some(),
            source_language: options.source_language,
            target_language: options.target_language,
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
        ja_text: String,
        zh_text: Option<String>,
    ) -> Result<LocalSubtitleSegmentRecord> {
        self.database
            .update_segment_text(job_id, segment_id, ja_text, zh_text)
            .await
    }

    pub async fn translate_segment(
        &self,
        job_id: &str,
        segment_id: &str,
    ) -> Result<LocalSubtitleSegmentRecord> {
        let _guard = self.translation_lock.lock().await;
        let segment = self
            .database
            .get_segment(job_id, segment_id)
            .await?
            .ok_or_else(|| anyhow!("subtitle segment not found: {segment_id}"))?;
        let translated = self.translate_records(job_id, &[segment]).await?;
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
        self.translate_records(job_id, &segments).await
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
                    .zh_text
                    .as_deref()
                    .is_none_or(|text| text.trim().is_empty())
            })
            .count();
        let job = Job::open(PathBuf::from(&workspace.job.storage_dir))?;
        subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;
        subtitle::write_srt(&job.zh_srt, &segments, subtitle::SubtitleTrack::Chinese)?;
        subtitle::write_srt(
            &job.bilingual_srt,
            &segments,
            subtitle::SubtitleTrack::Bilingual,
        )?;
        subtitle::write_ass(&job.bilingual_ass, &segments)?;

        Ok(LocalSubtitleExport {
            ja_srt: job.ja_srt.display().to_string(),
            zh_srt: job.zh_srt.display().to_string(),
            bilingual_srt: job.bilingual_srt.display().to_string(),
            bilingual_ass: job.bilingual_ass.display().to_string(),
            missing_translation_count,
        })
    }

    async fn translate_records(
        &self,
        job_id: &str,
        segments: &[LocalSubtitleSegmentRecord],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        let auth_key = self.deepl_auth_key.as_deref().ok_or_else(|| {
            anyhow!("DeepL API key missing. Set DEEPL_AUTH_KEY and restart Atogaki")
        })?;
        let source_texts = segments
            .iter()
            .map(|segment| segment.ja_text.clone())
            .collect::<Vec<_>>();
        let translated =
            deepl::translate_texts(auth_key, &TranslationOptions::default(), &source_texts)
                .await
                .context("failed to translate SQLite subtitle workspace")?;
        let updates = segments
            .iter()
            .zip(translated)
            .map(|(segment, translated_text)| LocalMachineTranslation {
                segment_id: segment.id.clone(),
                source_text: segment.ja_text.clone(),
                translated_text,
            })
            .collect::<Vec<_>>();
        self.database
            .apply_machine_translations(job_id, &updates)
            .await
    }
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
                ja_text: record.ja_text.clone(),
                zh_text: record.zh_text.clone(),
                source_edited: record.source_edited,
                translation_stale: record.translation_stale,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::LocalWorkspaceService;
    use crate::{
        application::{job_manifest::JobManifest, job_snapshot::JobSnapshot},
        domain::TranscriptSegment,
        infrastructure::{job_store::Job, local_db::LocalDatabase},
    };

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
        let manifest = JobManifest::new(&job, None, None);
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
        let japanese = fs::read_to_string(&exported.ja_srt).unwrap();
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
        let manifest = JobManifest::new(&job, None, None);
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
                segment.zh_text,
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
