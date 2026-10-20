CREATE TABLE local_asr_candidate_adoptions (
    run_id TEXT PRIMARY KEY REFERENCES local_asr_runs(id) ON DELETE CASCADE
);
INSERT OR IGNORE INTO local_asr_candidate_adoptions(run_id)
SELECT source_run_id FROM local_subtitle_provenance WHERE source_run_id IS NOT NULL;
INSERT OR IGNORE INTO local_asr_candidate_adoptions(run_id)
SELECT candidate_run_id FROM local_asr_repair_attempts WHERE status = 'adopted';
