ALTER TABLE local_glossaries ADD COLUMN builtin_key TEXT;
ALTER TABLE local_glossaries ADD COLUMN builtin_version TEXT;

CREATE UNIQUE INDEX local_glossaries_builtin_key_idx
    ON local_glossaries (builtin_key)
    WHERE builtin_key IS NOT NULL;

ALTER TABLE local_glossary_terms RENAME TO local_glossary_terms_legacy;

CREATE TABLE local_glossary_terms (
    id TEXT PRIMARY KEY,
    glossary_id TEXT NOT NULL REFERENCES local_glossaries(id) ON DELETE CASCADE,
    source_text TEXT NOT NULL,
    target_text TEXT,
    prompt_scope TEXT NOT NULL DEFAULT 'core'
        CHECK (prompt_scope IN ('core', 'content', 'correction_only')),
    content_group TEXT,
    CHECK (length(trim(source_text)) > 0)
);

INSERT INTO local_glossary_terms
    (id, glossary_id, source_text, target_text, prompt_scope, content_group)
SELECT id, glossary_id, source_text, target_text, prompt_scope, content_group
FROM local_glossary_terms_legacy;

DROP TABLE local_glossary_terms_legacy;

CREATE INDEX local_glossary_terms_glossary_idx
    ON local_glossary_terms (glossary_id);
CREATE INDEX local_glossary_terms_scope_idx
    ON local_glossary_terms (glossary_id, prompt_scope, content_group);
CREATE UNIQUE INDEX local_glossary_terms_identity_idx
    ON local_glossary_terms (
        glossary_id,
        source_text,
        COALESCE(target_text, ''),
        prompt_scope,
        COALESCE(content_group, '')
    );
