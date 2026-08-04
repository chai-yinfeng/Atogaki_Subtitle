use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    domain::glossary::{Glossary, GlossaryEntry},
    infrastructure::local_db::{
        LocalDatabase, LocalGlossaryCorrection, LocalGlossaryDetail, LocalGlossaryRecord,
        LocalGlossaryTermInput, LocalSubtitleSegmentRecord,
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGlossaryTermDraft {
    pub source_text: String,
    pub target_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalGlossarySegmentChange {
    pub segment_id: String,
    pub segment_index: i64,
    pub before_text: String,
    pub after_text: String,
    pub translation_will_be_stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalGlossaryPreview {
    pub glossary_id: String,
    pub glossary_name: String,
    pub changes: Vec<LocalGlossarySegmentChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalGlossaryApplyResult {
    pub changed_segments: usize,
    pub stale_translations: usize,
    pub segments: Vec<LocalSubtitleSegmentRecord>,
}

/// Application boundary for reusable recognition hints and conservative ASR
/// correction mappings stored in the desktop SQLite database.
#[derive(Debug, Clone)]
pub struct LocalGlossaryService {
    database: LocalDatabase,
}

impl LocalGlossaryService {
    pub fn new(database: LocalDatabase) -> Self {
        Self { database }
    }

    pub async fn ensure_builtins(&self) -> Result<()> {
        self.database
            .ensure_builtin_glossary(
                "Yorushika",
                include_str!("../../assets/glossaries/yorushika.txt"),
            )
            .await
    }

    pub async fn list(&self) -> Result<Vec<LocalGlossaryRecord>> {
        self.database.list_glossaries().await
    }

    pub async fn get(&self, glossary_id: &str) -> Result<LocalGlossaryDetail> {
        self.database
            .get_glossary(glossary_id)
            .await?
            .ok_or_else(|| anyhow!("local glossary not found: {glossary_id}"))
    }

    pub async fn save(
        &self,
        glossary_id: Option<&str>,
        name: String,
        terms: Vec<LocalGlossaryTermDraft>,
    ) -> Result<LocalGlossaryDetail> {
        self.database
            .save_glossary(
                glossary_id,
                name,
                terms
                    .into_iter()
                    .map(|term| LocalGlossaryTermInput {
                        source_text: term.source_text,
                        target_text: term.target_text,
                    })
                    .collect(),
            )
            .await
    }

    pub async fn delete(&self, glossary_id: &str) -> Result<()> {
        self.database.delete_glossary(glossary_id).await
    }

    pub async fn preview_apply(
        &self,
        job_id: &str,
        glossary_id: &str,
    ) -> Result<LocalGlossaryPreview> {
        let detail = self.get(glossary_id).await?;
        let glossary = glossary_from_detail(&detail)?;
        let segments = self.database.list_segments(job_id).await?;
        let changes = segments
            .into_iter()
            .filter_map(|segment| {
                let after_text = glossary.corrected_text(&segment.ja_text);
                (after_text != segment.ja_text).then(|| LocalGlossarySegmentChange {
                    segment_id: segment.id,
                    segment_index: segment.segment_index,
                    before_text: segment.ja_text,
                    after_text,
                    translation_will_be_stale: segment.zh_text.is_some(),
                })
            })
            .collect();
        Ok(LocalGlossaryPreview {
            glossary_id: detail.glossary.id,
            glossary_name: detail.glossary.name,
            changes,
        })
    }

    pub async fn apply(&self, job_id: &str, glossary_id: &str) -> Result<LocalGlossaryApplyResult> {
        let preview = self.preview_apply(job_id, glossary_id).await?;
        let stale_translations = preview
            .changes
            .iter()
            .filter(|change| change.translation_will_be_stale)
            .count();
        let corrections = preview
            .changes
            .iter()
            .map(|change| LocalGlossaryCorrection {
                segment_id: change.segment_id.clone(),
                source_text: change.before_text.clone(),
                corrected_text: change.after_text.clone(),
            })
            .collect::<Vec<_>>();
        let segments = self
            .database
            .apply_glossary_corrections(job_id, &corrections)
            .await?;
        Ok(LocalGlossaryApplyResult {
            changed_segments: corrections.len(),
            stale_translations,
            segments,
        })
    }
}

pub(crate) fn glossary_from_detail(detail: &LocalGlossaryDetail) -> Result<Glossary> {
    Glossary::from_entries(detail.terms.iter().map(|term| GlossaryEntry {
        source_text: term.source_text.clone(),
        target_text: term.target_text.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{LocalGlossaryService, LocalGlossaryTermDraft};
    use crate::{
        application::{job_manifest::JobManifest, job_snapshot::JobSnapshot},
        domain::TranscriptSegment,
        infrastructure::{job_store::Job, local_db::LocalDatabase},
    };

    #[tokio::test]
    async fn previews_and_applies_corrections_to_the_sqlite_workspace() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-local-glossary-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let service = LocalGlossaryService::new(database.clone());
        let glossary = service
            .save(
                None,
                "节目词表".to_string(),
                vec![LocalGlossaryTermDraft {
                    source_text: "ナブナ".to_string(),
                    target_text: Some("n-buna".to_string()),
                }],
            )
            .await
            .unwrap();
        let job = Job::create_in(&root).unwrap();
        let manifest = JobManifest::new(&job, None, None);
        let mut segment = TranscriptSegment::new(0, 1_000, "ナブナさん".to_string());
        segment.set_translation(Some("N-buna 老师".to_string()));
        database
            .sync_snapshot(&JobSnapshot {
                manifest: manifest.clone(),
                segments: vec![segment],
            })
            .await
            .unwrap();

        let preview = service
            .preview_apply(&manifest.job_id, &glossary.glossary.id)
            .await
            .unwrap();
        assert_eq!(preview.changes.len(), 1);
        assert_eq!(preview.changes[0].after_text, "n-bunaさん");

        let applied = service
            .apply(&manifest.job_id, &glossary.glossary.id)
            .await
            .unwrap();
        assert_eq!(applied.changed_segments, 1);
        assert_eq!(applied.stale_translations, 1);
        assert_eq!(applied.segments[0].ja_text, "n-bunaさん");
        assert!(applied.segments[0].translation_stale);

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
