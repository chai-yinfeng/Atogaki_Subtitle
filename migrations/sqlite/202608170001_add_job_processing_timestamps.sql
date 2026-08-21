ALTER TABLE local_jobs
ADD COLUMN started_at_unix INTEGER;

ALTER TABLE local_jobs
ADD COLUMN completed_at_unix INTEGER;
