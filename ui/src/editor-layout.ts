export type EditorLayout = { version: 1; video: boolean; waveform: boolean; videoRatio: number; topRatio: number; waveformRatio: number };
export const defaultEditorLayout: EditorLayout = { version: 1, video: true, waveform: true, videoRatio: 34, topRatio: 39, waveformRatio: 25 };
const limit = (value: unknown, fallback: number, min: number, max: number): number => typeof value === "number" && Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
export function readEditorLayout(raw: string | null): EditorLayout {
  try {
    const value = JSON.parse(raw ?? "null");
    if (value?.version !== 1) return { ...defaultEditorLayout };
    return { version: 1, video: typeof value.video === "boolean" ? value.video : true,
      waveform: typeof value.waveform === "boolean" ? value.waveform : true,
      videoRatio: limit(value.videoRatio, 34, 20, 55), topRatio: limit(value.topRatio, 39, 28, 52), waveformRatio: limit(value.waveformRatio, 25, 18, 32) };
  } catch { return { ...defaultEditorLayout }; }
}
/** One owner for resize handlers and layout preferences; hiding video keeps media mounted. */
export function mountEditorLayout(root: HTMLElement): { dispose(): void; refresh(): void } {
  const key = "atogaki.editorLayout.v1";
  let layout = readEditorLayout(null);
  try { layout = readEditorLayout(localStorage.getItem(key)); } catch { /* Preferences are optional. */ }
  const events = new AbortController();
  let drag: { pointerId: number; field: "videoRatio" | "topRatio" | "waveformRatio"; origin: number; value: number; extent: number; target: HTMLElement } | null = null;
  const render = () => {
    root.classList.toggle("editor-video-hidden", !layout.video);
    root.classList.toggle("editor-waveform-hidden", !layout.waveform);
    root.style.setProperty("--editor-video-ratio", `${layout.videoRatio}fr`);
    root.style.setProperty("--editor-text-ratio", `${100 - layout.videoRatio}fr`);
    root.style.setProperty("--editor-top-ratio", `${layout.topRatio}fr`);
    root.style.setProperty("--editor-wave-ratio", `${layout.waveformRatio}fr`);
    root.style.setProperty("--editor-list-ratio", `${100 - layout.topRatio - (layout.waveform ? layout.waveformRatio : 0)}fr`);
    for (const panel of ["video", "waveform"] as const) {
      const toggle = root.querySelector<HTMLButtonElement>(`[data-editor-toggle="${panel}"]`);
      toggle?.setAttribute("aria-expanded", String(layout[panel]));
      if (toggle) toggle.textContent = `${layout[panel] ? "收起" : "展开"}${panel === "video" ? "视频" : "波形"}`;
    }
    root.querySelectorAll<HTMLElement>("[data-editor-resize]").forEach(handle => handle.setAttribute("aria-valuenow", String(Math.round(layout[handle.dataset.editorResize as "videoRatio"])))) ;
  };
  const persist = () => { try { localStorage.setItem(key, JSON.stringify(layout)); } catch { /* Layout still works without storage. */ } };
  root.addEventListener("click", event => {
    const target = (event.target as HTMLElement).closest<HTMLElement>("[data-editor-toggle], [data-editor-reset]");
    if (!target) return;
    const panel = target.dataset.editorToggle;
    if (panel === "video" || panel === "waveform") layout[panel] = !layout[panel];
    else layout = { ...defaultEditorLayout };
    render(); persist();
  }, { signal: events.signal });
  root.querySelectorAll<HTMLElement>("[data-editor-resize]").forEach(handle => {
    const field = handle.dataset.editorResize as "videoRatio" | "topRatio" | "waveformRatio";
    const change = (value: number) => { layout = readEditorLayout(JSON.stringify({ ...layout, [field]: value })); render(); };
    handle.addEventListener("keydown", event => {
      const negative = field === "videoRatio" ? "ArrowLeft" : "ArrowUp", positive = field === "videoRatio" ? "ArrowRight" : "ArrowDown";
      if (event.key !== negative && event.key !== positive && event.key !== "Home") return;
      event.preventDefault(); event.stopPropagation();
      change(event.key === "Home" ? defaultEditorLayout[field] : layout[field] + (event.key === negative ? -2 : 2)); persist();
    }, { signal: events.signal });
    handle.addEventListener("pointerdown", event => {
      if (event.button !== 0) return;
      event.preventDefault(); handle.setPointerCapture(event.pointerId);
      const bounds = root.querySelector<HTMLElement>(field === "videoRatio" ? ".editor-top" : ".editor-panels")!.getBoundingClientRect();
      drag = { pointerId: event.pointerId, field, origin: field === "videoRatio" ? event.clientX : event.clientY, value: layout[field], extent: field === "videoRatio" ? bounds.width : bounds.height, target: handle };
    }, { signal: events.signal });
    handle.addEventListener("pointermove", event => {
      if (!drag || drag.target !== handle || drag.pointerId !== event.pointerId) return;
      change(drag.value + ((field === "videoRatio" ? event.clientX : event.clientY) - drag.origin) / Math.max(1, drag.extent) * 100);
    }, { signal: events.signal });
    const finish = () => { drag = null; persist(); };
    handle.addEventListener("pointerup", finish, { signal: events.signal });
    handle.addEventListener("pointercancel", finish, { signal: events.signal });
    handle.addEventListener("lostpointercapture", finish, { signal: events.signal });
  });
  render();
  return { refresh: render, dispose() { events.abort(); drag = null; } };
}
