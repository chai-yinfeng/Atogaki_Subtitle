CREATE TABLE local_jobs (
    job_id TEXT PRIMARY KEY,
    storage_dir TEXT NOT NULL UNIQUE,
    input_path TEXT,
    render_output_path TEXT,
    status TEXT NOT NULL,
    message TEXT NOT NULL,
    error_message TEXT,
    created_at_unix INTEGER NOT NULL,
    updated_at_unix INTEGER NOT NULL
);

CREATE TABLE local_subtitle_segments (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    segment_index INTEGER NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    ja_text TEXT NOT NULL,
    zh_text TEXT,
    source_edited INTEGER NOT NULL DEFAULT 0,
    translation_stale INTEGER NOT NULL DEFAULT 0,
    UNIQUE (job_id, segment_index),
    CHECK (segment_index >= 0),
    CHECK (start_ms >= 0),
    CHECK (end_ms > start_ms),
    CHECK (source_edited IN (0, 1)),
    CHECK (translation_stale IN (0, 1))
);

CREATE TABLE local_glossaries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at_unix INTEGER NOT NULL,
    updated_at_unix INTEGER NOT NULL,
    CHECK (length(trim(name)) > 0)
);

CREATE TABLE local_glossary_terms (
    id TEXT PRIMARY KEY,
    glossary_id TEXT NOT NULL REFERENCES local_glossaries(id) ON DELETE CASCADE,
    source_text TEXT NOT NULL,
    target_text TEXT,
    UNIQUE (glossary_id, source_text, target_text),
    CHECK (length(trim(source_text)) > 0)
);

CREATE INDEX local_jobs_updated_idx ON local_jobs (updated_at_unix DESC);
CREATE INDEX local_subtitle_segments_job_idx ON local_subtitle_segments (job_id, segment_index);
CREATE INDEX local_glossary_terms_glossary_idx ON local_glossary_terms (glossary_id);
