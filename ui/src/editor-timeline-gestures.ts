import { updateTimingPreview, type TimingGesture } from "./timeline-timing";
import type { WaveformWindow } from "./editor-types";
import { playheadHandleContains, timelineTimeAtX } from "./offline-tools";
import { timelineCueHit, timelineGeometry } from "./editor-timeline-view";

type Cue = { id: string; start_ms: number; end_ms: number };
type Snapshot<T> = {
  jobId: string; cues: readonly T[]; waveform: WaveformWindow;
  currentMs: number; viewStartMs: number; durationMs: number;
};
type GestureEvent = Event & { scale: number; clientX: number };
type Options<T extends Cue> = {
  snapshot(): Snapshot<T> | null;
  select(id: string): boolean;
  pause(): void;
  seek(ms: number): void;
  stopFollowing(): void;
  pan(startMs: number): void;
  zoomValue(): number;
  zoom(value: number): void;
  zoomForDuration(ms: number): number;
  preview(drag: TimingGesture<T>): void;
  commit(drag: TimingGesture<T>): Promise<void>;
  cancelled(): void;
};

/** Own pointer capture, preview, pan and pinch state with one disposable mount. */
export function mountTimelineGestures<T extends Cue>(canvas: HTMLCanvasElement, zoomInput: HTMLInputElement, root: HTMLElement, options: Options<T>) {
  const events = new AbortController();
  let timing: TimingGesture<T> | null = null;
  let pan: { pointerId: number; jobId: string; x: number; width: number; start: number; duration: number; moved: boolean } | null = null;
  let playhead: { pointerId: number; jobId: string; start: number; end: number } | null = null;
  let pinch: { jobId: string; duration: number; anchor: number; ratio: number } | null = null;
  let suppressClick = false;
  const active = () => timing || pan || playhead;
  const release = (id: number | undefined) => { if (id !== undefined && canvas.hasPointerCapture(id)) canvas.releasePointerCapture(id); };
  const reset = () => {
    const id = active()?.pointerId;
    timing = null; pan = null; playhead = null; pinch = null; suppressClick = false;
    release(id); canvas.style.cursor = "crosshair";
  };
  const cancel = () => {
    const hadTiming = !!timing;
    reset(); suppressClick = true;
    if (hadTiming) options.cancelled();
  };
  const hit = (x: number, y: number, state: Snapshot<T>) => {
    const bounds = canvas.getBoundingClientRect(), { trackTop } = timelineGeometry(bounds.height);
    if (y - bounds.top < trackTop || y - bounds.top > bounds.height - 4) return null;
    return timelineCueHit(state.cues, x - bounds.left, bounds.width, state.waveform.start_ms, state.waveform.end_ms);
  };
  const handle = (x: number, y: number, state: Snapshot<T>) => {
    const b = canvas.getBoundingClientRect();
    return playheadHandleContains(x - b.left, y - b.top, b.width, state.currentMs, state.waveform.start_ms, state.waveform.end_ms);
  };
  const seekHead = (x: number) => {
    const state = options.snapshot(), drag = playhead;
    if (!state || !drag || drag.jobId !== state.jobId) { cancel(); return; }
    const b = canvas.getBoundingClientRect();
    options.seek(timelineTimeAtX(x - b.left, b.width, drag.start, drag.end));
  };
  const boundedPan = (start: number, state: Snapshot<T>) => Math.max(0, Math.min(Math.max(0, state.waveform.duration_ms - state.durationMs), start));
  const moveWindow = (direction: number) => {
    const state = options.snapshot(); if (!state || active()) return;
    options.stopFollowing(); options.pan(boundedPan(state.viewStartMs + state.durationMs * .8 * direction, state));
  };
  canvas.addEventListener("pointerdown", event => {
    const state = options.snapshot();
    if (event.button !== 0 || !state || active()) return;
    const b = canvas.getBoundingClientRect(); if (b.width <= 0) return;
    suppressClick = false;
    const isHandle = handle(event.clientX, event.clientY, state);
    const target = isHandle ? null : hit(event.clientX, event.clientY, state);
    if (isHandle) {
      options.pause();
      playhead = { pointerId: event.pointerId, jobId: state.jobId, start: state.waveform.start_ms, end: state.waveform.end_ms };
      suppressClick = true; canvas.style.cursor = "ew-resize";
    } else if (target) {
      if (!options.select(state.cues[target.segmentIndex].id)) { suppressClick = true; return; }
      options.pause();
      timing = { jobId: state.jobId, pointerId: event.pointerId, segmentIndex: target.segmentIndex, mode: target.mode,
        pointerStartX: event.clientX, width: b.width, rangeMs: state.waveform.end_ms - state.waveform.start_ms,
        before: state.cues.map(cue => ({ ...cue })), draft: state.cues.map(cue => ({ ...cue })), moved: false };
      suppressClick = true;
    } else {
      pan = { pointerId: event.pointerId, jobId: state.jobId, x: event.clientX, width: b.width,
        start: state.viewStartMs, duration: state.durationMs, moved: false };
    }
    canvas.setPointerCapture(event.pointerId); event.preventDefault();
  }, { signal: events.signal });
  canvas.addEventListener("pointermove", event => {
    const state = options.snapshot(); if (!state) { if (active()) cancel(); return; }
    if (!active()) {
      const target = hit(event.clientX, event.clientY, state);
      canvas.style.cursor = handle(event.clientX, event.clientY, state) ? "ew-resize" : target?.mode === "move" ? "grab" : target ? "ew-resize" : "crosshair";
      return;
    }
    if (active()?.jobId !== state.jobId) { cancel(); return; }
    if (playhead?.pointerId === event.pointerId) seekHead(event.clientX);
    if (timing?.pointerId === event.pointerId) {
      const moved = timing.moved;
      updateTimingPreview(timing, event.clientX, state.waveform.duration_ms);
      if (!moved && timing.moved) options.stopFollowing();
      options.preview(timing);
    }
    if (pan?.pointerId === event.pointerId) {
      const delta = event.clientX - pan.x, moved = pan.moved;
      pan.moved ||= Math.abs(delta) >= 3;
      if (!moved && pan.moved) options.stopFollowing();
      if (pan.moved) { canvas.style.cursor = "grabbing"; options.pan(boundedPan(pan.start - delta / pan.width * pan.duration, state)); }
    }
    event.preventDefault();
  }, { signal: events.signal });
  canvas.addEventListener("pointerup", event => {
    if (playhead?.pointerId === event.pointerId) { seekHead(event.clientX); playhead = null; suppressClick = true; }
    if (timing?.pointerId === event.pointerId) {
      const drag = timing, state = options.snapshot();
      if (state?.jobId === drag.jobId) updateTimingPreview(drag, event.clientX, state.waveform.duration_ms);
      timing = null;
      if (state?.jobId === drag.jobId) void options.commit(drag);
      else options.cancelled();
    }
    if (pan?.pointerId === event.pointerId) { suppressClick = pan.moved; pan = null; }
    release(event.pointerId); event.preventDefault();
  }, { signal: events.signal });
  canvas.addEventListener("pointercancel", cancel, { signal: events.signal });
  canvas.addEventListener("lostpointercapture", () => { if (active()) cancel(); }, { signal: events.signal });
  document.addEventListener("keydown", event => {
    if (event.key === "Escape" && active()) { event.preventDefault(); event.stopPropagation(); cancel(); }
  }, { capture: true, signal: events.signal });
  canvas.addEventListener("wheel", event => {
    if (active()) { event.preventDefault(); return; }
    const state = options.snapshot(); if (!state) return;
    const b = canvas.getBoundingClientRect(); if (b.width <= 0) return;
    if (event.ctrlKey || event.metaKey) {
      event.preventDefault(); if (pinch) return;
      const ratio = Math.max(0, Math.min(1, (event.clientX - b.left) / b.width));
      const anchor = state.viewStartMs + ratio * state.durationMs;
      options.zoom(Math.max(0, Math.min(100, options.zoomValue() + event.deltaY * .08)));
      options.stopFollowing(); options.pan(Math.max(0, anchor - ratio * (options.snapshot()?.durationMs ?? state.durationMs)));
    } else {
      const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.shiftKey ? event.deltaY : 0;
      if (delta !== 0) { options.stopFollowing(); options.pan(boundedPan(state.viewStartMs + delta / b.width * state.durationMs, state)); event.preventDefault(); }
    }
  }, { passive: false, signal: events.signal });
  canvas.addEventListener("gesturestart", ((event: GestureEvent) => {
    const state = options.snapshot(); if (!state || active()) return;
    const b = canvas.getBoundingClientRect(); if (b.width <= 0) return;
    const ratio = Math.max(0, Math.min(1, (event.clientX - b.left) / b.width));
    pinch = { jobId: state.jobId, duration: state.durationMs, anchor: state.viewStartMs + ratio * state.durationMs, ratio };
    options.stopFollowing(); event.preventDefault();
  }) as EventListener, { passive: false, signal: events.signal });
  canvas.addEventListener("gesturechange", ((event: GestureEvent) => {
    const state = options.snapshot(); if (!state || !pinch || state.jobId !== pinch.jobId) { pinch = null; return; }
    options.zoom(options.zoomForDuration(Math.max(2_000, Math.min(120_000, pinch.duration / Math.max(.05, event.scale)))));
    options.pan(Math.max(0, pinch.anchor - pinch.ratio * (options.snapshot()?.durationMs ?? state.durationMs))); event.preventDefault();
  }) as EventListener, { passive: false, signal: events.signal });
  canvas.addEventListener("gestureend", event => { pinch = null; event.preventDefault(); }, { passive: false, signal: events.signal });
  canvas.addEventListener("click", event => {
    if (suppressClick) { suppressClick = false; return; }
    const state = options.snapshot(); if (!state) return;
    const b = canvas.getBoundingClientRect(); if (b.width <= 0) return;
    options.seek(timelineTimeAtX(event.clientX - b.left, b.width, state.waveform.start_ms, state.waveform.end_ms));
  }, { signal: events.signal });
  zoomInput.addEventListener("input", () => {
    const state = options.snapshot(); if (!state) return;
    if (active()) cancel();
    const oldRange = state.waveform.end_ms - state.waveform.start_ms;
    options.zoom(Number(zoomInput.value)); options.stopFollowing();
    options.pan(Math.max(0, state.viewStartMs + oldRange / 2 - state.durationMs / 2));
  }, { signal: events.signal });
  root.querySelector("#karaoke-window-back")?.addEventListener("click", () => moveWindow(-1), { signal: events.signal });
  root.querySelector("#karaoke-window-forward")?.addEventListener("click", () => moveWindow(1), { signal: events.signal });
  return { get timing() { return timing; }, get active() { return !!active(); }, reset, dispose() { events.abort(); reset(); } };
}
