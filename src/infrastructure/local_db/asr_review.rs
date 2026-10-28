//! ASR review signals, append-only decisions and candidate repair attempts.
//! All methods borrow the existing database pool; transaction boundaries stay local.
use anyhow::{Context, Result, anyhow};

use crate::application::{QualitySignal, RepairAttempt, ReviewDecision};

use super::{LocalAsrQualitySignalRecord, LocalAsrRepairAttemptRecord, LocalDatabase, to_i64};

impl LocalDatabase {
    pub async fn replace_asr_quality_signals(
        &self,
        run_id: &str,
        signals: &[QualitySignal],
    ) -> Result<()> {
        if signals.iter().any(|signal| signal.run_id != run_id) {
            return Err(anyhow!("ASR quality signal belongs to a different run"));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin ASR quality transaction")?;
        sqlx::query("DELETE FROM local_asr_quality_signals WHERE run_id = ?")
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .context("failed to replace ASR quality signals")?;
        for signal in signals {
            sqlx::query(
                "INSERT INTO local_asr_quality_signals (
                    id, job_id, run_id, signal_kind, severity, start_ms, end_ms,
                    cue_ids_json, detector_version, message, evidence_json, created_at_unix
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&signal.id)
            .bind(&signal.job_id)
            .bind(&signal.run_id)
            .bind(signal.kind.as_str())
            .bind(signal.severity.as_str())
            .bind(
                signal
                    .start_ms
                    .map(|value| to_i64(value, "quality signal start"))
                    .transpose()?,
            )
            .bind(
                signal
                    .end_ms
                    .map(|value| to_i64(value, "quality signal end"))
                    .transpose()?,
            )
            .bind(serde_json::to_string(&signal.cue_ids)?)
            .bind(&signal.detector_version)
            .bind(&signal.message)
            .bind(serde_json::to_string(&signal.evidence)?)
            .bind(to_i64(
                signal.created_at_unix,
                "quality signal creation time",
            )?)
            .execute(&mut *tx)
            .await
            .context("failed to insert ASR quality signal")?;
        }
        tx.commit()
            .await
            .context("failed to commit ASR quality signals")
    }

    pub async fn list_asr_quality_signals(
        &self,
        job_id: &str,
    ) -> Result<Vec<LocalAsrQualitySignalRecord>> {
        sqlx::query_as(
            "SELECT s.id, s.job_id, s.run_id, s.signal_kind, s.severity,
                    s.start_ms, s.end_ms, s.cue_ids_json, s.detector_version,
                    s.message, s.evidence_json, s.created_at_unix,
                    (SELECT d.decision FROM local_asr_review_decisions d
                     WHERE d.signal_id = s.id
                     ORDER BY d.created_at_unix DESC, d.rowid DESC LIMIT 1) AS latest_decision,
                    (SELECT d.note FROM local_asr_review_decisions d
                     WHERE d.signal_id = s.id
                     ORDER BY d.created_at_unix DESC, d.rowid DESC LIMIT 1) AS latest_decision_note
             FROM local_asr_quality_signals s
             WHERE s.job_id = ?
             ORDER BY COALESCE(s.start_ms, 0), s.created_at_unix, s.id",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("failed to list ASR quality signals for task {job_id}"))
    }

    pub async fn record_asr_review_decision(&self, decision: &ReviewDecision) -> Result<()> {
        sqlx::query(
            "INSERT INTO local_asr_review_decisions
                (id, signal_id, decision, note, created_at_unix)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&decision.id)
        .bind(&decision.signal_id)
        .bind(decision.decision.as_str())
        .bind(&decision.note)
        .bind(to_i64(
            decision.created_at_unix,
            "review decision creation time",
        )?)
        .execute(&self.pool)
        .await
        .context("failed to record ASR review decision")?;
        Ok(())
    }

    pub async fn record_asr_repair_attempt(&self, attempt: &RepairAttempt) -> Result<()> {
        let same_job: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM local_asr_quality_signals s
                JOIN local_asr_runs r ON r.id = ?
                WHERE s.id = ? AND s.job_id = r.job_id
             )",
        )
        .bind(&attempt.candidate_run_id)
        .bind(&attempt.signal_id)
        .fetch_one(&self.pool)
        .await
        .context("failed to validate ASR repair attempt")?;
        if !same_job {
            return Err(anyhow!("ASR repair attempt crosses task boundaries"));
        }
        sqlx::query(
            "INSERT INTO local_asr_repair_attempts
                (id, signal_id, candidate_run_id, status, created_at_unix, updated_at_unix)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(signal_id, candidate_run_id) DO UPDATE SET
                status = excluded.status, updated_at_unix = excluded.updated_at_unix",
        )
        .bind(&attempt.id)
        .bind(&attempt.signal_id)
        .bind(&attempt.candidate_run_id)
        .bind(attempt.status.as_str())
        .bind(to_i64(
            attempt.created_at_unix,
            "repair attempt creation time",
        )?)
        .bind(to_i64(
            attempt.updated_at_unix,
            "repair attempt update time",
        )?)
        .execute(&self.pool)
        .await
        .context("failed to record ASR repair attempt")?;
        Ok(())
    }

    pub async fn mark_asr_repair_attempts_for_run(
        &self,
        candidate_run_id: &str,
        status: crate::application::RepairAttemptStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE local_asr_repair_attempts
             SET status = ?, updated_at_unix = ?
             WHERE candidate_run_id = ?",
        )
        .bind(status.as_str())
        .bind(chrono::Utc::now().timestamp())
        .bind(candidate_run_id)
        .execute(&self.pool)
        .await
        .context("failed to update ASR repair attempt")?;
        Ok(result.rows_affected())
    }

    pub async fn list_asr_repair_attempts(
        &self,
        signal_id: &str,
    ) -> Result<Vec<LocalAsrRepairAttemptRecord>> {
        sqlx::query_as(
            "SELECT id, signal_id, candidate_run_id, status, created_at_unix, updated_at_unix
             FROM local_asr_repair_attempts WHERE signal_id = ?
             ORDER BY created_at_unix DESC, id DESC",
        )
        .bind(signal_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list ASR repair attempts")
    }
}
