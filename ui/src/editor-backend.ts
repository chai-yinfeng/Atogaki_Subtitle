import type { SubtitleSegment } from "./editor-types";
import type { DraftText } from "./subtitle-drafts";

type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
/** Editor persistence adapter. Session/view modules never import Tauri. */
export function createEditorBackend(invoke: Invoke) {
  return {
    async updateText(before: SubtitleSegment, value: DraftText): Promise<SubtitleSegment> {
      if (!value.source.trim()) throw new Error("原文不能为空；草稿已保留");
      return invoke<SubtitleSegment>("update_subtitle", { request: {
        jobId: before.job_id, segmentId: before.id, sourceText: value.source,
        translatedText: value.translation.trim() || null, startMs: before.start_ms, endMs: before.end_ms,
      } });
    },
    restoreText(snapshot: SubtitleSegment) { return invoke<SubtitleSegment>("restore_subtitle", { snapshot }); },
    saveTiming(jobId: string, beforeSegments: SubtitleSegment[], afterSegments: SubtitleSegment[]) {
      return invoke<SubtitleSegment[]>("save_subtitle_timing", { request: { jobId, beforeSegments, afterSegments } });
    },
  };
}
