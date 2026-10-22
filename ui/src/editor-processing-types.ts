import type { SubtitleSegment } from "./editor-types";

export type RetranscriptionCandidate = {
  id: string;
  start_ms: number;
  end_ms: number;
  source_text: string;
  translated_text: string | null;
  source_edited: boolean;
  translation_stale: boolean;
};

export type RetranscriptionPreview = {
  comparison_id?: string | null;
  workspace_revision?: number | null;
  preview_id: string;
  job_id: string;
  start_ms: number;
  end_ms: number;
  original_segments: SubtitleSegment[];
  candidate_segments: RetranscriptionCandidate[];
  context_mode: "isolated" | "provider-managed";
  provider_name?: string;
  model_identity?: string;
  timing_granularities?: string[];
  configuration?: Record<string, unknown>;
};

export type AsrQualitySignal = {
  id: string;
  job_id: string;
  run_id: string;
  signal_kind: "repeated_loop" | "boundary_repetition" | "abnormal_speech_rate" | "speech_during_silence" | "manual_concern";
  severity: "info" | "warning" | "critical";
  start_ms: number | null;
  end_ms: number | null;
  message: string;
  latest_decision: "confirmed_problem" | "false_positive" | "resolved" | null;
  latest_decision_note: string | null;
};

export type ReviewDecision = "confirmed_problem" | "false_positive" | "resolved";
export type CandidateProgress = { phase: string; completed_ms?: number; duration_ms?: number };
export type PreviewRequest = { jobId: string; startMs: number; endMs: number; providerId: string; authorizeCloudAudioUpload: boolean; qualitySignalId: string | null };
