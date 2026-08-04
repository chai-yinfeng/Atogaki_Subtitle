CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (position('@' in email) > 1)
);

CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    local_job_id TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    storage_dir TEXT NOT NULL,
    input_file_id UUID,
    render_output_file_id UUID,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CHECK (
        status IN (
            'queued',
            'created',
            'extracting_audio',
            'transcribing',
            'refining_segments',
            'translating',
            'exporting_subtitles',
            'rendering_video',
            'done',
            'failed',
            'cancelled'
        )
    )
);

CREATE TABLE files (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    job_id UUID REFERENCES jobs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime_type TEXT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (size_bytes >= 0),
    CHECK (kind IN ('input', 'audio', 'segments', 'subtitle', 'render', 'glossary', 'other'))
);

ALTER TABLE jobs
    ADD CONSTRAINT jobs_input_file_fk
    FOREIGN KEY (input_file_id) REFERENCES files(id) ON DELETE SET NULL;

ALTER TABLE jobs
    ADD CONSTRAINT jobs_render_output_file_fk
    FOREIGN KEY (render_output_file_id) REFERENCES files(id) ON DELETE SET NULL;

CREATE TABLE subtitle_segments (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    segment_index INTEGER NOT NULL,
    start_ms BIGINT NOT NULL,
    end_ms BIGINT NOT NULL,
    ja_text TEXT NOT NULL,
    zh_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (job_id, segment_index),
    CHECK (segment_index >= 0),
    CHECK (start_ms >= 0),
    CHECK (end_ms > start_ms)
);

CREATE TABLE glossaries (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, name),
    CHECK (length(trim(name)) > 0)
);

CREATE TABLE glossary_terms (
    id UUID PRIMARY KEY,
    glossary_id UUID NOT NULL REFERENCES glossaries(id) ON DELETE CASCADE,
    source_text TEXT NOT NULL,
    target_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (glossary_id, source_text, target_text),
    CHECK (length(trim(source_text)) > 0)
);

CREATE TABLE job_events (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX users_email_idx ON users (email);
CREATE INDEX jobs_user_created_idx ON jobs (user_id, created_at DESC);
CREATE INDEX jobs_status_created_idx ON jobs (status, created_at);
CREATE INDEX files_user_job_idx ON files (user_id, job_id);
CREATE INDEX subtitle_segments_job_index_idx ON subtitle_segments (job_id, segment_index);
CREATE INDEX glossaries_user_name_idx ON glossaries (user_id, name);
CREATE INDEX glossary_terms_glossary_idx ON glossary_terms (glossary_id);
CREATE INDEX job_events_job_created_idx ON job_events (job_id, created_at);
