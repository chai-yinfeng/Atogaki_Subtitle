DELETE FROM local_glossary_terms AS prompt
WHERE prompt.target_text IS NULL
  AND EXISTS (
      SELECT 1
      FROM local_glossary_terms AS correction
      WHERE correction.glossary_id = prompt.glossary_id
        AND correction.target_text = prompt.source_text
  );
