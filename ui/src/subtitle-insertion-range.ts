type CueTiming = { start_ms: number; end_ms: number };
export type InsertionRange = { startMs: number; endMs: number; durationMs: number };

/** Suggest up to two seconds after the playing cue, without skipping adjacent cues. */
export function suggestSubtitleInsertionRange(cues: readonly CueTiming[], positionMs: number, durationMs: number): InsertionRange | null {
  if (!Number.isFinite(durationMs) || durationMs <= 0 || !Number.isFinite(positionMs) || positionMs < 0) return null;
  const position = Math.min(durationMs, Math.round(positionMs / 10) * 10);
  const current = cues.find(cue => cue.start_ms <= position && cue.end_ms > position);
  const startMs = Math.min(durationMs, current?.end_ms ?? position);
  const next = cues.find(cue => cue.start_ms >= startMs);
  return { startMs, endMs: Math.min(startMs + 2_000, next?.start_ms ?? durationMs, durationMs), durationMs };
}
