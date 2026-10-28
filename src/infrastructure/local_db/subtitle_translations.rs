//! Machine translation application and run metadata persistence.
//! Group validation, cue writes and context dependencies retain one transaction.
use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};

use super::{
    LocalDatabase, LocalMachineTranslation, LocalSubtitleSegmentRecord, LocalTranslationDependency,
    NewLocalTranslationRun,
};

impl LocalDatabase {
    pub async fn apply_machine_translations(
        &self,
        job_id: &str,
        translations: &[LocalMachineTranslation],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        self.apply_machine_translation_group(job_id, translations, &[])
            .await
    }

    pub async fn apply_machine_translation_group(
        &self,
        job_id: &str,
        translations: &[LocalMachineTranslation],
        dependencies: &[LocalTranslationDependency],
    ) -> Result<Vec<LocalSubtitleSegmentRecord>> {
        if translations.is_empty() {
            return self.list_segments(job_id).await;
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin local translation transaction")?;

        let mut dependency_stale = false;
        let mut dependency_revisions = HashMap::new();
        for dependency in dependencies {
            let current = sqlx::query_as::<_, (String, i64)>(
                "SELECT s.source_text, p.source_revision
                 FROM local_subtitle_segments s
                 JOIN local_subtitle_provenance p ON p.segment_id = s.id
                 WHERE s.job_id = ? AND s.id = ?",
            )
            .bind(job_id)
            .bind(&dependency.segment_id)
            .fetch_optional(&mut *tx)
            .await
            .context("failed to verify translation context revision")?;
            match current {
                Some((source_text, revision)) => {
                    dependency_stale |= source_text != dependency.source_text
                        || revision != dependency.expected_source_revision;
                    dependency_revisions.insert(dependency.segment_id.as_str(), revision);
                }
                None => dependency_stale = true,
            }
        }

        for translation in translations {
            if translation.expected_source_revision.is_some()
                || translation.expected_translation_revision.is_some()
            {
                let current: (i64, i64) = sqlx::query_as("SELECT source_revision, translation_revision FROM local_subtitle_provenance WHERE job_id = ? AND segment_id = ?")
                    .bind(job_id).bind(&translation.segment_id).fetch_one(&mut *tx).await?;
                if translation
                    .expected_source_revision
                    .is_some_and(|revision| revision != current.0)
                    || translation
                        .expected_translation_revision
                        .is_some_and(|revision| revision != current.1)
                {
                    return Err(anyhow!(
                        "subtitle revision changed while it was being translated: {}",
                        translation.segment_id
                    ));
                }
            }
            let translated_text = translation.translated_text.trim();
            if translated_text.is_empty() {
                return Err(anyhow!(
                    "machine translation cannot be empty for segment {}",
                    translation.segment_id
                ));
            }
            let result = sqlx::query(
                "UPDATE local_subtitle_segments
                 SET translated_text = ?, translation_edited = 0, translation_stale = 0
                 WHERE job_id = ? AND id = ? AND source_text = ?
                   AND translated_text IS ? AND translation_edited = ?",
            )
            .bind(translated_text)
            .bind(job_id)
            .bind(&translation.segment_id)
            .bind(&translation.source_text)
            .bind(&translation.expected_translated_text)
            .bind(translation.expected_translation_edited)
            .execute(&mut *tx)
            .await
            .context("failed to store local machine translation")?;
            if result.rows_affected() != 1 {
                return Err(anyhow!(
                    "subtitle changed while it was being translated: {}",
                    translation.segment_id
                ));
            }

            // An identical translation does not fire the translated_text trigger.
            // Always replace its context snapshot, including removed dependencies.
            sqlx::query(
                "DELETE FROM local_translation_dependency_edges WHERE target_segment_id = ?",
            )
            .bind(&translation.segment_id)
            .execute(&mut *tx)
            .await
            .context("failed to replace translation context dependencies")?;

            for (context_segment_id, context_source_revision) in &dependency_revisions {
                if *context_segment_id == translation.segment_id {
                    continue;
                }
                sqlx::query(
                    "INSERT INTO local_translation_dependency_edges (
                        target_segment_id, context_segment_id, context_source_revision
                     ) VALUES (?, ?, ?)",
                )
                .bind(&translation.segment_id)
                .bind(context_segment_id)
                .bind(context_source_revision)
                .execute(&mut *tx)
                .await
                .context("failed to record translation context dependency")?;
            }
            sqlx::query(
                "UPDATE local_subtitle_provenance
                 SET translation_dependency_stale = ?,
                     updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
                 WHERE segment_id = ?",
            )
            .bind(dependency_stale)
            .bind(&translation.segment_id)
            .execute(&mut *tx)
            .await
            .context("failed to update translation dependency state")?;
        }

        sqlx::query(
            "UPDATE local_jobs
             SET updated_at_unix = MAX(updated_at_unix, ?)
             WHERE job_id = ?",
        )
        .bind(chrono::Utc::now().timestamp())
        .bind(job_id)
        .execute(&mut *tx)
        .await
        .context("failed to update local task translation timestamp")?;
        tx.commit()
            .await
            .context("failed to commit local translation transaction")?;
        self.list_segments(job_id).await
    }

    pub async fn record_translation_run(&self, run: &NewLocalTranslationRun) -> Result<()> {
        sqlx::query(
            "INSERT INTO local_translation_runs (
                id, job_id, provider_id, provider_name, model, endpoint_kind,
                grouping_strategy, options_json, plan_json,
                segment_count, input_tokens, output_tokens, completed_at_unix
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.job_id)
        .bind(&run.provider_id)
        .bind(&run.provider_name)
        .bind(&run.model)
        .bind(&run.endpoint_kind)
        .bind(&run.grouping_strategy)
        .bind(&run.options_json)
        .bind(&run.plan_json)
        .bind(run.segment_count)
        .bind(run.input_tokens)
        .bind(run.output_tokens)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await
        .context("failed to record local translation run")?;
        Ok(())
    }
}
