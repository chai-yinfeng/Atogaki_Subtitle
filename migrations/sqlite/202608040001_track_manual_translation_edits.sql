ALTER TABLE local_subtitle_segments
    ADD COLUMN translation_edited INTEGER NOT NULL DEFAULT 0
    CHECK (translation_edited IN (0, 1));
