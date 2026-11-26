use anyhow::{Result, anyhow};

use crate::infrastructure::local_db::{
    LocalDatabase, LocalLearningItemDetail, LocalSubtitleSegmentRecord,
    NewLocalLearningDictionarySelection, NewLocalLearningGrammarFeedback,
    NewLocalLearningGrammarSelection, NewLocalLearningLookupResult, NewLocalLearningSelection,
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

    pub async fn save_dictionary_selection(
        &self,
        input: NewLocalLearningSelection,
        dictionary: NewLocalLearningDictionarySelection,
    ) -> Result<LocalLearningItemDetail> {
        self.database
            .save_learning_dictionary_selection(input, dictionary)
            .await
    }

    pub async fn grammar_context(
        &self,
        job_id: &str,
        start_segment_id: &str,
        end_segment_id: &str,
        language: &str,
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.database
            .learning_grammar_context(job_id, start_segment_id, end_segment_id, language)
            .await
    }

    pub async fn save_grammar_selection(
        &self,
        input: NewLocalLearningSelection,
        grammar: NewLocalLearningGrammarSelection,
    ) -> Result<LocalLearningItemDetail> {
        self.database
            .save_learning_grammar_selection(input, grammar)
            .await
    }

    pub async fn dictionary_entry_id(&self, item_id: &str) -> Result<Option<String>> {
        self.database.learning_dictionary_entry_id(item_id).await
    }

    pub async fn save_grammar_feedback(
        &self,
        input: NewLocalLearningSelection,
        feedback: NewLocalLearningGrammarFeedback,
    ) -> Result<LocalLearningItemDetail> {
        self.database
            .save_learning_grammar_feedback(input, feedback)
            .await
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

fn tsv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// One note per learning item. All source snapshots stay available, including
/// occurrences whose original task has been deleted. The chosen meaning is
/// exported as saved.
pub fn anki_tsv(items: &[LocalLearningItemDetail], language: &str) -> String {
    let mut result = String::from(
        "#separator:tab\n#html:false\n#guid column:9\n#columns:Expression\tMeaning\tExamples\tTranslations\tSources\tTimecodes\tLanguage\tKind\tGUID\n",
    );
    for detail in items
        .iter()
        .filter(|d| d.item.source_language == language && d.item.grammar_feedback.is_none())
    {
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
        let guid = format!("atogaki-basic-{}", detail.item.id);
        let fields = [
            detail.item.source_text.as_str(),
            detail.item.meaning_text.as_deref().unwrap_or(""),
            &examples,
            &translations,
            &sources,
            &times,
            &detail.item.source_language,
            &detail.item.item_type,
            &guid,
        ];
        result.push_str(
            &fields
                .iter()
                .map(|f| tsv_cell(f))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        result.push('\n');
    }
    result
}

/// Export one saved context per card. The GUID keeps distinct senses separate
/// even when their cloze text is identical.
pub fn anki_cloze_tsv(items: &[LocalLearningItemDetail], language: &str) -> Option<String> {
    fn html(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('{', "&#123;")
            .replace('}', "&#125;")
            .replace(':', "&#58;")
            .replace("\r\n", "<br>")
            .replace(['\r', '\n'], "<br>")
    }
    let mut result = String::from(
        "#separator:tab\n#html:true\n#notetype:Cloze\n#guid column:3\n#columns:Text\tBack Extra\tGUID\n",
    );
    let header_len = result.len();
    for detail in items.iter().filter(|detail| {
        detail.item.source_language == language
            && detail.item.item_type == "selection"
            && detail.item.grammar_feedback.is_none()
    }) {
        let Some((occurrence, text)) = detail.occurrences.iter().find_map(|occurrence| {
            let chars: Vec<u16> = occurrence.segment_source_snapshot.encode_utf16().collect();
            let start = usize::try_from(occurrence.selection_start_utf16).ok()?;
            let end = usize::try_from(occurrence.selection_end_utf16).ok()?;
            let before = String::from_utf16(chars.get(..start)?).ok()?;
            let answer = String::from_utf16(chars.get(start..end)?).ok()?;
            let after = String::from_utf16(chars.get(end..)?).ok()?;
            if answer.trim().is_empty()
                || !before
                    .chars()
                    .chain(after.chars())
                    .any(char::is_alphanumeric)
            {
                return None;
            }
            Some((
                occurrence,
                format!(
                    "{}{{{{c1::{}}}}}{}",
                    html(&before),
                    html(&answer),
                    html(&after)
                ),
            ))
        }) else {
            continue;
        };
        let source = format!(
            "{} · {}–{} ms",
            occurrence.job_display_name_snapshot, occurrence.start_ms, occurrence.end_ms
        );
        let extra = [
            detail.item.meaning_text.as_deref().unwrap_or(""),
            occurrence
                .segment_translation_snapshot
                .as_deref()
                .unwrap_or(""),
            &source,
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(html)
        .collect::<Vec<_>>()
        .join("<br>");
        let guid = format!("atogaki-{}", detail.item.id);
        result.push_str(&[tsv_cell(&text), tsv_cell(&extra), tsv_cell(&guid)].join("\t"));
        result.push('\n');
    }
    (result.len() != header_len).then_some(result)
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
            grammar_id: None,
            grammar_feedback: None,
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
        assert!(exported.contains("#guid column:9"));
        assert!(exported.contains("atogaki-basic-item"));
        assert!(!anki_tsv(&[detail.clone()], "ja").contains("hello"));

        let mut detail = detail;
        detail.item.item_type = "selection".into();
        let text = "🙂 She said <hi> {{c9::literal}}.";
        detail.occurrences[0].segment_source_snapshot = text.into();
        detail.occurrences[0].selection_start_utf16 = 12;
        detail.occurrences[0].selection_end_utf16 = 16;
        let cloze = anki_cloze_tsv(&[detail.clone()], "en").unwrap();
        assert!(cloze.contains("#notetype:Cloze"));
        assert!(
            cloze.contains(
                "She said {{c1::&lt;hi&gt;}} &#123;&#123;c9&#58;&#58;literal&#125;&#125;"
            )
        );
        assert!(cloze.contains("问候<br>第二行"));
        assert!(cloze.contains("atogaki-item"));
        assert!(anki_cloze_tsv(&[detail.clone()], "ja").is_none());
        detail.item.item_type = "sentence".into();
        assert!(anki_cloze_tsv(&[detail], "en").is_none());
    }
}
