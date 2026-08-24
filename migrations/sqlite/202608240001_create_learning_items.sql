CREATE TABLE local_learning_items (
    id TEXT PRIMARY KEY,
    source_language TEXT NOT NULL,
    target_language TEXT NOT NULL,
    item_type TEXT NOT NULL,
    source_text TEXT NOT NULL,
    source_key TEXT NOT NULL,
    meaning_text TEXT,
    meaning_provider_id TEXT,
    meaning_source_label TEXT,
    created_at_unix INTEGER NOT NULL,
    updated_at_unix INTEGER NOT NULL,
    UNIQUE (source_language, target_language, item_type, source_key),
    CHECK (item_type IN ('selection', 'sentence')),
    CHECK (length(trim(source_text)) > 0)
);

CREATE TABLE local_learning_occurrences (
    id TEXT PRIMARY KEY,
    learning_item_id TEXT NOT NULL REFERENCES local_learning_items(id) ON DELETE CASCADE,
    job_id TEXT REFERENCES local_jobs(job_id) ON DELETE SET NULL,
    segment_id TEXT REFERENCES local_subtitle_segments(id) ON DELETE SET NULL,
    job_display_name_snapshot TEXT NOT NULL,
    segment_source_snapshot TEXT NOT NULL,
    segment_translation_snapshot TEXT,
    selection_start_utf16 INTEGER NOT NULL,
    selection_end_utf16 INTEGER NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    created_at_unix INTEGER NOT NULL,
    UNIQUE (learning_item_id, job_id, segment_id, selection_start_utf16, selection_end_utf16),
    CHECK (selection_start_utf16 >= 0),
    CHECK (selection_end_utf16 > selection_start_utf16),
    CHECK (start_ms >= 0),
    CHECK (end_ms > start_ms)
);

CREATE INDEX local_learning_items_updated_idx
    ON local_learning_items (updated_at_unix DESC, id DESC);

CREATE INDEX local_learning_occurrences_item_idx
    ON local_learning_occurrences (learning_item_id, created_at_unix DESC, id DESC);

CREATE INDEX local_learning_occurrences_source_idx
    ON local_learning_occurrences (job_id, segment_id);
