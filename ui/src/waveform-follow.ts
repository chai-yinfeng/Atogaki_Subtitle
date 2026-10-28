import type { WaveformWindow } from "./editor-types";

/** Only recenter when the clamped window can move; endpoints stay drawable. */
export function waveformFollowAnchor(positionMs: number, waveform: WaveformWindow, windowMs: number): number | null {
  const margin = (waveform.end_ms - waveform.start_ms) * 0.12;
  if (positionMs >= waveform.start_ms + margin && positionMs <= waveform.end_ms - margin) return null;
  const desired = Math.max(0, Math.min(Math.round(positionMs - windowMs / 2), Math.max(0, waveform.duration_ms - windowMs)));
  return desired === waveform.start_ms ? null : positionMs;
}
