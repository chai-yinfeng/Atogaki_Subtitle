export type MediaCapabilities = {
  binary_path: string;
  version: string;
  ass_filter: boolean;
  videotoolbox_encoder: boolean;
  mpeg4_encoder: boolean;
  ready_for_hard_subtitles: boolean;
};

export type VideoRenderQuality = "compact" | "balanced" | "high";
export type VideoRenderEstimate = {
  quality: VideoRenderQuality;
  width: number | null;
  height: number | null;
  duration_ms: number;
  target_video_bitrate_bps: number;
  estimated_audio_bitrate_bps: number;
  estimated_size_bytes: number;
  bitrate_source: "source_bitrate" | "resolution_fallback";
};

export type VideoRender = {
  id: string;
  source_job_id: string;
  input_path: string;
  subtitle_path: string;
  output_path: string;
  subtitle_track: "source" | "translation" | "bilingual";
  status: "queued" | "running" | "done" | "failed" | "cancelled";
  progress: number;
  encoder: string | null;
  audio_encoder: string | null;
  fallback_reason: string | null;
  error_message: string | null;
  created_at_unix: number;
  updated_at_unix: number;
};

export type VideoOutputSelection = {
  path: string;
  alreadyExists: boolean;
};
