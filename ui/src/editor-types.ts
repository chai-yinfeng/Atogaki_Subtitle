export type SubtitleSegment = {
  id: string;
  job_id: string;
  segment_index: number;
  start_ms: number;
  end_ms: number;
  source_text: string;
  translated_text: string | null;
  source_edited: boolean;
  translation_edited: boolean;
  translation_stale: boolean;
  timing_edited: boolean;
};

export type WaveformWindow = {
  duration_ms: number; start_ms: number; end_ms: number;
  point_duration_ms: number; peaks: { min: number; max: number }[];
};
