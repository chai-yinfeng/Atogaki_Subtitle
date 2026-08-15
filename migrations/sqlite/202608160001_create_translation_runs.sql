CREATE TABLE local_translation_runs (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    model TEXT,
    endpoint_kind TEXT NOT NULL,
    segment_count INTEGER NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    completed_at_unix INTEGER NOT NULL,
    CHECK (length(trim(provider_id)) > 0),
    CHECK (length(trim(provider_name)) > 0),
    CHECK (length(trim(endpoint_kind)) > 0),
    CHECK (segment_count > 0),
    CHECK (input_tokens IS NULL OR input_tokens >= 0),
    CHECK (output_tokens IS NULL OR output_tokens >= 0)
);

CREATE INDEX local_translation_runs_job_idx
    ON local_translation_runs (job_id, completed_at_unix DESC);
