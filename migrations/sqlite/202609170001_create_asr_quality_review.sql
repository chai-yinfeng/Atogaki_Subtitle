CREATE TABLE local_asr_quality_signals (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES local_asr_runs(id) ON DELETE CASCADE,
    signal_kind TEXT NOT NULL CHECK (signal_kind IN (
        'repeated_loop', 'boundary_repetition', 'abnormal_speech_rate',
        'speech_during_silence', 'manual_concern'
    )),
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    start_ms INTEGER,
    end_ms INTEGER,
    cue_ids_json TEXT NOT NULL,
    detector_version TEXT NOT NULL,
    message TEXT NOT NULL,
    evidence_json TEXT NOT NULL,
    created_at_unix INTEGER NOT NULL
);

CREATE INDEX local_asr_quality_signals_job_range_idx
ON local_asr_quality_signals (job_id, start_ms, end_ms, created_at_unix);

CREATE TABLE local_asr_review_decisions (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES local_asr_quality_signals(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK (decision IN ('confirmed_problem', 'false_positive', 'resolved')),
    note TEXT,
    created_at_unix INTEGER NOT NULL
);

CREATE INDEX local_asr_review_decisions_signal_idx
ON local_asr_review_decisions (signal_id, created_at_unix DESC);

CREATE TABLE local_asr_repair_attempts (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL REFERENCES local_asr_quality_signals(id) ON DELETE CASCADE,
    candidate_run_id TEXT NOT NULL REFERENCES local_asr_runs(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('candidate', 'adopted', 'rejected')),
    created_at_unix INTEGER NOT NULL,
    updated_at_unix INTEGER NOT NULL,
    UNIQUE(signal_id, candidate_run_id)
);
