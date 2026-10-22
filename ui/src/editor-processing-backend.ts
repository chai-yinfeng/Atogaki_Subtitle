import type { CandidateRun } from "./editor-candidate-history";
import type { SubtitleSegment } from "./editor-types";
import type { AsrQualitySignal, CandidateProgress, PreviewRequest, RetranscriptionPreview, ReviewDecision } from "./editor-processing-types";
type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
/** Preserve command payloads and backend revision/transaction validation. */
export function createProcessingBackend(invoke: Invoke) {
  return {
    runs(jobId: string) { return invoke<CandidateRun[]>("list_asr_runs", { jobId }); },
    release(jobId: string, runId: string) { return invoke<number>("release_asr_candidate", { jobId, runId }); },
    preview(request: PreviewRequest) { return invoke<RetranscriptionPreview>("preview_retranscription", { request }); },
    full(jobId: string) { return invoke<RetranscriptionPreview>("preview_full_retranscription", { jobId }); },
    refresh(jobId: string, runId: string) { return invoke<RetranscriptionPreview>("refresh_asr_comparison", { jobId, runId }); },
    reopen(jobId: string, runId: string) { return invoke<RetranscriptionPreview>("reopen_asr_candidate", { jobId, runId }); },
    adopt(jobId: string, previewId: string, comparisonId: string | null) {
      return invoke<SubtitleSegment[]>("confirm_retranscription", { request: { jobId, previewId, comparisonId } });
    },
    cancel(jobId: string) { return invoke<void>("cancel_asr_candidate", { jobId }); },
    progress(jobId: string) { return invoke<CandidateProgress | null>("asr_candidate_progress", { jobId }); },
    signals(jobId: string) { return invoke<AsrQualitySignal[]>("list_asr_quality_signals", { jobId }); },
    review(signalId: string, decision: ReviewDecision) { return invoke<void>("save_asr_review_decision", { request: { signalId, decision, note: null } }); },
  };
}
