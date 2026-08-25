ALTER TABLE local_jobs
ADD COLUMN sort_position INTEGER;

CREATE INDEX local_jobs_sort_position_idx
ON local_jobs (sort_position ASC);
