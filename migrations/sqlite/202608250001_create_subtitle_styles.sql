CREATE TABLE local_subtitle_styles (
    job_id TEXT PRIMARY KEY REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    styles_json TEXT NOT NULL,
    updated_at_unix INTEGER NOT NULL,
    CHECK (length(trim(styles_json)) > 0)
);
