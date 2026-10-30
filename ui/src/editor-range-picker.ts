import { createEditorWaveform } from "./editor-waveform";
import { mountEditorTimeline } from "./editor-timeline-view";
import type { SubtitleSegment, WaveformWindow } from "./editor-types";
type Range = { startMs: number; endMs: number };
type Options = {
  context(): { jobId: string; navigation: number; durationMs: number; cues: SubtitleSegment[] } | null;
  request(jobId: string, startMs: number, endMs: number, points: number): Promise<WaveformWindow>;
  time(ms: number): string;
  parse(value: string): number | null;
  position(): number;
  changed(): void;
  audition(range: Range): void;
  stop(): void;
};
/** A disposable read-only timeline. Only the range inputs change; cue timing never does. */
export function mountEditorRangePicker(root: HTMLElement, options: Options) {
  const canvas = root.querySelector<HTMLCanvasElement>("#repair-range-canvas")!;
  const startInput = root.querySelector<HTMLInputElement>("#retranscription-start")!;
  const endInput = root.querySelector<HTMLInputElement>("#retranscription-end")!;
  const summary = root.querySelector<HTMLElement>("#repair-range-summary")!;
  const text = root.querySelector<HTMLElement>("#repair-range-text")!;
  const status = root.querySelector<HTMLElement>("#repair-wave-status")!;
  const zoom = root.querySelector<HTMLInputElement>("#repair-range-zoom")!;
  const stage = root.querySelector<HTMLElement>(".repair-range-stage")!;
  const shade = root.querySelector<HTMLElement>(".repair-range-selection")!;
  const handles = [...root.querySelectorAll<HTMLButtonElement>("[data-range-edge]")];
  const events = new AbortController();
  let opened = false, windowMs = 30_000, frame: number | null = null;
  let metadataKey = "", metadataCues: SubtitleSegment[] | null = null;
  let drag: { id: number; edge: "start" | "end" | "new"; anchor: number; start: number; end: number; width: number; left: number } | null = null;
  const range = (): Range | null => {
    const startMs = options.parse(startInput.value), endMs = options.parse(endInput.value);
    return startMs !== null && endMs !== null && startMs >= 0 && endMs > startMs ? { startMs, endMs } : null;
  };
  const duration = () => waveform.window?.duration_ms || options.context()?.durationMs || 0;
  const waveform = createEditorWaveform({
    context: () => { const c = options.context(); return opened && c ? { ...c, durationMs: duration(), windowMs } : null; },
    points: () => canvas.clientWidth * devicePixelRatio,
    request: options.request,
    started: () => { status.textContent = "正在读取波形…"; },
    accepted: () => { status.textContent = "拖动两端手柄调整范围；拖动空白处重新选区；横向滚动平移。"; render(); },
    failed: error => { status.textContent = `波形读取失败：${String(error)}`; render(); },
  });
  const timeline = mountEditorTimeline(canvas, root, { time: options.time, resized: () => { if (opened) render(); } });
  const setRange = (start: number, end: number) => {
    startInput.value = options.time(Math.round(Math.max(0, start)));
    endInput.value = options.time(Math.round(Math.min(duration(), end)));
    options.changed(); render();
  };
  const view = () => ({ startMs: waveform.startMs, endMs: waveform.startMs + windowMs });
  const render = () => {
    if (!opened || frame !== null) return;
    frame = requestAnimationFrame(() => {
      frame = null;
      if (!opened) return;
      const context = options.context(), selected = range(), viewport = view();
      const cues = context?.cues.filter(cue => cue.start_ms < viewport.endMs && cue.end_ms > viewport.startMs) ?? [];
      timeline.render({ waveform: waveform.window, viewport, cues, selectedCueId: null, draggedCueId: null, playbackCueId: null, currentMs: options.position(), gain: 1, emptyMessage: "正在准备波形…" });
      shade.hidden = !selected;
      for (const handle of handles) {
        const ms = selected ? handle.dataset.rangeEdge === "start" ? selected.startMs : selected.endMs : -1;
        const percent = (ms - viewport.startMs) / windowMs * 100;
        handle.hidden = !selected || percent < 0 || percent > 100;
        handle.style.left = `${Math.max(0, Math.min(100, percent))}%`;
        handle.setAttribute("aria-label", `${handle.dataset.rangeEdge === "start" ? "选区开始" : "选区结束"} ${options.time(Math.max(0, ms))}`);
      }
      if (!selected) { metadataKey = ""; summary.textContent = "尚未选择有效范围，请拖动波形选区或填写起止时间。"; text.textContent = ""; return; }
      const left = Math.max(0, Math.min(100, (selected.startMs - viewport.startMs) / windowMs * 100));
      const right = Math.max(0, Math.min(100, (selected.endMs - viewport.startMs) / windowMs * 100));
      shade.style.left = `${left}%`; shade.style.width = `${right - left}%`;
      const key = `${context?.jobId}:${selected.startMs}:${selected.endMs}`;
      if (key === metadataKey && metadataCues === context?.cues) return;
      metadataKey = key; metadataCues = context?.cues ?? null;
      const affected = context?.cues.filter(cue => cue.start_ms < selected.endMs && cue.end_ms > selected.startMs) ?? [];
      const cut = affected.some(cue => cue.start_ms < selected.startMs || cue.end_ms > selected.endMs);
      summary.textContent = `${options.time(selected.startMs)} → ${options.time(selected.endMs)} · ${((selected.endMs - selected.startMs) / 1000).toFixed(2)} 秒 · ${affected.length} 段字幕${cut ? " · 边界切穿字幕，请对齐到完整字幕块" : affected.length ? " · 完整字幕范围" : " · 字幕空白范围"}`;
      text.textContent = affected.map(cue => `#${cue.segment_index + 1} ${cue.source_text}`).join("\n");
    });
  };
  const pan = (start: number) => { waveform.schedule(Math.max(0, Math.min(Math.max(0, duration() - windowMs), start))); render(); };
  const timeAt = (x: number, left: number, width: number, start: number, end: number) => Math.round(Math.max(0, Math.min(duration(), start + Math.max(0, Math.min(1, (x - left) / Math.max(1, width))) * (end - start))));
  stage.addEventListener("pointerdown", event => {
    if (event.button !== 0 || !waveform.window) return;
    const bounds = canvas.getBoundingClientRect(), selected = range(), viewport = view();
    const edge = (event.target as HTMLElement).closest<HTMLElement>("[data-range-edge]")?.dataset.rangeEdge as "start" | "end" | undefined;
    const anchor = Math.min(Math.max(0, duration() - 1), timeAt(event.clientX, bounds.left, bounds.width, viewport.startMs, viewport.endMs));
    drag = { id: event.pointerId, edge: edge ?? "new", anchor, start: viewport.startMs, end: viewport.endMs, width: bounds.width, left: bounds.left };
    if (!edge || !selected) setRange(anchor, Math.min(duration(), anchor + 10));
    stage.setPointerCapture(event.pointerId); event.preventDefault(); options.stop();
  }, { signal: events.signal });
  const move = (event: PointerEvent) => {
    if (!drag || drag.id !== event.pointerId) return;
    const selected = range(); if (!selected) return;
    const ms = timeAt(event.clientX, drag.left, drag.width, drag.start, drag.end);
    if (drag.edge === "start") setRange(Math.min(ms, selected.endMs - 1), selected.endMs);
    else if (drag.edge === "end") setRange(selected.startMs, Math.max(selected.startMs + 1, ms));
    else setRange(Math.min(ms, drag.anchor), Math.max(ms, drag.anchor) + (ms === drag.anchor ? 1 : 0));
    event.preventDefault();
  };
  stage.addEventListener("pointermove", move, { signal: events.signal });
  stage.addEventListener("pointerup", event => { move(event); drag = null; if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId); }, { signal: events.signal });
  for (const name of ["pointercancel", "lostpointercapture"]) stage.addEventListener(name, () => { drag = null; }, { signal: events.signal });
  for (const handle of handles) handle.addEventListener("keydown", event => {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    const selected = range(); if (!selected) return;
    const delta = (event.key === "ArrowLeft" ? -1 : 1) * (event.shiftKey ? 100 : 10);
    if (handle.dataset.rangeEdge === "start") setRange(Math.max(0, Math.min(selected.endMs - 1, selected.startMs + delta)), selected.endMs);
    else setRange(selected.startMs, Math.max(selected.startMs + 1, Math.min(duration(), selected.endMs + delta)));
    event.preventDefault(); event.stopPropagation();
  }, { signal: events.signal });
  canvas.addEventListener("wheel", event => {
    if (drag) return;
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.shiftKey ? event.deltaY : 0;
    if (delta) { pan(waveform.startMs + delta / Math.max(1, canvas.clientWidth) * windowMs); event.preventDefault(); }
  }, { passive: false, signal: events.signal });
  zoom.addEventListener("input", () => {
    const center = waveform.startMs + windowMs / 2;
    windowMs = Math.round(2000 * Math.pow(60, Number(zoom.value) / 100)); pan(center - windowMs / 2);
  }, { signal: events.signal });
  for (const direction of [-1, 1]) root.querySelector(`[data-range-pan="${direction}"]`)?.addEventListener("click", () => pan(waveform.startMs + direction * windowMs * .75), { signal: events.signal });
  root.querySelector("#repair-range-audition")?.addEventListener("click", () => { const selected = range(); if (selected) options.audition(selected); }, { signal: events.signal });
  root.querySelector("#repair-range-stop")?.addEventListener("click", options.stop, { signal: events.signal });
  for (const input of [startInput, endInput]) input.addEventListener("input", () => { options.changed(); render(); }, { signal: events.signal });
  const sync = (recenter = false) => {
    render();
    if (opened && recenter) { const selected = range(); if (selected) void waveform.load((selected.startMs + selected.endMs) / 2); }
  };
  const close = () => {
    opened = false; drag = null; waveform.reset(); timeline.clear(); options.stop();
    if (frame !== null) cancelAnimationFrame(frame); frame = null;
  };
  return {
    open() {
      opened = true; metadataKey = "";
      const selected = range();
      windowMs = Math.max(8000, Math.min(120000, selected ? (selected.endMs - selected.startMs) * 3 : 30000));
      zoom.value = String(Math.log(windowMs / 2000) / Math.log(60) * 100);
      void waveform.load(selected ? (selected.startMs + selected.endMs) / 2 : 0); render();
    }, sync, close,
    dispose() { close(); events.abort(); waveform.dispose(); timeline.dispose(); },
  };
}
