ALTER TABLE local_jobs
ADD COLUMN workspace_revision INTEGER NOT NULL DEFAULT 0;

CREATE TABLE local_subtitle_provenance (
    segment_id TEXT PRIMARY KEY REFERENCES local_subtitle_segments(id) ON DELETE CASCADE,
    job_id TEXT NOT NULL REFERENCES local_jobs(job_id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL DEFAULT 'legacy'
        CHECK (source_kind IN ('legacy', 'asr', 'manual', 'structural')),
    source_run_id TEXT REFERENCES local_asr_runs(id) ON DELETE SET NULL,
    timed_unit_ids_json TEXT NOT NULL DEFAULT '[]',
    timing_source TEXT NOT NULL DEFAULT 'legacy'
        CHECK (timing_source IN ('legacy', 'model', 'forced_alignment', 'interpolated', 'manual', 'unknown')),
    alignment_status TEXT NOT NULL DEFAULT 'unknown'
        CHECK (alignment_status IN ('current', 'stale', 'unknown')),
    source_revision INTEGER NOT NULL DEFAULT 0 CHECK (source_revision >= 0),
    translation_revision INTEGER NOT NULL DEFAULT 0 CHECK (translation_revision >= 0),
    translation_dependency_stale INTEGER NOT NULL DEFAULT 0
        CHECK (translation_dependency_stale IN (0, 1)),
    updated_at_unix INTEGER NOT NULL
);

CREATE TABLE local_translation_dependency_edges (
    target_segment_id TEXT NOT NULL REFERENCES local_subtitle_segments(id) ON DELETE CASCADE,
    context_segment_id TEXT NOT NULL REFERENCES local_subtitle_segments(id) ON DELETE CASCADE,
    context_source_revision INTEGER NOT NULL CHECK (context_source_revision >= 0),
    PRIMARY KEY (target_segment_id, context_segment_id),
    CHECK (target_segment_id != context_segment_id)
);

INSERT INTO local_subtitle_provenance (segment_id, job_id, updated_at_unix)
SELECT id, job_id, CAST(strftime('%s', 'now') AS INTEGER)
FROM local_subtitle_segments;

CREATE TRIGGER local_subtitle_provenance_after_insert
AFTER INSERT ON local_subtitle_segments
BEGIN
    INSERT INTO local_subtitle_provenance (segment_id, job_id, updated_at_unix)
    VALUES (NEW.id, NEW.job_id, CAST(strftime('%s', 'now') AS INTEGER));
    UPDATE local_jobs
    SET workspace_revision = workspace_revision + 1
    WHERE job_id = NEW.job_id;
END;

CREATE TRIGGER local_subtitle_provenance_after_source_update
AFTER UPDATE OF source_text, start_ms, end_ms ON local_subtitle_segments
WHEN OLD.source_text != NEW.source_text
  OR OLD.start_ms != NEW.start_ms
  OR OLD.end_ms != NEW.end_ms
BEGIN
    UPDATE local_subtitle_provenance
    SET source_kind = CASE WHEN OLD.source_text != NEW.source_text THEN 'manual' ELSE source_kind END,
        timing_source = CASE
            WHEN OLD.start_ms != NEW.start_ms OR OLD.end_ms != NEW.end_ms THEN 'manual'
            ELSE timing_source
        END,
        alignment_status = CASE WHEN OLD.source_text != NEW.source_text THEN 'stale' ELSE alignment_status END,
        source_revision = source_revision + 1,
        updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
    WHERE segment_id = NEW.id;
    UPDATE local_subtitle_provenance
    SET translation_dependency_stale = 1,
        updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
    WHERE segment_id IN (
        SELECT target_segment_id
        FROM local_translation_dependency_edges
        WHERE context_segment_id = NEW.id
    );
    UPDATE local_jobs
    SET workspace_revision = workspace_revision + 1
    WHERE job_id = NEW.job_id;
END;

CREATE TRIGGER local_subtitle_provenance_after_translation_update
AFTER UPDATE OF translated_text ON local_subtitle_segments
WHEN OLD.translated_text IS NOT NEW.translated_text
BEGIN
    UPDATE local_subtitle_provenance
    SET translation_revision = translation_revision + 1,
        translation_dependency_stale = 0,
        updated_at_unix = CAST(strftime('%s', 'now') AS INTEGER)
    WHERE segment_id = NEW.id;
    DELETE FROM local_translation_dependency_edges
    WHERE target_segment_id = NEW.id;
    UPDATE local_jobs
    SET workspace_revision = workspace_revision + 1
    WHERE job_id = NEW.job_id;
END;

CREATE INDEX local_subtitle_provenance_job_idx
ON local_subtitle_provenance (job_id);

CREATE INDEX local_translation_dependency_context_idx
ON local_translation_dependency_edges (context_segment_id);
