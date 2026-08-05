CREATE TABLE local_render_jobs (
    id TEXT PRIMARY KEY,
    source_job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    input_path TEXT NOT NULL,
    subtitle_path TEXT NOT NULL,
    output_path TEXT NOT NULL,
    subtitle_track TEXT NOT NULL,
    status TEXT NOT NULL,
    progress REAL NOT NULL DEFAULT 0,
    encoder TEXT,
    audio_encoder TEXT,
    error_message TEXT,
    created_at_unix INTEGER NOT NULL,
    updated_at_unix INTEGER NOT NULL,
    CHECK (subtitle_track IN ('japanese', 'chinese', 'bilingual')),
    CHECK (status IN ('queued', 'running', 'done', 'failed', 'cancelled')),
    CHECK (progress >= 0 AND progress <= 1)
);

CREATE INDEX local_render_jobs_source_idx
    ON local_render_jobs (source_job_id, created_at_unix DESC);

CREATE INDEX local_render_jobs_status_idx
    ON local_render_jobs (status, updated_at_unix DESC);
