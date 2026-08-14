ALTER TABLE local_jobs
ADD COLUMN subtitle_structure_edited INTEGER NOT NULL DEFAULT 0
CHECK (subtitle_structure_edited IN (0, 1));
