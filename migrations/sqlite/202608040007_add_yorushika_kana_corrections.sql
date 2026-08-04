INSERT OR IGNORE INTO local_glossary_terms
    (id, glossary_id, source_text, target_text, prompt_scope, content_group)
SELECT
    'builtin-yorushika-suis-hiragana',
    id,
    'すい',
    'suis',
    'correction_only',
    NULL
FROM local_glossaries AS glossary
WHERE name = 'Yorushika'
  AND NOT EXISTS (
      SELECT 1
      FROM local_glossary_terms AS existing
      WHERE existing.glossary_id = glossary.id
        AND existing.source_text = 'すい'
  );

INSERT OR IGNORE INTO local_glossary_terms
    (id, glossary_id, source_text, target_text, prompt_scope, content_group)
SELECT
    'builtin-yorushika-nbuna-hiragana',
    id,
    'なぶな',
    'n-buna',
    'correction_only',
    NULL
FROM local_glossaries AS glossary
WHERE name = 'Yorushika'
  AND NOT EXISTS (
      SELECT 1
      FROM local_glossary_terms AS existing
      WHERE existing.glossary_id = glossary.id
        AND existing.source_text = 'なぶな'
  );
