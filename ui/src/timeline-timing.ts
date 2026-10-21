type Cue = { id: string; start_ms: number; end_ms: number };
export type TimingGesture<T> = {
  jobId: string; pointerId: number; segmentIndex: number; mode: "move" | "start" | "end";
  pointerStartX: number; width: number; rangeMs: number;
  before: T[]; draft: T[]; moved: boolean;
};

/** Preview only the dragged cue; persistence still rebases against live text. */
export function updateTimingPreview<T extends Cue>(drag: TimingGesture<T>, clientX: number, duration: number): void {
  const delta = Math.round((clientX - drag.pointerStartX) / Math.max(1, drag.width) * drag.rangeMs / 10) * 10;
  const original = drag.before[drag.segmentIndex], cue = drag.draft[drag.segmentIndex];
  const previous = drag.before[drag.segmentIndex - 1], next = drag.before[drag.segmentIndex + 1];
  const limit = duration || Number.MAX_SAFE_INTEGER;
  if (drag.mode === "move") {
    const bounded = Math.max((previous?.end_ms ?? 0) - original.start_ms, Math.min((next?.start_ms ?? limit) - original.end_ms, delta));
    cue.start_ms = original.start_ms + bounded; cue.end_ms = original.end_ms + bounded;
  } else if (drag.mode === "start") {
    cue.start_ms = Math.max(previous?.end_ms ?? 0, Math.min(original.end_ms - 10, original.start_ms + delta));
  } else {
    cue.end_ms = Math.max(original.start_ms + 10, Math.min(next?.start_ms ?? limit, original.end_ms + delta));
  }
  drag.moved = cue.start_ms !== original.start_ms || cue.end_ms !== original.end_ms;
}

