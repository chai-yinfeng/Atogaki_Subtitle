import type { SubtitleSegment } from "./editor-types";

export function needsTranslation(cue: SubtitleSegment, contextStaleIds: readonly string[]) {
  return cue.translation_stale || !cue.translated_text?.trim() || contextStaleIds.includes(cue.id);
}

/** Count actual cues once, including translations invalidated by context edits. */
export function staleTranslationCount(cues: readonly Pick<SubtitleSegment, "id" | "translation_stale">[], contextStaleIds: readonly string[]) {
  const staleContext = new Set(contextStaleIds);
  return cues.reduce((count, cue) => count + Number(cue.translation_stale || staleContext.has(cue.id)), 0);
}
