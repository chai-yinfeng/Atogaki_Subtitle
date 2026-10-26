import type { SubtitleSegment } from "./editor-types";

/** Backend session descriptors only: provenance and dependency snapshots never cross IPC. */
export type StructureUndoDescriptor = { token: string; label: string; expected_revision: number };
export type StructureHistoryRecord = {
  latest: StructureUndoDescriptor | null; discarded_entries: number; oversized: boolean;
};
export type StructureEditResult = {
  segments: SubtitleSegment[]; workspace_revision: number; translation_context_stale_ids: string[];
  undo: StructureHistoryRecord; warnings: string[];
};
export type StructureUndoResult = {
  segments: SubtitleSegment[]; workspace_revision: number; translation_context_stale_ids: string[];
  latest: StructureUndoDescriptor | null; discarded_entries: number;
};
export type StructureEditState = {
  segments: SubtitleSegment[]; workspace_revision: number; latest: StructureUndoDescriptor | null;
};
