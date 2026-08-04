ALTER TABLE local_jobs
    ADD COLUMN display_name TEXT;

CREATE INDEX local_jobs_display_name_idx ON local_jobs (display_name);
