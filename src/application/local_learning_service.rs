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
