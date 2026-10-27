import type { SubtitleSegment, WaveformWindow } from "./editor-types";
import type { TranslationContextState } from "./editor-context-status";
import type { DraftText } from "./subtitle-drafts";

type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
/** Editor persistence adapter. Session/view modules never import Tauri. */
export function createEditorBackend<Detail = unknown>(invoke: Invoke) {
  return {
    detail(jobId: string) { return invoke<Detail>("get_job_detail", { jobId }); },
    contextStatus(jobId: string) { return invoke<TranslationContextState>("get_translation_context_state", { jobId }); },
    waveform(jobId: string, startMs: number, endMs: number, pointCount: number) {
      return invoke<WaveformWindow>("get_waveform_window", { request: { jobId, startMs, endMs, pointCount } });
    },
    async updateText(before: SubtitleSegment, value: DraftText): Promise<SubtitleSegment> {
      if (!value.source.trim()) throw new Error("原文不能为空；草稿已保留");
      return invoke<SubtitleSegment>("update_subtitle", { request: {
        jobId: before.job_id, segmentId: before.id, sourceText: value.source,
        translatedText: value.translation.trim() || null, startMs: before.start_ms, endMs: before.end_ms,
      } });
    },
    restoreText(snapshot: SubtitleSegment) { return invoke<SubtitleSegment>("restore_subtitle", { snapshot }); },
  };
}
