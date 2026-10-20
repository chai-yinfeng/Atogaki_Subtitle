CREATE TABLE local_acoustic_checkpoints (
 job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
 chunk_index INTEGER NOT NULL, run_id TEXT NOT NULL,
 start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL,
 artifact_path TEXT NOT NULL, device TEXT NOT NULL,
 PRIMARY KEY (job_id, chunk_index)
);
