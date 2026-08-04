ALTER TABLE local_jobs
    ADD COLUMN glossary_id TEXT REFERENCES local_glossaries(id) ON DELETE SET NULL;

ALTER TABLE local_jobs
    ADD COLUMN glossary_name TEXT;

ALTER TABLE local_jobs
    ADD COLUMN glossary_snapshot_path TEXT;

CREATE INDEX local_jobs_glossary_idx ON local_jobs (glossary_id);
