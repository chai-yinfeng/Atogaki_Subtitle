ALTER TABLE local_jobs
    ADD COLUMN source_language TEXT NOT NULL DEFAULT 'ja';

ALTER TABLE local_jobs
    ADD COLUMN target_language TEXT NOT NULL DEFAULT 'zh-Hans';

ALTER TABLE local_subtitle_segments
    RENAME COLUMN ja_text TO source_text;

ALTER TABLE local_subtitle_segments
    RENAME COLUMN zh_text TO translated_text;

ALTER TABLE local_glossaries
    ADD COLUMN source_language TEXT NOT NULL DEFAULT 'ja';

CREATE INDEX local_jobs_languages_idx
    ON local_jobs (source_language, target_language);

CREATE INDEX local_glossaries_language_idx
    ON local_glossaries (source_language, updated_at_unix DESC);

CREATE TABLE local_render_jobs_language_tracks (
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
    fallback_reason TEXT,
    CHECK (subtitle_track IN ('source', 'translation', 'bilingual')),
    CHECK (status IN ('queued', 'running', 'done', 'failed', 'cancelled')),
    CHECK (progress >= 0 AND progress <= 1)
);

INSERT INTO local_render_jobs_language_tracks (
    id, source_job_id, input_path, subtitle_path, output_path,
    subtitle_track, status, progress, encoder, audio_encoder, error_message,
    created_at_unix, updated_at_unix, fallback_reason
)
SELECT
    id, source_job_id, input_path, subtitle_path, output_path,
    CASE subtitle_track
        WHEN 'japanese' THEN 'source'
        WHEN 'chinese' THEN 'translation'
        ELSE 'bilingual'
    END,
    status, progress, encoder, audio_encoder, error_message,
    created_at_unix, updated_at_unix, fallback_reason
FROM local_render_jobs;

DROP TABLE local_render_jobs;
ALTER TABLE local_render_jobs_language_tracks RENAME TO local_render_jobs;

CREATE INDEX local_render_jobs_source_idx
    ON local_render_jobs (source_job_id, created_at_unix DESC);

CREATE INDEX local_render_jobs_status_idx
    ON local_render_jobs (status, updated_at_unix DESC);
