ALTER TABLE jobs
    ADD COLUMN source_language TEXT NOT NULL DEFAULT 'ja',
    ADD COLUMN target_language TEXT NOT NULL DEFAULT 'zh-Hans';

ALTER TABLE subtitle_segments
    RENAME COLUMN ja_text TO source_text;

ALTER TABLE subtitle_segments
    RENAME COLUMN zh_text TO translated_text;

ALTER TABLE glossaries
    ADD COLUMN source_language TEXT NOT NULL DEFAULT 'ja';

CREATE INDEX jobs_languages_idx ON jobs (source_language, target_language);
CREATE INDEX glossaries_language_idx ON glossaries (source_language, updated_at DESC);
