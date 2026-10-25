import type { SubtitleSegment, WaveformWindow } from "./editor-types";
import type { DraftText } from "./subtitle-drafts";

type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
/** Editor persistence adapter. Session/view modules never import Tauri. */
export function createEditorBackend(invoke: Invoke) {
  return {
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
    saveTiming(jobId: string, beforeSegments: SubtitleSegment[], afterSegments: SubtitleSegment[]) {
      return invoke<SubtitleSegment[]>("save_subtitle_timing", { request: { jobId, beforeSegments, afterSegments } });
    },
    split(request: { jobId: string; segmentId: string; boundaryMs: number; leftSourceText: string; rightSourceText: string; leftTranslatedText: string | null; rightTranslatedText: string | null }) {
      return invoke<SubtitleSegment[]>("split_subtitle", { request });
    },
    merge(request: { jobId: string; leftSegmentId: string; rightSegmentId: string; sourceText: string; translatedText: string | null }) {
      return invoke<SubtitleSegment[]>("merge_subtitles", { request });
    },
    restoreStructure(jobId: string, beforeSegments: SubtitleSegment[], afterSegments: SubtitleSegment[]) {
      return invoke<SubtitleSegment[]>("restore_subtitle_structure", { request: { jobId, beforeSegments, afterSegments } });
    },
    editBlock(jobId: string, beforeSegments: SubtitleSegment[], removeId: string | null, insertion: SubtitleSegment | null) {
      return invoke<SubtitleSegment[]>("edit_subtitle_block", { request: { jobId, beforeSegments, removeId, insertion } });
    },
  };
}
