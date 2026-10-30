/** Preserve aligned edges; in gaps use the first boundary outward, or media ends. */
export function alignRepairRange(cues: readonly { start_ms: number; end_ms: number }[], range: { startMs: number; endMs: number }, durationMs: number) {
  const boundaries = [0, durationMs, ...cues.flatMap(cue => [cue.start_ms, cue.end_ms])];
  let startMs = Math.max(...boundaries.filter(ms => ms <= range.startMs));
  let endMs = Math.min(...boundaries.filter(ms => ms >= range.endMs));
  return { startMs, endMs };
}
