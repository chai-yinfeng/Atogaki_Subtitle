use std::collections::{BTreeSet, HashSet};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        LanguageCode,
        glossary::{Glossary, GlossaryEntry},
    },
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
    pub prompt_scope: String,
    pub content_group: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalGlossaryPromptPreview {
    pub glossary_id: String,
    pub glossary_name: String,
    pub available_content_groups: Vec<String>,
    pub selected_content_groups: Vec<String>,
    pub core_term_count: usize,
    pub selected_content_term_count: usize,
    pub correction_only_count: usize,
    pub included_prompt_term_count: usize,
    pub prompt_character_count: usize,
    pub prompt: Option<String>,
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
        let terms = parse_builtin_glossary(include_str!("../../assets/glossaries/yorushika.txt"))?;
        self.database
            .ensure_builtin_glossary("Yorushika", "prompt-scopes-v1", terms)
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
        source_language: LanguageCode,
        terms: Vec<LocalGlossaryTermDraft>,
    ) -> Result<LocalGlossaryDetail> {
        self.database
            .save_glossary(
                glossary_id,
                name,
                source_language.as_str(),
                terms
                    .into_iter()
                    .map(|term| LocalGlossaryTermInput {
                        source_text: term.source_text,
                        target_text: term.target_text,
                        prompt_scope: term.prompt_scope,
                        content_group: term.content_group,
                    })
                    .collect(),
            )
            .await
    }

    pub async fn delete(&self, glossary_id: &str) -> Result<()> {
        self.database.delete_glossary(glossary_id).await
    }

    pub async fn prompt_preview(
        &self,
        glossary_id: &str,
        selected_content_groups: &[String],
    ) -> Result<LocalGlossaryPromptPreview> {
        let detail = self.get(glossary_id).await?;
        prompt_preview_from_detail(&detail, selected_content_groups)
    }

    pub async fn preview_apply(
        &self,
        job_id: &str,
        glossary_id: &str,
    ) -> Result<LocalGlossaryPreview> {
        let detail = self.get(glossary_id).await?;
        self.ensure_job_language_matches_glossary(job_id, &detail)
            .await?;
        let glossary = glossary_from_detail(&detail)?;
        let segments = self.database.list_segments(job_id).await?;
        let changes = segments
            .into_iter()
            .filter_map(|segment| {
                let after_text = glossary.corrected_text(&segment.source_text);
                (after_text != segment.source_text).then(|| LocalGlossarySegmentChange {
                    segment_id: segment.id,
                    segment_index: segment.segment_index,
                    before_text: segment.source_text,
                    after_text,
                    translation_will_be_stale: segment.translated_text.is_some(),
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

    async fn ensure_job_language_matches_glossary(
        &self,
        job_id: &str,
        detail: &LocalGlossaryDetail,
    ) -> Result<()> {
        let job = self
            .database
            .get_job(job_id)
            .await?
            .ok_or_else(|| anyhow!("local task not found: {job_id}"))?;
        if job.source_language != detail.glossary.source_language {
            return Err(anyhow!(
                "glossary language {} does not match task source language {}",
                detail.glossary.source_language,
                job.source_language
            ));
        }
        Ok(())
    }
}

pub(crate) fn glossary_from_detail(detail: &LocalGlossaryDetail) -> Result<Glossary> {
    Glossary::from_entries(detail.terms.iter().map(|term| GlossaryEntry {
        source_text: term.source_text.clone(),
        target_text: term.target_text.clone(),
        include_in_prompt: term.prompt_scope != "correction_only",
    }))
}

pub(crate) fn glossary_for_task(
    detail: &LocalGlossaryDetail,
    selected_content_groups: &[String],
) -> Result<Glossary> {
    let selected = normalized_selected_groups(detail, selected_content_groups)?;
    Glossary::from_entries(detail.terms.iter().filter_map(|term| {
        let include_in_prompt = match term.prompt_scope.as_str() {
            "core" => true,
            "content" => selected.contains(term.content_group.as_deref().unwrap_or_default()),
            "correction_only" => false,
            _ => return None,
        };
        let included = include_in_prompt || term.prompt_scope == "correction_only";
        included.then(|| GlossaryEntry {
            source_text: term.source_text.clone(),
            target_text: term.target_text.clone(),
            include_in_prompt,
        })
    }))
}

fn prompt_preview_from_detail(
    detail: &LocalGlossaryDetail,
    selected_content_groups: &[String],
) -> Result<LocalGlossaryPromptPreview> {
    let selected = normalized_selected_groups(detail, selected_content_groups)?;
    let glossary = glossary_for_task(detail, selected_content_groups)?;
    let prompt = glossary.whisper_prompt(None);
    Ok(LocalGlossaryPromptPreview {
        glossary_id: detail.glossary.id.clone(),
        glossary_name: detail.glossary.name.clone(),
        available_content_groups: available_content_groups(detail),
        selected_content_groups: selected.iter().cloned().collect(),
        core_term_count: detail
            .terms
            .iter()
            .filter(|term| term.prompt_scope == "core")
            .count(),
        selected_content_term_count: detail
            .terms
            .iter()
            .filter(|term| {
                term.prompt_scope == "content"
                    && selected.contains(term.content_group.as_deref().unwrap_or_default())
            })
            .count(),
        correction_only_count: detail
            .terms
            .iter()
            .filter(|term| term.prompt_scope == "correction_only")
            .count(),
        included_prompt_term_count: detail
            .terms
            .iter()
            .filter(|term| {
                term.prompt_scope == "core"
                    || term.prompt_scope == "content"
                        && selected.contains(term.content_group.as_deref().unwrap_or_default())
            })
            .count(),
        prompt_character_count: prompt
            .as_deref()
            .map(|text| text.chars().count())
            .unwrap_or(0),
        prompt,
    })
}

fn available_content_groups(detail: &LocalGlossaryDetail) -> Vec<String> {
    detail
        .terms
        .iter()
        .filter(|term| term.prompt_scope == "content")
        .filter_map(|term| term.content_group.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalized_selected_groups(
    detail: &LocalGlossaryDetail,
    selected_content_groups: &[String],
) -> Result<BTreeSet<String>> {
    let available = available_content_groups(detail)
        .into_iter()
        .collect::<HashSet<_>>();
    let selected = selected_content_groups
        .iter()
        .map(|group| group.trim())
        .filter(|group| !group.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if let Some(group) = selected.iter().find(|group| !available.contains(*group)) {
        return Err(anyhow!("unknown glossary content group: {group}"));
    }
    Ok(selected)
}

fn parse_builtin_glossary(file_text: &str) -> Result<Vec<LocalGlossaryTermInput>> {
    let mut scope = "core".to_string();
    let mut content_group = None;
    let mut terms = Vec::new();
    for line in file_text.lines().map(str::trim) {
        if let Some(group) = line.strip_prefix("# @content ") {
            scope = "content".to_string();
            content_group = Some(group.trim().to_string());
            continue;
        }
        if line == "# @core" {
            scope = "core".to_string();
            content_group = None;
            continue;
        }
        if line == "# @correction-only" {
            scope = "correction_only".to_string();
            content_group = None;
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (source_text, target_text) = line
            .split_once("=>")
            .or_else(|| line.split_once('\t'))
            .map(|(source, target)| (source.trim().to_string(), Some(target.trim().to_string())))
            .unwrap_or_else(|| (line.to_string(), None));
        terms.push(LocalGlossaryTermInput {
            source_text,
            target_text,
            prompt_scope: scope.clone(),
            content_group: content_group.clone(),
        });
    }
    Ok(terms)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{LocalGlossaryService, LocalGlossaryTermDraft, glossary_for_task};
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
                crate::domain::LanguageCode::Japanese,
                vec![LocalGlossaryTermDraft {
                    source_text: "ナブナ".to_string(),
                    target_text: Some("n-buna".to_string()),
                    prompt_scope: "core".to_string(),
                    content_group: None,
                }],
            )
            .await
            .unwrap();
        let job = Job::create_in(&root).unwrap();
        let manifest = JobManifest::new(&job, None, None, crate::domain::LanguagePair::default());
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
        assert_eq!(applied.segments[0].source_text, "n-bunaさん");
        assert!(applied.segments[0].translation_stale);

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn task_prompt_uses_core_and_selected_content_but_keeps_corrections_separate() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-local-prompt-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let service = LocalGlossaryService::new(database.clone());
        let detail = service
            .save(
                None,
                "范围词表".to_string(),
                crate::domain::LanguageCode::Japanese,
                vec![
                    LocalGlossaryTermDraft {
                        source_text: "スイ".to_string(),
                        target_text: Some("suis".to_string()),
                        prompt_scope: "core".to_string(),
                        content_group: None,
                    },
                    LocalGlossaryTermDraft {
                        source_text: "月に吠える".to_string(),
                        target_text: None,
                        prompt_scope: "content".to_string(),
                        content_group: Some("幻燈".to_string()),
                    },
                    LocalGlossaryTermDraft {
                        source_text: "水井".to_string(),
                        target_text: Some("suis".to_string()),
                        prompt_scope: "correction_only".to_string(),
                        content_group: None,
                    },
                ],
            )
            .await
            .unwrap();

        let core_only = service
            .prompt_preview(&detail.glossary.id, &[])
            .await
            .unwrap();
        let core_prompt = core_only.prompt.unwrap();
        assert!(core_prompt.contains("スイ（表記: suis）"));
        assert!(!core_prompt.contains("月に吠える"));
        assert!(!core_prompt.contains("水井"));

        let selected = service
            .prompt_preview(&detail.glossary.id, &["幻燈".to_string()])
            .await
            .unwrap();
        assert!(selected.prompt.unwrap().contains("月に吠える"));
        let resolved = glossary_for_task(&detail, &["幻燈".to_string()]).unwrap();
        assert!(
            resolved
                .to_file_text()
                .contains("@correction-only 水井 => suis")
        );
        assert_eq!(resolved.corrected_text("水井です"), "suisです");

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn yorushika_builtin_keeps_people_core_and_songs_in_content_groups() {
        let root = std::env::temp_dir().join(format!(
            "atogaki-builtin-scope-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let service = LocalGlossaryService::new(database.clone());
        service.ensure_builtins().await.unwrap();
        let glossary = service.list().await.unwrap().remove(0);

        assert_eq!(glossary.core_term_count, 17);
        assert_eq!(glossary.content_term_count, 126);
        assert_eq!(glossary.correction_only_count, 2);
        assert_eq!(glossary.content_group_count, 11);
        let detail = service.get(&glossary.id).await.unwrap();
        let resolved = glossary_for_task(&detail, &[]).unwrap();
        assert_eq!(resolved.corrected_text("すいとなぶな"), "suisとn-buna");
        let preview = service.prompt_preview(&glossary.id, &[]).await.unwrap();
        let prompt = preview.prompt.unwrap();
        assert!(prompt.contains("スイ（表記: suis）"));
        assert!(!prompt.contains("すい"));
        assert!(!prompt.contains("月に吠える"));

        drop(service);
        drop(database);
        fs::remove_dir_all(root).unwrap();
    }
}
