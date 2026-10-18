use anyhow::{Result, anyhow};

use crate::infrastructure::local_db::{
    LocalDatabase, LocalLearningItemDetail, NewLocalLearningLookupResult, NewLocalLearningSelection,
};

#[derive(Debug, Clone)]
pub struct LocalLearningService {
    database: LocalDatabase,
}

impl LocalLearningService {
    pub fn new(database: LocalDatabase) -> Self {
        Self { database }
    }

    pub async fn save_selection(
        &self,
        input: NewLocalLearningSelection,
    ) -> Result<LocalLearningItemDetail> {
        self.database.save_learning_selection(input).await
    }

    pub async fn list_items(&self) -> Result<Vec<LocalLearningItemDetail>> {
        self.database.list_learning_items().await
    }

    pub async fn get_item(&self, item_id: &str) -> Result<Option<LocalLearningItemDetail>> {
        self.database.get_learning_item(item_id).await
    }

    pub async fn upsert_lookup_result(
        &self,
        input: NewLocalLearningLookupResult,
    ) -> Result<LocalLearningItemDetail> {
        self.database.upsert_learning_lookup_result(input).await
    }

    pub async fn update_meaning(
        &self,
        item_id: &str,
        meaning_text: Option<String>,
    ) -> Result<LocalLearningItemDetail> {
        self.database
            .update_learning_item_meaning(item_id, meaning_text)
            .await
    }

    pub async fn use_lookup_definition(
        &self,
        item_id: &str,
        provider_id: &str,
        definition: &str,
    ) -> Result<LocalLearningItemDetail> {
        let detail = self
            .database
            .get_learning_item(item_id)
            .await?
            .ok_or_else(|| anyhow!("learning item not found: {item_id}"))?;
        let result = detail
            .lookup_results
            .iter()
            .find(|result| result.provider_id == provider_id)
            .ok_or_else(|| anyhow!("dictionary result not found: {provider_id}"))?;
        let definition = definition.trim();
        let stored_definition = result
            .senses
            .iter()
            .flat_map(|sense| sense.definitions.iter())
            .find(|candidate| candidate.trim() == definition)
            .ok_or_else(|| {
                anyhow!("selected definition is not part of the saved dictionary result")
            })?;
        self.database
            .update_learning_item_meaning_from_provider(
                item_id,
                stored_definition,
                &result.provider_id,
                &result.provider_name,
            )
            .await
    }

    pub async fn delete_item(&self, item_id: &str) -> Result<()> {
        self.database.delete_learning_item(item_id).await
    }
}

/// One note per learning item. All source snapshots stay available, including
/// occurrences whose original task has been deleted. No dictionary content is
/// silently substituted for the user's selected meaning.
pub fn anki_tsv(items: &[LocalLearningItemDetail], language: &str) -> String {
    fn cell(value: &str) -> String {
        format!("\"{}\"", value.replace('"', "\"\""))
    }
    let mut result = String::from(
        "#separator:tab\n#html:false\n#columns:Expression\tMeaning\tExamples\tTranslations\tSources\tTimecodes\tLanguage\tKind\n",
    );
    for detail in items.iter().filter(|d| d.item.source_language == language) {
        let join = |values: Vec<String>| values.join("\n");
        let examples = join(
            detail
                .occurrences
                .iter()
                .map(|o| o.segment_source_snapshot.clone())
                .collect(),
        );
        let translations = join(
            detail
                .occurrences
                .iter()
                .map(|o| o.segment_translation_snapshot.clone().unwrap_or_default())
                .collect(),
        );
        let sources = join(
            detail
                .occurrences
                .iter()
                .map(|o| o.job_display_name_snapshot.clone())
                .collect(),
        );
        let times = join(
            detail
                .occurrences
                .iter()
                .map(|o| format!("{}–{} ms", o.start_ms, o.end_ms))
                .collect(),
        );
        let fields = [
            detail.item.source_text.as_str(),
            detail.item.meaning_text.as_deref().unwrap_or(""),
            &examples,
            &translations,
            &sources,
            &times,
            &detail.item.source_language,
            &detail.item.item_type,
        ];
        result.push_str(
            &fields
                .iter()
                .map(|f| cell(f))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::local_db::{LocalLearningItemRecord, LocalLearningOccurrenceRecord};
    #[test]
    fn anki_export_keeps_quotes_tabs_all_sources_and_deleted_task_snapshots() {
        let item = LocalLearningItemRecord {
            id: "item".into(),
            source_language: "en".into(),
            target_language: "zh-Hans".into(),
            item_type: "phrase".into(),
            source_text: "say \"hello\"\t世界".into(),
            meaning_text: Some("问候\n第二行".into()),
            meaning_provider_id: None,
            meaning_source_label: None,
            occurrence_count: 2,
            created_at_unix: 0,
            updated_at_unix: 0,
        };
        let occurrence = LocalLearningOccurrenceRecord {
            id: "source".into(),
            learning_item_id: "item".into(),
            job_id: None,
            segment_id: None,
            job_display_name_snapshot: "Deleted program".into(),
            segment_source_snapshot: "Original context".into(),
            segment_translation_snapshot: Some("原始语境".into()),
            selection_start_utf16: 0,
            selection_end_utf16: 3,
            start_ms: 1000,
            end_ms: 2000,
            created_at_unix: 0,
        };
        let mut second = occurrence.clone();
        second.id = "second".into();
        second.job_display_name_snapshot = "Another program".into();
        let detail = LocalLearningItemDetail {
            item,
            occurrences: vec![occurrence, second],
            lookup_results: vec![],
        };
        let exported = anki_tsv(&[detail.clone()], "en");
        assert!(exported.contains("\"say \"\"hello\"\"\t世界\""));
        assert!(exported.contains("\"问候\n第二行\""));
        assert!(exported.contains("Deleted program\nAnother program"));
        assert!(!anki_tsv(&[detail], "ja").contains("hello"));
    }
}
