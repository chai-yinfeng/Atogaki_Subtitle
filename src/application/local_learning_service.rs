use anyhow::Result;

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

    pub async fn delete_item(&self, item_id: &str) -> Result<()> {
        self.database.delete_learning_item(item_id).await
    }
}
