ALTER TABLE local_glossary_terms
    ADD COLUMN prompt_scope TEXT NOT NULL DEFAULT 'core'
    CHECK (prompt_scope IN ('core', 'content', 'correction_only'));

ALTER TABLE local_glossary_terms
    ADD COLUMN content_group TEXT;

CREATE INDEX local_glossary_terms_scope_idx
    ON local_glossary_terms (glossary_id, prompt_scope, content_group);
