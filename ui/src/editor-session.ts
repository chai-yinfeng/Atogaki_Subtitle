/** UI selection is independent from playback and from the processing range. */
export type EditorCue = { id: string; start_ms: number; end_ms: number; source_text: string; translated_text: string | null; translation_stale: boolean };
export type CueFilter = "all" | "untranslated" | "stale" | "review";
export class EditorSession {
  jobId: string | null = null;
  selectedCueId: string | null = null;
  playbackCueId: string | null = null;
  processingRange: { startMs: number; endMs: number } | null = null;
  timelineViewport = { startMs: 0, endMs: 30_000 };
  query = "";
  filter: CueFilter = "all";
  followPlayback = true;
  open(jobId: string, cues: readonly EditorCue[]): void {
    if (jobId !== this.jobId) {
      this.jobId = jobId;
      this.selectedCueId = cues[0]?.id ?? null;
      this.playbackCueId = null;
      this.processingRange = null;
      this.timelineViewport = { startMs: 0, endMs: 30_000 };
      this.query = ""; this.filter = "all"; this.followPlayback = true;
    }
    this.reconcile(cues);
  }
  reconcile(cues: readonly EditorCue[], preferredId?: string): void {
    if (preferredId && cues.some(cue => cue.id === preferredId)) this.selectedCueId = preferredId;
    else if (!cues.some(cue => cue.id === this.selectedCueId)) this.selectedCueId = cues[0]?.id ?? null;
  }
  visible<T extends EditorCue>(cues: readonly T[], reviewIds: ReadonlySet<string>, staleIds: readonly string[] = []): T[] {
    const query = this.query.normalize("NFKC").trim().toLocaleLowerCase();
    const stale = new Set(staleIds);
    return cues.filter(cue => (!query || `${cue.source_text}\n${cue.translated_text ?? ""}`.normalize("NFKC").toLocaleLowerCase().includes(query))
      && (this.filter === "all" || this.filter === "untranslated" && !cue.translated_text?.trim()
        || this.filter === "stale" && (cue.translation_stale || stale.has(cue.id))
        || this.filter === "review" && reviewIds.has(cue.id)));
  }
  adjacent(cues: readonly EditorCue[], direction: number): string | null {
    if (!cues.length) return null;
    const index = cues.findIndex(cue => cue.id === this.selectedCueId);
    return cues[index < 0 ? (direction > 0 ? 0 : cues.length - 1) : Math.max(0, Math.min(cues.length - 1, index + direction))]?.id ?? null;
  }
}

/** Rebase a timing gesture after text autosave, without copying stale text. */
export function mergeDraggedCueTiming<T extends EditorCue>(current: readonly T[], initial: T, moved: T): T[] {
  const cue = current.find(cue => cue.id === initial.id);
  if (!cue || cue.start_ms !== initial.start_ms || cue.end_ms !== initial.end_ms) throw new Error("字幕时间已变化，请重新拖动");
  return current.map(cue => cue.id === moved.id ? { ...cue, start_ms: moved.start_ms, end_ms: moved.end_ms } : { ...cue });
}
