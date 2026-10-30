import type { SubtitleSegment, WaveformWindow } from "./editor-types";
export type TimelineSnapshot = {
  waveform: WaveformWindow | null;
  viewport?: { startMs: number; endMs: number };
  cues: readonly SubtitleSegment[];
  selectedCueId: string | null;
  draggedCueId: string | null;
  playbackCueId: string | null;
  gain: number;
  currentMs: number;
  emptyMessage: string;
};
/** Shared by paint and pointer hit testing, including compact canvas heights. */
export function timelineGeometry(height: number) {
  const waveformTop = 28;
  const waveformHeight = Math.max(12, (height - 56) * 0.5);
  const trackTop = waveformTop + waveformHeight + (height < 100 ? 6 : 14);
  return { waveformTop, waveformHeight, trackTop, trackHeight: Math.max(14, height - trackTop - 4) };
}
/** Resolve close handles by distance, rather than whichever cue is last. */
export function timelineCueHit(cues: readonly { start_ms: number; end_ms: number }[], x: number, width: number, start: number, end: number): { segmentIndex: number; mode: "start" | "end" | "move" } | null {
  if (width <= 0 || end <= start || x < 0 || x > width) return null;
  let edge: { segmentIndex: number; mode: "start" | "end"; distance: number; inside: boolean } | null = null;
  let body: { segmentIndex: number; mode: "move" } | null = null;
  cues.forEach((cue, segmentIndex) => {
    const left = (cue.start_ms - start) / (end - start) * width;
    const right = (cue.end_ms - start) / (end - start) * width;
    const inside = x >= left && x <= right;
    if (inside) body = { segmentIndex, mode: "move" };
    for (const [mode, position] of [["start", left], ["end", right]] as const) {
      const distance = Math.abs(x - position);
      if (distance <= 8 && (!edge || distance < edge.distance || distance === edge.distance && inside && !edge.inside)) edge = { segmentIndex, mode, distance, inside };
    }
  });
  return edge ?? body;
}
/** Own the static layer, playhead drawing and resize observer; no backend calls. */
export function mountEditorTimeline(canvas: HTMLCanvasElement, styleRoot: HTMLElement,
  options: { time(ms: number): string; resized(): void }) {
  let bitmapCache: { waveform: WaveformWindow; key: string; canvas: HTMLCanvasElement } | null = null;
  let disposed = false;
  const clear = () => {
    if (bitmapCache) { bitmapCache.canvas.width = 0; bitmapCache.canvas.height = 0; }
    bitmapCache = null;
  };
  const observer = new ResizeObserver(() => {
    if (!disposed && canvas.getBoundingClientRect().width > 0 && canvas.getBoundingClientRect().height > 0) options.resized();
  });
  observer.observe(canvas);
  const render = (snapshot: TimelineSnapshot) => {
    if (disposed) return;
    const bounds = canvas.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return;
    const scale = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.round(bounds.width * scale));
    const height = Math.max(1, Math.round(bounds.height * scale));
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }
    const theme = getComputedStyle(styleRoot);
    const accent = theme.getPropertyValue("--accent").trim();
    const accentSoft = theme.getPropertyValue("--accent-soft").trim();
    const context = canvas.getContext("2d");
    if (!context) return;
    context.setTransform(scale, 0, 0, scale, 0, 0);
    const cssWidth = width / scale;
    const cssHeight = height / scale;
    context.clearRect(0, 0, cssWidth, cssHeight);
    context.fillStyle = theme.getPropertyValue("--surface-soft").trim() || "#f8f5ec";
    context.fillRect(0, 0, cssWidth, cssHeight);
    const { waveform, cues: timelineSegments, selectedCueId: activeId, draggedCueId, playbackCueId, gain, currentMs, emptyMessage } = snapshot;
    if (!waveform || waveform.peaks.length === 0) {
      context.fillStyle = "#777a72";
      context.font = "13px system-ui";
      context.textAlign = "center";
      clear();
      context.fillText(emptyMessage, cssWidth / 2, cssHeight / 2);
      return;
    }

    const start = snapshot.viewport?.startMs ?? waveform.start_ms;
    const end = snapshot.viewport?.endMs ?? waveform.end_ms;
    const bitmapKey = JSON.stringify([width, height, start, end, gain, accent, accentSoft,
      theme.getPropertyValue("--ink"), theme.getPropertyValue("--surface-soft"), activeId,
      draggedCueId,
      timelineSegments.map(cue => [cue.id, cue.start_ms, cue.end_ms, cue.source_text])]);
    const range = end - start;
    const rulerHeight = 28;
    if (bitmapCache?.waveform === waveform && bitmapCache.key === bitmapKey) {
      context.drawImage(bitmapCache.canvas, 0, 0, cssWidth, cssHeight);
    } else {
      const { waveformTop, waveformHeight, trackTop, trackHeight } = timelineGeometry(cssHeight);
      const centerY = waveformTop + waveformHeight / 2;
      context.strokeStyle = "#d6d0c3";
      context.lineWidth = 1;
      context.beginPath();
      context.moveTo(0, centerY);
      context.lineTo(cssWidth, centerY);
      context.stroke();
      context.strokeStyle = accent;
      context.lineWidth = Math.max(1, cssWidth * (waveform.end_ms - waveform.start_ms) / range / waveform.peaks.length);
      context.beginPath();
      waveform.peaks.forEach((peak, index) => {
        const time = waveform.start_ms + (index + 0.5) / waveform.peaks.length * (waveform.end_ms - waveform.start_ms);
        const x = (time - start) / range * cssWidth;
        if (x < 0 || x > cssWidth) return;
        const minimum = Math.max(-1, peak.min * gain);
        const maximum = Math.min(1, peak.max * gain);
        context.moveTo(x, centerY + minimum * waveformHeight * 0.46);
        context.lineTo(x, centerY + maximum * waveformHeight * 0.46);
      });
      context.stroke();

      context.fillStyle = "#74776f";
      context.font = "10px ui-monospace, monospace";
      context.textAlign = "center";
      for (let marker = 0; marker <= 5; marker += 1) {
        const x = marker / 5 * cssWidth;
        const time = start + range * marker / 5;
        context.fillText(options.time(time), Math.min(cssWidth - 45, Math.max(45, x)), 17);
        context.strokeStyle = "rgba(112, 115, 106, .2)";
        context.beginPath();
        context.moveTo(x, rulerHeight - 5);
        context.lineTo(x, cssHeight);
        context.stroke();
      }

      for (const segment of timelineSegments) {
        if (segment.end_ms < start || segment.start_ms > end) continue;
        const left = Math.max(0, (segment.start_ms - start) / range * cssWidth);
        const right = Math.min(cssWidth, (segment.end_ms - start) / range * cssWidth);
        const active = segment.id === activeId;
        context.fillStyle = active ? accent : accentSoft;
        context.strokeStyle = accent;
        context.lineWidth = active ? 2 : 1;
        context.beginPath();
        context.roundRect(left + 1, trackTop, Math.max(3, right - left - 2), trackHeight, 5);
        context.fill();
        context.stroke();
        const dragged = segment.id === draggedCueId;
        context.strokeStyle = dragged ? "#b34b36" : accent;
        context.lineWidth = dragged ? 3 : 2;
        context.beginPath();
        context.moveTo(left + 3, trackTop + 5);
        context.lineTo(left + 3, trackTop + trackHeight - 5);
        context.moveTo(right - 3, trackTop + 5);
        context.lineTo(right - 3, trackTop + trackHeight - 5);
        context.stroke();
        if (right - left > 45) {
          context.save();
          context.beginPath();
          context.rect(left + 5, trackTop + 2, Math.max(1, right - left - 10), trackHeight - 4);
          context.clip();
          context.fillStyle = active ? "#fffaf1" : theme.getPropertyValue("--ink").trim();
          context.font = "11px system-ui";
          context.textAlign = "left";
          context.fillText(segment.source_text, left + 7, trackTop + trackHeight / 2 + 4);
          context.restore();
        }
      }

      for (let index = 0; index + 1 < timelineSegments.length; index += 1) {
        const left = timelineSegments[index];
        const right = timelineSegments[index + 1];
        if (left.end_ms <= right.start_ms || left.end_ms < start || right.start_ms > end) continue;
        const warningLeft = Math.max(0, (right.start_ms - start) / range * cssWidth);
        const warningRight = Math.min(cssWidth, (left.end_ms - start) / range * cssWidth);
        context.fillStyle = "rgba(179, 75, 54, .3)";
        context.fillRect(warningLeft, trackTop, Math.max(2, warningRight - warningLeft), trackHeight);
      }

      const bitmap = bitmapCache?.canvas ?? document.createElement("canvas");
      if (bitmap.width !== width || bitmap.height !== height) { bitmap.width = width; bitmap.height = height; }
      const bitmapContext = bitmap.getContext("2d");
      bitmapContext?.clearRect(0, 0, width, height);
      bitmapContext?.drawImage(canvas, 0, 0);
      bitmapCache = { waveform, key: bitmapKey, canvas: bitmap };
    }

    const playing = timelineSegments.find(cue => cue.id === playbackCueId);
    if (playing) {
      const { trackTop, trackHeight } = timelineGeometry(cssHeight);
      const left = Math.max(0, (playing.start_ms - start) / range * cssWidth);
      const right = Math.min(cssWidth, (playing.end_ms - start) / range * cssWidth);
      context.strokeStyle = "#b07800"; context.lineWidth = 2;
      context.strokeRect(left + 1, trackTop + 1, Math.max(2, right - left - 2), Math.max(2, trackHeight - 2));
    }
    if (currentMs >= start && currentMs <= end) {
      const playheadX = (currentMs - start) / range * cssWidth;
      context.strokeStyle = "#b34b36";
      context.lineWidth = 2;
      context.beginPath();
      context.moveTo(playheadX, rulerHeight - 4);
      context.lineTo(playheadX, cssHeight);
      context.stroke();
      context.fillStyle = "#b34b36";
      context.beginPath();
      context.moveTo(playheadX - 6, rulerHeight - 5);
      context.lineTo(playheadX + 6, rulerHeight - 5);
      context.lineTo(playheadX, rulerHeight + 4);
      context.closePath();
      context.fill();
    }
  };
  return { render, clear, dispose() { disposed = true; observer.disconnect(); clear(); } };
}
