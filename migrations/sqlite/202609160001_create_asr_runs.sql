CREATE TABLE local_asr_runs (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    parent_run_id TEXT REFERENCES local_asr_runs(id) ON DELETE SET NULL,
    run_kind TEXT NOT NULL CHECK (run_kind IN ('full', 'selected_range')),
    provider_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    model_identity TEXT NOT NULL,
    input_media_path TEXT NOT NULL,
    scope_start_ms INTEGER NOT NULL CHECK (scope_start_ms >= 0),
    scope_end_ms INTEGER CHECK (scope_end_ms IS NULL OR scope_end_ms > scope_start_ms),
    config_json TEXT NOT NULL,
    artifact_dir TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    created_at_unix INTEGER NOT NULL,
    started_at_unix INTEGER,
    completed_at_unix INTEGER,
    error_message TEXT
);

CREATE INDEX local_asr_runs_job_created_idx
ON local_asr_runs (job_id, created_at_unix DESC, id DESC);

CREATE INDEX local_asr_runs_unfinished_idx
ON local_asr_runs (status)
WHERE status IN ('queued', 'running');
