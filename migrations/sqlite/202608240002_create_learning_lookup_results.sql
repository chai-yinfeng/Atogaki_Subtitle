CREATE TABLE local_learning_lookup_results (
    id TEXT PRIMARY KEY,
    learning_item_id TEXT NOT NULL REFERENCES local_learning_items(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    headword TEXT NOT NULL,
    reading TEXT,
    pronunciation TEXT,
    senses_json TEXT NOT NULL,
    attribution_text TEXT NOT NULL,
    source_url TEXT,
    license_label TEXT,
    data_version TEXT,
    fetched_at_unix INTEGER NOT NULL,
    cache_expires_at_unix INTEGER,
    UNIQUE (learning_item_id, provider_id),
    CHECK (length(trim(provider_id)) > 0),
    CHECK (length(trim(provider_name)) > 0),
    CHECK (length(trim(headword)) > 0),
    CHECK (length(trim(attribution_text)) > 0)
);

CREATE INDEX local_learning_lookup_results_item_idx
    ON local_learning_lookup_results (learning_item_id, provider_id);
