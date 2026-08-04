CREATE TABLE local_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at_unix INTEGER NOT NULL,
    CHECK (length(trim(key)) > 0)
);
