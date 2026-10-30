import { timelineGeometry } from "./editor-timeline-view";
import type { SubtitleSegment, WaveformWindow } from "./editor-types";
type Range = { startMs: number; endMs: number };
type Options = {
  context(): { jobId: string; navigation: number; durationMs: number; cues: SubtitleSegment[] } | null;
  timeline(): { waveform: WaveformWindow | null; startMs: number; endMs: number };
  canvas: HTMLCanvasElement;
  time(ms: number): string;
  parse(value: string): number | null;
  changed(): void;
  audition(range: Range): void;
  stop(): void;
};
/** Mount the single editor timeline in repair; the overlay changes range inputs, never cue timing. */
export function mountEditorRangePicker(root: HTMLElement, options: Options) {
  const canvas = options.canvas;
  const panel = canvas.closest<HTMLElement>(".waveform-panel")!;
  const host = root.querySelector<HTMLElement>("#repair-timeline-host")!;
  const panelAnchor = document.createComment("shared-timeline-home");
  const canvasAnchor = document.createComment("shared-canvas-home");
  panel.before(panelAnchor); canvas.before(canvasAnchor);
  const startInput = root.querySelector<HTMLInputElement>("#retranscription-start")!;
  const endInput = root.querySelector<HTMLInputElement>("#retranscription-end")!;
  const summary = root.querySelector<HTMLElement>("#repair-range-summary")!;
  const text = root.querySelector<HTMLElement>("#repair-range-text")!;
  const status = root.querySelector<HTMLElement>("#repair-wave-status")!;
  const stage = root.querySelector<HTMLElement>(".repair-range-stage")!;
  const shade = root.querySelector<HTMLElement>(".repair-range-selection")!;
  const handles = [...root.querySelectorAll<HTMLButtonElement>("[data-range-edge]")];
  const events = new AbortController();
  let opened = false, frame: number | null = null;
  let metadataKey = "", metadataCues: SubtitleSegment[] | null = null;
  let drag: { id: number; edge: "start" | "end" | "new"; anchor: number; start: number; end: number; width: number; left: number; x: number; moved: boolean } | null = null;
  const range = (): Range | null => {
    const startMs = options.parse(startInput.value), endMs = options.parse(endInput.value);
    return startMs !== null && endMs !== null && startMs >= 0 && endMs > startMs ? { startMs, endMs } : null;
  };
  const duration = () => options.timeline().waveform?.duration_ms || options.context()?.durationMs || 0;
  const setEdge = (edge: "start" | "end", ms: number) => {
    (edge === "start" ? startInput : endInput).value = options.time(Math.round(Math.max(0, Math.min(duration(), ms))));
    options.changed(); render();
  };
  const setRange = (start: number, end: number) => {
    startInput.value = options.time(Math.round(Math.max(0, start)));
    endInput.value = options.time(Math.round(Math.min(duration(), end)));
    options.changed(); render();
  };
  const view = () => options.timeline();
  const render = () => {
    if (!opened || frame !== null) return;
    frame = requestAnimationFrame(() => {
      frame = null;
      if (!opened) return;
      const context = options.context(), selected = range(), viewport = view();
      const windowMs = viewport.endMs - viewport.startMs;
      status.textContent = "与主时间轴同步；按住拖动波形或字幕轨道重新选区，拖动整条起止线调整；标尺定位，横向滚动或 Alt＋拖动波形平移。";
      shade.hidden = !selected;
      for (const handle of handles) {
        const ms = options.parse(handle.dataset.rangeEdge === "start" ? startInput.value : endInput.value) ?? -1;
        const percent = (ms - viewport.startMs) / windowMs * 100;
        handle.hidden = ms < 0 || percent < 0 || percent > 100;
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
  const timeAt = (x: number, left: number, width: number, start: number, end: number) => Math.round(Math.max(0, Math.min(duration(), start + Math.max(0, Math.min(1, (x - left) / Math.max(1, width))) * (end - start))));
  stage.addEventListener("pointerdown", event => {
    if (event.button !== 0 || !options.timeline().waveform) return;
    const bounds = canvas.getBoundingClientRect(), viewport = view();
    const edge = (event.target as HTMLElement).closest<HTMLElement>("[data-range-edge]")?.dataset.rangeEdge as "start" | "end" | undefined;
    // Keep the ruler/playhead and explicit pan gestures with the shared controller.
    const y = event.clientY - bounds.top;
    if (!edge && (y < 38 || event.altKey && y < timelineGeometry(bounds.height).trackTop)) return;
    event.stopPropagation();
    const anchor = Math.min(Math.max(0, duration() - 1), timeAt(event.clientX, bounds.left, bounds.width, viewport.startMs, viewport.endMs));
    drag = { id: event.pointerId, edge: edge ?? "new", anchor, start: viewport.startMs, end: viewport.endMs, width: bounds.width, left: bounds.left, x: event.clientX, moved: false };
    stage.setPointerCapture(event.pointerId); event.preventDefault(); options.stop();
  }, { capture: true, signal: events.signal });
  const move = (event: PointerEvent) => {
    if (!drag || drag.id !== event.pointerId) return;
    event.stopPropagation();
    drag.moved ||= Math.abs(event.clientX - drag.x) >= 3;
    if (drag.edge === "new" && !drag.moved) return;
    const ms = timeAt(event.clientX, drag.left, drag.width, drag.start, drag.end);
    if (drag.edge === "start" || drag.edge === "end") setEdge(drag.edge, ms);
    else setRange(Math.min(ms, drag.anchor), Math.max(ms, drag.anchor) + (ms === drag.anchor ? 1 : 0));
    event.preventDefault();
  };
  stage.addEventListener("pointermove", move, { capture: true, signal: events.signal });
  stage.addEventListener("pointerup", event => { if (!drag) return; move(event); drag = null; if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId); }, { capture: true, signal: events.signal });
  stage.addEventListener("click", event => { if ((event.target as HTMLElement).closest("[data-range-edge]") || !event.altKey && event.clientY - canvas.getBoundingClientRect().top >= 38) event.stopPropagation(); }, { capture: true, signal: events.signal });
  for (const name of ["pointercancel", "lostpointercapture"]) stage.addEventListener(name, () => { drag = null; }, { signal: events.signal });
  for (const handle of handles) handle.addEventListener("keydown", event => {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    const edge = handle.dataset.rangeEdge as "start" | "end";
    const value = options.parse(edge === "start" ? startInput.value : endInput.value); if (value === null) return;
    const delta = (event.key === "ArrowLeft" ? -1 : 1) * (event.shiftKey ? 100 : 10);
    setEdge(edge, value + delta);
    event.preventDefault(); event.stopPropagation();
  }, { signal: events.signal });
  root.querySelector("#repair-range-audition")?.addEventListener("click", () => { const selected = range(); if (selected) options.audition(selected); }, { signal: events.signal });
  root.querySelector("#repair-range-stop")?.addEventListener("click", options.stop, { signal: events.signal });
  for (const input of [startInput, endInput]) input.addEventListener("input", () => { options.changed(); render(); }, { signal: events.signal });
  const sync = () => render();
  const close = () => {
    if (!opened) return;
    opened = false; drag = null; options.stop();
    canvasAnchor.after(canvas); host.append(stage); panelAnchor.after(panel);
    if (frame !== null) cancelAnimationFrame(frame); frame = null;
  };
  return {
    open() {
      if (opened) return;
      opened = true; metadataKey = "";
      host.append(panel); stage.prepend(canvas); canvasAnchor.after(stage); render();
    }, sync, close,
    dispose() { close(); events.abort(); panelAnchor.remove(); canvasAnchor.remove(); },
  };
}
