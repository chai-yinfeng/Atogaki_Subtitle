ALTER TABLE local_subtitle_segments
    ADD COLUMN timing_edited INTEGER NOT NULL DEFAULT 0
    CHECK (timing_edited IN (0, 1));
