export type MediaCapabilities = {
  binary_path: string;
  version: string;
  ass_filter: boolean;
  videotoolbox_encoder: boolean;
  mpeg4_encoder: boolean;
  ready_for_hard_subtitles: boolean;
  ready_for_quality_render: boolean;
  ready_for_prores: boolean;
  ready_for_soft_matroska: boolean;
};

export type VideoExportMode = "burn" | "soft_matroska";
export type VideoRenderQuality = "compact" | "balanced" | "high" | "quality" | "soft_matroska" | "prores";

export function videoOutputExtension(quality: VideoRenderQuality): string {
  if (quality === "soft_matroska") return "mkv";
  if (quality === "prores") return "mov";
  return "mp4";
}

export function videoProfileAvailable(quality: VideoRenderQuality, capabilities: MediaCapabilities): boolean {
  if (quality === "soft_matroska") return capabilities.ready_for_soft_matroska;
  if (quality === "prores") return capabilities.ready_for_prores;
  if (quality === "quality") return capabilities.ready_for_quality_render;
  return capabilities.ready_for_hard_subtitles;
}
export type VideoRenderEstimate = {
  quality: VideoRenderQuality;
  width: number | null;
  height: number | null;
  duration_ms: number;
  target_video_bitrate_bps: number | null;
  hdr_video: boolean;
  estimated_audio_bitrate_bps: number;
  estimated_size_bytes: number | null;
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
