import type { SubtitleSegment } from "./editor-types";
import type { StructureEditResult, StructureEditState, StructureUndoResult } from "./editor-structure-types";

type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
/** Structure IPC shared by every editor operation and the sole token history. */
export function createStructureEditBackend(invoke: Invoke) {
  return {
    state(jobId: string) { return invoke<StructureEditState>("get_structure_edit_state", { jobId }); },
    saveTiming(jobId: string, beforeSegments: SubtitleSegment[], afterSegments: SubtitleSegment[]) {
      return invoke<StructureEditResult>("save_subtitle_timing_recorded", { request: { jobId, beforeSegments, afterSegments } });
    },
    split(request: { jobId: string; expectedRevision: number; segmentId: string; boundaryMs: number; leftSourceText: string; rightSourceText: string; leftTranslatedText: string | null; rightTranslatedText: string | null }) {
      return invoke<StructureEditResult>("split_subtitle_recorded", { request });
    },
    merge(request: { jobId: string; expectedRevision: number; leftSegmentId: string; rightSegmentId: string; sourceText: string; translatedText: string | null }) {
      return invoke<StructureEditResult>("merge_subtitles_recorded", { request });
    },
    editBlock(jobId: string, beforeSegments: SubtitleSegment[], removeId: string | null, insertion: SubtitleSegment | null) {
      return invoke<StructureEditResult>("edit_subtitle_block_recorded", { request: { jobId, beforeSegments, removeId, insertion } });
    },
    undo(jobId: string, token: string) {
      return invoke<StructureUndoResult>("undo_subtitle_structure", { request: { jobId, token } });
    },
    adopt(jobId: string, previewId: string, comparisonId: string | null) {
      return invoke<StructureEditResult>("confirm_retranscription_recorded", { request: { jobId, previewId, comparisonId } });
    },
  };
}
